//! События `winit` (ADR-0004): уведомления приложения, применение [`Request`], обработчик цикла.

use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::PhysicalPosition;
use winit::event::{Ime, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::{Fullscreen, WindowId};

use super::{App, AppHandler, Ctx, Grid, GridMode, PointerEvent, Request};

impl ApplicationHandler for App {
	fn resumed(&mut self, event_loop: &ActiveEventLoop) {
		if self.window.is_some() {
			return;
		}
		if let Err(e) = self.create_window_and_backend(event_loop) {
			self.stop_with(event_loop, e);
		}
	}

	fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
		match event {
			WindowEvent::CloseRequested => event_loop.exit(),
			WindowEvent::Resized(size) => match self.resize_window(size, event_loop) {
				Ok(()) => self.request_redraw(),
				Err(e) => self.stop_with(event_loop, e),
			},
			WindowEvent::ScaleFactorChanged {
				scale_factor,
				mut inner_size_writer,
			} => self.apply_scale_factor(event_loop, scale_factor, &mut inner_size_writer),
			WindowEvent::RedrawRequested => {
				if !std::mem::take(&mut self.is_redraw_requested) {
					self.is_full_present_pending = true;
				}
				self.redraw(event_loop)
			}
			// Нажатия и отпускания (ADR-0028): состояние проверяет приложение.
			WindowEvent::KeyboardInput { event, .. } => {
				let request = self.notify(|handler, ctx| handler.on_key(&event, ctx));
				self.apply_request(&request, event_loop);
			}
			// Модификаторы приходят отдельным событием до клавиши; приложение читает их из `Ctx`.
			WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
			// Фокус окна (ADR-0029): библиотека ничего не сбрасывает, решает приложение.
			WindowEvent::Focused(focused) => {
				let request = self.notify(|handler, ctx| handler.on_focus(focused, ctx));
				self.apply_request(&request, event_loop);
			}
			WindowEvent::Ime(Ime::Commit(text)) => {
				let request = self.notify(|handler, ctx| handler.on_text(&text, ctx));
				self.apply_request(&request, event_loop);
			}
			WindowEvent::CursorMoved { position, .. } => {
				// `winit` не передаёт дельту мыши, поэтому она считается из прошлой позиции.
				let delta =
					PhysicalPosition::new(position.x - self.pointer.x, position.y - self.pointer.y);
				self.pointer = position;
				let cell = self.pointer_cell();
				let request = self.notify(|handler, ctx| {
					handler.on_pointer(
						&PointerEvent::Moved {
							position,
							delta,
							cell,
						},
						ctx,
					)
				});
				self.apply_request(&request, event_loop);
			}
			WindowEvent::MouseInput { state, button, .. } => {
				let cell = self.pointer_cell();
				let request = self.notify(|handler, ctx| {
					handler.on_pointer(
						&PointerEvent::Input {
							state,
							button,
							cell,
						},
						ctx,
					)
				});
				self.apply_request(&request, event_loop);
			}
			WindowEvent::MouseWheel { delta, .. } => {
				let cell = self.pointer_cell();
				let request = self.notify(|handler, ctx| {
					handler.on_pointer(
						&PointerEvent::Wheel {
							delta: &delta,
							cell,
						},
						ctx,
					)
				});
				self.apply_request(&request, event_loop);
			}
			WindowEvent::CursorLeft { .. } => {
				let request =
					self.notify(|handler, ctx| handler.on_pointer(&PointerEvent::Left, ctx));
				self.apply_request(&request, event_loop);
			}
			_ => {}
		}
	}

	/// Тик приложения (ADR-0033). Срок проверяется здесь, перед каждым ожиданием, а не по причине
	/// пробуждения: так тик приходит и тогда, когда цикл занят вводом или кадрами.
	fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
		// До создания окна и после запроса выхода тиков нет.
		if self.window.is_none() || event_loop.exiting() {
			return;
		}
		self.run_due_tick(event_loop);
		let control_flow = match self.ticker.deadline() {
			Some(deadline) => ControlFlow::WaitUntil(deadline),
			None => ControlFlow::Wait,
		};
		event_loop.set_control_flow(control_flow);
	}
}

impl App {
	/// Вызвать обработчик с `Ctx` и вернуть накопленные команды: применяются вызывающим, чтобы
	/// приложение не наблюдало состояние, которое ещё не применено.
	pub(super) fn notify<F>(&mut self, f: F) -> Request
	where
		F: FnOnce(&mut dyn AppHandler, &mut Ctx<'_>),
	{
		let mut request = Request::default();
		let Some(mut handler) = self.handler.take() else {
			return request;
		};
		let cell_px = self.cell_px_u32();
		{
			let mut ctx = Ctx {
				screen: &mut self.screen,
				request: &mut request,
				stats: self.stats,
				modifiers: self.modifiers,
				cell_px,
			};
			f(handler.as_mut(), &mut ctx);
		}
		self.handler = Some(handler);
		request
	}

	/// Вызвать `on_tick`, если срок наступил. Следующий срок отсчитывается после возврата из
	/// обработчика, поэтому пропущенные тики не догоняются; смена периода в самом обработчике
	/// применяется следом и отсчёт не сбивает.
	fn run_due_tick(&mut self, event_loop: &ActiveEventLoop) {
		self.ticker.start(Instant::now());
		if !self.ticker.is_due(Instant::now()) {
			return;
		}
		let request = self.notify(|handler, ctx| handler.on_tick(ctx));
		self.ticker.restart(Instant::now());
		self.apply_request(&request, event_loop);
	}

	/// Применить команды приложения: сетка, полный экран, бэкенд, период тика, перерисовка, выход.
	pub(super) fn apply_request(&mut self, request: &Request, event_loop: &ActiveEventLoop) {
		// Сетку задаёт приложение только в `FixedGrid`: в `AutoGrid` она выводится из окна, и
		// следующая же перерисовка по размеру окна вернула бы заданное значение обратно (FR-23).
		if let (true, Some(grid)) = (self.options.grid_mode == GridMode::FixedGrid, request.grid) {
			if (grid.cols, grid.rows) != (self.screen.cols(), self.screen.rows()) {
				let old = Grid::new(self.screen.cols(), self.screen.rows());
				self.screen.resize(grid.cols, grid.rows);
				let inner = self.notify(|handler, ctx| handler.on_grid_resized(old, ctx));
				self.apply_request(&inner, event_loop);
			}
		}
		if let Some(fullscreen) = request.fullscreen {
			self.fullscreen = fullscreen;
			if let Some(window) = &self.window {
				// Безрамочный полный экран: размер окна меняет система, сетка приложения та же
				// в `FixedGrid`, а в `AutoGrid` пересчитается по новому размеру окна (FR-22).
				let mode = fullscreen.then_some(Fullscreen::Borderless(None));
				window.set_fullscreen(mode);
			}
		}
		if let Some(kind) = request.backend {
			self.switch_backend(event_loop, kind);
		}
		if let Some(mode) = request.box_drawing {
			match &mut self.backend {
				Some(backend) => backend.set_box_drawing_mode(mode),
				// До первого бэкенда растеризатор ещё у `App`.
				None => {
					if let Some(rasterizer) = &mut self.rasterizer {
						rasterizer.set_box_drawing_mode(mode);
					}
				}
			}
			self.request_redraw();
		}
		if let Some(period) = request.tick_period {
			self.ticker.set_period(period, Instant::now());
		}
		if request.redraw {
			self.request_redraw();
		}
		if request.exit {
			event_loop.exit();
		}
	}
}
