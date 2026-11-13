//! Кадр и цикл (§5.3 п.5, ADR-0006, ADR-0007, ADR-0010): отрисовка, повтор, инжекция ошибок,
//! запрос кадра.

use std::time::Instant;

use winit::event_loop::ActiveEventLoop;

use super::api::DrawCtx;
use super::backend::Backend;
use super::{App, AppError, FrameStats, Grid};
use crate::renderer::{RenderError, Renderer};

impl App {
	/// Один кадр плюс его статистика (ADR-0006): `cpu` — часть без ожидания монитора, `total` —
	/// весь кадр. Ошибочный кадр в статистику не попадает.
	pub(super) fn render_frame(&mut self) -> Result<(), AppError> {
		let Some(backend) = &mut self.backend else {
			return Ok(());
		};
		let started = Instant::now();
		let cpu = match backend {
			Backend::Gl(gl) => {
				gl.render_overlaid(&mut self.screen, &self.overlay)
					.map_err(AppError::Render)?;
				gl.last_cpu_time()
			}
			Backend::Soft(soft) => {
				soft.renderer
					.render_overlaid(&mut self.screen, &self.overlay)
					.map_err(AppError::Render)?;
				let cpu = started.elapsed();
				soft.present(
					std::mem::take(&mut self.is_full_present_pending),
					self.config.debug,
				)?;
				cpu
			}
		};
		self.take_injected_fault()?;
		self.stats = FrameStats {
			frames: self.stats.frames + 1,
			cpu,
			total: started.elapsed(),
		};
		Ok(())
	}

	/// Ошибка кадра по флагам `Config` (ADR-0007). Приходит после настоящего рендера и вывода —
	/// так же, как ошибка GL после кадра, — и дальше обрабатывается общим путём в `redraw`.
	pub(super) fn take_injected_fault(&mut self) -> Result<(), AppError> {
		if self.is_context_loss_pending {
			self.is_context_loss_pending = false;
			return Err(AppError::Render(RenderError::ContextLost));
		}
		if self.frames_to_fail > 0 {
			self.frames_to_fail -= 1;
			return Err(AppError::Render(RenderError::Other(
				"injected by --fail-frames".to_owned(),
			)));
		}
		Ok(())
	}

	/// Кадр с обработкой ошибок §5.3 п.5: `ContextLost` → откат сразу; иная ошибка — один
	/// повтор, затем откат; откат невозможен — остановка с ошибкой.
	pub(super) fn redraw(&mut self, event_loop: &ActiveEventLoop) {
		self.collect_overlay();
		match self.render_frame() {
			Ok(()) => {
				self.frame_retried = false;
				// Кадр состоялся — сообщаем приложению (ADR-0006): отсюда `--bench` просит следующий.
				let request = self.notify(|handler, ctx| handler.on_frame(ctx));
				self.apply_request(&request, event_loop);
			}
			Err(AppError::Render(RenderError::ContextLost)) => {
				self.fall_back(event_loop, "context lost")
			}
			Err(AppError::Render(e)) if !self.frame_retried => {
				eprintln!("gterm: frame failed ({e}); retrying once");
				self.frame_retried = true;
				self.request_redraw();
			}
			Err(AppError::Render(e)) => self.fall_back(event_loop, &e.to_string()),
			Err(e) => self.stop_with(event_loop, e),
		}
	}

	/// Наложение кадра (ADR-0014): очистить и дать приложению нарисовать свои буферы.
	fn collect_overlay(&mut self) {
		self.overlay.clear();
		let Some(mut handler) = self.handler.take() else {
			return;
		};
		let grid = Grid::new(self.screen.cols(), self.screen.rows());
		handler.on_redraw(&mut DrawCtx {
			overlay: &mut self.overlay,
			grid,
		});
		self.handler = Some(handler);
	}

	/// Запросить кадр от имени приложения (`Request::redraw`, смена размера, откат):
	/// такой `RedrawRequested` выводится частично (ADR-0010 п.4).
	pub(super) fn request_redraw(&mut self) {
		if let Some(window) = &self.window {
			self.is_redraw_requested = true;
			window.request_redraw();
		}
	}

	pub(super) fn stop_with(&mut self, event_loop: &ActiveEventLoop, error: AppError) {
		self.error = Some(error);
		event_loop.exit();
	}
}
