//! Окно и цикл событий (ТЗ §5.3, FR-22…FR-26, FR-28) на `winit`; выбор бэкенда по цепочке уровней
//! (FR-17…FR-19), откат при ошибке инициализации, повтор кадра и откат при ошибке рендера
//! (§5.3 п.5), переключение на Software при потере контекста без перезапуска (FR-21).
//!
//! События приложению — трейт [`AppHandler`] и [`Ctx`] (ADR-0004): типы `winit` не переводятся,
//! `Esc` не перехватывается, курсор не рисуется библиотекой.
//!
//! Геометрия целиком выводится из метрик растеризации (ADR-0005): физический размер глифа — из
//! DPI окна ([`crate::font::glyph_px`]), ячейка и сетка — из него. Целочисленного масштаба нет.

mod api;
mod backend;
mod chain;
mod events;
mod frame;
mod options;
mod window;

pub use api::{AppHandler, Ctx, DrawCtx, FrameStats, PointerEvent, Request};
pub use options::{AppError, AppOptions, Grid, GridMode, auto_grid, centered_origin, pointer_cell};

use std::sync::Arc;

use winit::dpi::PhysicalPosition;
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::ModifiersState;
use winit::window::Window;

use crate::buffer::ScreenBuffer;
use crate::config::Config;
use crate::overlay::Overlay;
use crate::rasterizer::Rasterizer;
use crate::renderer::RendererKind;

use backend::Backend;

/// Окно с сеткой; бэкенд выбирается при создании окна и может смениться во время работы.
pub struct App {
	title: String,
	screen: ScreenBuffer,
	config: Config,
	options: AppOptions,
	/// Приложение; `None` только до `run` и на паузе во время вызова обработчика.
	handler: Option<Box<dyn AppHandler>>,
	/// Растеризатор до создания окна; после — внутри атласа бэкенда.
	rasterizer: Option<Rasterizer>,
	backend: Option<Backend>,
	window: Option<Arc<Window>>,
	/// Конфиг GL, если окно создано через `DisplayBuilder`: на нём можно пересоздать контекст
	/// другого уровня (FR-28). На окне, созданном без GL, переключиться на GL нельзя.
	gl_config: Option<glutin::config::Config>,
	/// Полный экран включён приложением (FR-22).
	fullscreen: bool,
	/// Физический размер растеризации, выведенный из DPI окна (ADR-0005).
	glyph_px: u16,
	/// Последняя позиция указателя для `PointerEvent::Moved` и ячейки под указателем.
	pointer: PhysicalPosition<f64>,
	/// Последнее состояние модификаторов (`WindowEvent::ModifiersChanged`) для [`Ctx::modifiers`].
	modifiers: ModifiersState,
	/// Кадр уже повторяли после ошибки (§5.3 п.5): следующая ошибка — откат.
	frame_retried: bool,
	/// Сколько попыток кадра ещё завершить ошибкой (`--fail-frames`, ADR-0007).
	frames_to_fail: u32,
	/// Ближайшую попытку кадра завершить потерей контекста (`--lose-context`, ADR-0007).
	is_context_loss_pending: bool,
	/// Статистика кадров для приложения (ADR-0006).
	stats: FrameStats,
	/// Наложение текущего кадра (ADR-0014): заполняется в `on_redraw`, очищается перед ним.
	overlay: Overlay,
	/// Ошибка, из-за которой цикл остановлен; возвращается из `run`.
	error: Option<AppError>,
	/// Кадр запрошен самой `App` ([`App::request_redraw`]); `RedrawRequested` без этой пометки —
	/// перерисовка по запросу системы (ADR-0010 п.4).
	is_redraw_requested: bool,
	/// Следующий кадр Software вывести в окно целиком: первый кадр, окно только что показано или
	/// перерисовку запросила система (ADR-0010 п.4).
	is_full_present_pending: bool,
}

impl App {
	pub fn new(
		title: &str,
		screen: ScreenBuffer,
		rasterizer: Rasterizer,
		config: Config,
		options: AppOptions,
	) -> Self {
		let glyph_px = rasterizer.size_px();
		let frames_to_fail = config.fail_frames;
		let is_context_loss_pending = config.lose_context;
		Self {
			title: title.to_owned(),
			screen,
			config,
			options,
			handler: None,
			rasterizer: Some(rasterizer),
			backend: None,
			window: None,
			gl_config: None,
			fullscreen: false,
			glyph_px,
			pointer: PhysicalPosition::new(0.0, 0.0),
			modifiers: ModifiersState::empty(),
			frame_retried: false,
			frames_to_fail,
			is_context_loss_pending,
			stats: FrameStats::default(),
			overlay: Overlay::new(),
			error: None,
			is_redraw_requested: false,
			is_full_present_pending: true,
		}
	}

	/// Запустить цикл событий; `handler` получает все события ввода и уведомления (ADR-0004).
	/// Возвращается после закрытия окна или `ctx.exit()`.
	pub fn run(mut self, handler: impl AppHandler + 'static) -> Result<(), AppError> {
		let event_loop = EventLoop::new().map_err(AppError::EventLoop)?;
		event_loop.set_control_flow(ControlFlow::Wait);
		self.handler = Some(Box::new(handler));
		let result = event_loop.run_app(&mut self).map_err(AppError::EventLoop);
		// Приложение больше не нужно: освобождаем его до возврата, чтобы не держать буфер.
		self.handler = None;
		result?;
		match self.error.take() {
			Some(e) => Err(e),
			None => Ok(()),
		}
	}

	/// Имя активного бэкенда (FR-20).
	pub fn backend_name(&self) -> Option<&'static str> {
		self.backend.as_ref().map(Backend::name)
	}

	/// Уровень цепочки активного бэкенда — тем же сравнением проверяется переключение.
	fn backend_kind(&self) -> Option<RendererKind> {
		self.backend.as_ref().map(Backend::kind)
	}

	/// Физический размер растеризации, выведенный из DPI окна (ADR-0005).
	pub fn glyph_px(&self) -> u16 {
		self.glyph_px
	}

	/// Размер сетки приложения.
	pub fn grid(&self) -> Grid {
		Grid::new(self.screen.cols(), self.screen.rows())
	}
}
