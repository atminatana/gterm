//! Цепочка уровней GL → GLES → Software (FR-17…FR-21, FR-28): создание окна и бэкенда, откат,
//! переключение на лету.

use std::sync::Arc;

use glutin_winit::DisplayBuilder;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use softbuffer::{Context, Surface};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes};

use crate::atlas::{Atlas, PAGE_PX_MAX, page_px_for_limit};
use crate::font::FontError;
use crate::rasterizer::Rasterizer;
use crate::renderer::gl_backend::{self, GlBackend, GlContext, GlOptions};
use crate::renderer::soft_backend::SoftBackend;
use crate::renderer::{RenderError, Renderer, RendererKind};

use super::backend::Backend;
use super::backend::SoftState;
use super::{App, AppError};

impl App {
	pub(super) fn gl_options(&self) -> GlOptions {
		GlOptions {
			fail_gl: self.config.fail_gl,
			allow_sw_gl: self.config.allow_sw_gl,
			vsync: self.options.vsync,
		}
	}

	/// Цепочка уровней из конфигурации (FR-17…FR-19): первый успешный — активный бэкенд.
	pub(super) fn create_window_and_backend(
		&mut self,
		event_loop: &ActiveEventLoop,
	) -> Result<(), AppError> {
		let placeholder = self
			.rasterizer
			.as_ref()
			.ok_or_else(|| AppError::Font(FontError::TtfParse("rasterizer missing")))?;
		let attributes = self.window_attributes(placeholder)?;
		let options = self.gl_options();

		let mut window: Option<Arc<Window>> = None;
		let mut backend: Option<Backend> = None;

		for &kind in self.config.backend.chain() {
			if kind.is_gl() {
				if self.gl_config.is_none() {
					match Self::build_gl_window(event_loop, attributes.clone()) {
						Ok((w, c)) => {
							window = Some(w);
							self.gl_config = Some(c);
						}
						Err(reason) => {
							eprintln!("gterm: {} unavailable: {reason}", kind.name());
							continue;
						}
					}
				}
				let (Some(w), Some(c)) = (window.clone(), self.gl_config.clone()) else {
					continue;
				};
				let rasterizer = self.fit_rasterizer_to_window(&w)?;
				match Self::try_gl_level(&c, &w, kind, &options, rasterizer) {
					Ok(gl) => {
						let info = gl.info();
						eprintln!(
							"gterm: backend {} — {} / {} / {} (GL_MAX_TEXTURE_SIZE {}, glyph_px {})",
							gl.name(),
							info.vendor,
							info.renderer,
							info.version,
							info.max_texture_size,
							self.glyph_px
						);
						backend = Some(Backend::Gl(Box::new(gl)));
						break;
					}
					Err((reason, returned)) => {
						eprintln!("gterm: {} unavailable: {reason}", kind.name());
						self.rasterizer = Some(*returned);
					}
				}
			} else {
				let w = match window.take() {
					Some(w) => w,
					None => Arc::new(
						event_loop
							.create_window(attributes.clone())
							.map_err(AppError::Window)?,
					),
				};
				let rasterizer = self.fit_rasterizer_to_window(&w)?;
				let soft = Self::software_on(&w, rasterizer)?;
				eprintln!("gterm: backend {}", soft.renderer.name());
				window = Some(w);
				backend = Some(Backend::Soft(Box::new(soft)));
				break;
			}
		}
		let (Some(window), Some(backend)) = (window, backend) else {
			return Err(AppError::Render(RenderError::Other(
				"no renderer could be initialised".to_owned(),
			)));
		};
		window.set_ime_allowed(true);
		self.backend = Some(backend);
		self.window = Some(window.clone());
		self.apply_title();
		let name = self.backend_name().unwrap_or("Software");
		let request = self.notify(|handler, ctx| handler.on_backend_changed(name, ctx));
		self.apply_request(&request, event_loop);
		self.resize_window(window.inner_size(), event_loop)?;
		self.redraw(event_loop);
		if self.error.is_some() {
			return Ok(()); // причина уже сохранена
		}
		window.set_visible(true);
		// Первый кадр выведен в скрытое окно: показанное окно получает его целиком.
		self.is_full_present_pending = true;
		self.request_redraw();
		Ok(())
	}

	/// Окно и конфиг совместно (`DisplayBuilder`); на WGL окно создаётся до выбора конфига.
	pub(super) fn build_gl_window(
		event_loop: &ActiveEventLoop,
		attributes: WindowAttributes,
	) -> Result<(Arc<Window>, glutin::config::Config), String> {
		let built = DisplayBuilder::new()
			.with_window_attributes(Some(attributes.clone()))
			.build(
				event_loop,
				gl_backend::config_template(),
				gl_backend::pick_config,
			);
		match built {
			Ok((Some(window), config)) => Ok((Arc::new(window), config)),
			Ok((None, config)) => glutin_winit::finalize_window(event_loop, attributes, &config)
				.map(|w| (Arc::new(w), config))
				.map_err(|e| format!("window for GL config: {e}")),
			Err(e) => Err(format!("GL config: {e}")),
		}
	}

	/// Один GL-уровень: контекст, атлас по лимиту текстуры, бэкенд. Ошибка возвращает растеризатор.
	pub(super) fn try_gl_level(
		config: &glutin::config::Config,
		window: &Arc<Window>,
		kind: RendererKind,
		options: &GlOptions,
		rasterizer: Rasterizer,
	) -> Result<GlBackend, (String, Box<Rasterizer>)> {
		let size = window.inner_size();
		let ctx = match GlContext::new(
			config,
			window.as_ref(),
			size.width,
			size.height,
			kind,
			options,
		) {
			Ok(ctx) => ctx,
			Err(e) => return Err((e.to_string(), Box::new(rasterizer))),
		};
		let page = page_px_for_limit(Some(ctx.max_texture_size()));
		match GlBackend::new(ctx, Atlas::new(rasterizer, page)) {
			Ok(gl) => Ok(gl),
			Err((e, atlas)) => Err((e.to_string(), Box::new(atlas.into_rasterizer()))),
		}
	}

	/// Software на существующем окне (последний уровень и цель отката FR-21).
	pub(super) fn software_on(
		window: &Arc<Window>,
		rasterizer: Rasterizer,
	) -> Result<SoftState, AppError> {
		let context = Context::new(window.clone()).map_err(AppError::Surface)?;
		let surface = Surface::new(&context, window.clone()).map_err(AppError::Surface)?;
		let is_wayland = matches!(
			window.window_handle().map(|handle| handle.as_raw()),
			Ok(RawWindowHandle::Wayland(_))
		);
		Ok(SoftState {
			renderer: SoftBackend::new(Atlas::new(rasterizer, PAGE_PX_MAX)),
			surface,
			damage_rects: Vec::new(),
			is_wayland,
		})
	}

	/// Переход на Software на том же окне без перезапуска (FR-21); с Software падать некуда.
	pub(super) fn fall_back(&mut self, event_loop: &ActiveEventLoop, reason: &str) {
		let (Some(window), Some(backend)) = (self.window.clone(), self.backend.take()) else {
			return;
		};
		let rasterizer = match backend {
			Backend::Gl(gl) => {
				eprintln!(
					"gterm: {} failed ({reason}); switching to Software",
					gl.name()
				);
				gl.into_atlas().into_rasterizer()
			}
			Backend::Soft(_) => {
				self.stop_with(
					event_loop,
					AppError::Render(RenderError::Other(reason.to_owned())),
				);
				return;
			}
		};
		match Self::software_on(&window, rasterizer) {
			Ok(soft) => {
				self.backend = Some(Backend::Soft(Box::new(soft)));
				self.frame_retried = false;
				self.apply_title();
				let name = self.backend_name().unwrap_or("Software");
				let request = self.notify(|handler, ctx| handler.on_backend_changed(name, ctx));
				self.apply_request(&request, event_loop);
				if let Err(e) = self.resize_window(window.inner_size(), event_loop) {
					self.stop_with(event_loop, e);
					return;
				}
				self.request_redraw();
			}
			Err(e) => self.stop_with(event_loop, e),
		}
	}

	/// Переключение бэкенда по запросу приложения (FR-28).
	///
	/// На окно, созданное `DisplayBuilder`, можно вернуть любой GL-уровень: конфиг тот же,
	/// пересоздаётся только контекст. На окно, созданное без GL, переключиться нельзя — нужно
	/// новое окно, поэтому приложение получает сообщение в stderr, а не падение (NFR-7).
	pub(super) fn switch_backend(&mut self, event_loop: &ActiveEventLoop, kind: RendererKind) {
		let previous = self.backend_kind();
		if previous == Some(kind) {
			return;
		}
		if kind == RendererKind::Software {
			self.fall_back(event_loop, "requested by the application");
			return;
		}
		let (Some(window), Some(config)) = (self.window.clone(), self.gl_config.clone()) else {
			eprintln!(
				"gterm: {} needs a window created with a GL config; \
                 switch is not available at runtime, restart to change backend",
				kind.name()
			);
			return;
		};
		let Some(current) = self.backend.take() else {
			return;
		};
		let rasterizer = current.into_rasterizer();
		match Self::try_gl_level(&config, &window, kind, &self.gl_options(), rasterizer) {
			Ok(gl) => {
				eprintln!("gterm: backend {}", gl.name());
				self.backend = Some(Backend::Gl(Box::new(gl)));
				self.after_switch(event_loop, &window);
			}
			Err((reason, returned)) => {
				eprintln!("gterm: {} unavailable: {reason}", kind.name());
				// Снятый бэкенд пришлось бы строить заново: остался только растеризатор. Возвращаем
				// прежний уровень, а если он тоже не поднялся — Software (FR-21, NFR-7).
				self.restore_backend(event_loop, &window, previous.unwrap_or(kind), *returned);
			}
		}
	}

	/// Вернуть уровень `kind` (или Software, если он не поднимается) после неудачного переключения.
	///
	/// Растеризатор не теряется ни на одном шаге: неудачный уровень возвращает его вместе с
	/// причиной, и следующая попытка получает его обратно.
	pub(super) fn restore_backend(
		&mut self,
		event_loop: &ActiveEventLoop,
		window: &Arc<Window>,
		kind: RendererKind,
		rasterizer: Rasterizer,
	) {
		let attempt: Result<Backend, Rasterizer> = match kind {
			RendererKind::Software => match Self::software_on(window, rasterizer) {
				Ok(soft) => Ok(Backend::Soft(Box::new(soft))),
				// Растеризатор уже внутри `soft`/`gl`; его можно вернуть только созданием заново,
				// поэтому при ошибке поверхности дальше идти нечем — останавливаемся.
				Err(e) => {
					self.stop_with(event_loop, e);
					return;
				}
			},
			gl_kind => match self.gl_config.clone() {
				Some(config) => {
					match Self::try_gl_level(
						&config,
						window,
						gl_kind,
						&self.gl_options(),
						rasterizer,
					) {
						Ok(gl) => {
							eprintln!("gterm: backend {}", gl.name());
							Ok(Backend::Gl(Box::new(gl)))
						}
						Err((reason, returned)) => {
							eprintln!("gterm: {} unavailable too: {reason}", gl_kind.name());
							Err(*returned)
						}
					}
				}
				None => Err(rasterizer),
			},
		};
		let backend = match attempt {
			Ok(backend) => backend,
			Err(rasterizer) => match Self::software_on(window, rasterizer) {
				Ok(soft) => Backend::Soft(Box::new(soft)),
				Err(e) => {
					self.stop_with(event_loop, e);
					return;
				}
			},
		};
		self.backend = Some(backend);
		self.after_switch(event_loop, window);
	}

	/// Общая часть успешного переключения: заголовок, уведомление, размер, перерисовка.
	pub(super) fn after_switch(&mut self, event_loop: &ActiveEventLoop, window: &Arc<Window>) {
		self.apply_title();
		self.frame_retried = false;
		let name = self.backend_name().unwrap_or("Software");
		let request = self.notify(|handler, ctx| handler.on_backend_changed(name, ctx));
		self.apply_request(&request, event_loop);
		if let Err(e) = self.resize_window(window.inner_size(), event_loop) {
			self.stop_with(event_loop, e);
			return;
		}
		self.request_redraw();
	}
}
