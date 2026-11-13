//! Геометрия окна и DPI (FR-22…FR-24, ADR-0005): атрибуты окна, размер растеризации, смена
//! размера окна и масштаба, заголовок.

use std::num::NonZeroU32;

use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Fullscreen, Window, WindowAttributes};

use super::backend::Backend;
use super::{App, AppError, Grid, GridMode, auto_grid, centered_origin, pointer_cell};
use crate::font::{FONT_SCALE, FontError, glyph_px_for};
use crate::rasterizer::Rasterizer;

impl App {
	/// Размер ячейки в пикселях из метрик активного растеризатора.
	pub(super) fn cell_px(&self) -> (usize, usize) {
		match &self.backend {
			Some(Backend::Gl(gl)) => (gl.atlas().glyph_w(), gl.atlas().glyph_h()),
			Some(Backend::Soft(soft)) => {
				let atlas = soft.renderer.atlas();
				(atlas.glyph_w(), atlas.glyph_h())
			}
			None => (1, 1),
		}
	}

	/// То же для [`Ctx::cell_px`](super::Ctx::cell_px): глиф не больше 48 px, усечение безопасно.
	pub(super) fn cell_px_u32(&self) -> (u32, u32) {
		let (w, h) = self.cell_px();
		(w as u32, h as u32)
	}

	/// Ячейка сетки под последним положением указателя (ADR-0026): сетка центрирована в окне
	/// (§7.2 `u_origin`, [`centered_origin`]), поэтому смещение вычитается до деления.
	pub(super) fn pointer_cell(&self) -> (i32, i32) {
		let cell_px = self.cell_px_u32();
		let grid_px = (
			(self.screen.cols() as u32).saturating_mul(cell_px.0),
			(self.screen.rows() as u32).saturating_mul(cell_px.1),
		);
		let window_px = self
			.window
			.as_ref()
			.map_or(grid_px, |w| (w.inner_size().width, w.inner_size().height));
		pointer_cell(
			(self.pointer.x, self.pointer.y),
			centered_origin(grid_px, window_px),
			cell_px,
		)
	}

	/// Начальный размер окна в **логических** единицах (FR-22, ADR-0005 п.5): при `scale_factor
	/// != 1` физический размер подставит система, иначе на 200 % окно было бы вдвое меньше
	/// задуманного, а потом ещё «прыгнуло» бы при определении DPI.
	///
	/// Логическая ячейка — метрики при `scale_factor = 1`; физическая получится умножением на
	/// DPI, то есть ровно тем же расчётом, что и `glyph_px`.
	pub(super) fn window_attributes(
		&self,
		rasterizer: &Rasterizer,
	) -> Result<WindowAttributes, AppError> {
		let logical_px = glyph_px_for(self.options.font_size_px, FONT_SCALE, 1.0);
		let m = rasterizer.family().metrics_at(logical_px)?;
		let w = (self.screen.cols() as u32)
			.saturating_mul(m.glyph_w as u32)
			.max(1);
		let h = (self.screen.rows() as u32)
			.saturating_mul(m.glyph_h as u32)
			.max(1);
		// Окно создаётся скрытым: иначе система показывает его с белым фоном класса окна до
		// первого кадра. Первый кадр рисуем сами, затем показываем (FR-22).
		Ok(Window::default_attributes()
			.with_title(&self.title)
			.with_visible(false)
			.with_resizable(self.options.resizable)
			.with_fullscreen(self.fullscreen.then_some(Fullscreen::Borderless(None)))
			.with_inner_size(LogicalSize::new(w, h)))
	}

	/// Растеризатор под физический размер окна (ADR-0005 п.7).
	///
	/// DPI известен только **после** создания окна, поэтому при необходимости хинтеры
	/// перестраиваются на месте. Размер окна при этом не меняется: система применит его сама.
	pub(super) fn fit_rasterizer_to_window(
		&mut self,
		window: &Window,
	) -> Result<Rasterizer, AppError> {
		let rasterizer = self
			.rasterizer
			.take()
			.ok_or_else(|| AppError::Font(FontError::TtfParse("rasterizer is consumed once")))?;
		let rasterizer = self.fit_rasterizer(window, rasterizer)?;
		Ok(rasterizer)
	}

	/// То же для растеризатора, уже находящегося в `self.rasterizer`.
	pub(super) fn fit_rasterizer(
		&mut self,
		window: &Window,
		mut rasterizer: Rasterizer,
	) -> Result<Rasterizer, AppError> {
		let scale_factor = window.scale_factor();
		let wanted = glyph_px_for(self.options.font_size_px, FONT_SCALE, scale_factor);
		let was = rasterizer.size_px();
		rasterizer.set_size(wanted)?;
		self.glyph_px = wanted;
		if self.config.debug || wanted != was {
			eprintln!("gterm: dpi {scale_factor} → glyph_px {wanted} (было {was})");
		}
		Ok(rasterizer)
	}

	/// FR-20: имя бэкенда в заголовке при `--debug`.
	pub(super) fn apply_title(&self) {
		if let (true, Some(window), Some(name)) =
			(self.config.debug, &self.window, self.backend_name())
		{
			window.set_title(&format!("{} [{name}]", self.title));
		}
	}

	/// Изменить размер окна: поверхность, кадровый буфер и, в `AutoGrid`, сетку (FR-23).
	pub(super) fn resize_window(
		&mut self,
		size: PhysicalSize<u32>,
		event_loop: &ActiveEventLoop,
	) -> Result<(), AppError> {
		let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
			return Ok(()); // свёрнутое окно: рисовать нечего
		};
		if self.options.grid_mode == GridMode::AutoGrid {
			let wanted = auto_grid((w.get(), h.get()), self.cell_px());
			if (wanted.cols, wanted.rows) != (self.screen.cols(), self.screen.rows()) {
				let old = Grid::new(self.screen.cols(), self.screen.rows());
				self.screen.resize(wanted.cols, wanted.rows);
				let request = self.notify(|handler, ctx| handler.on_grid_resized(old, ctx));
				self.apply_request(&request, event_loop);
			}
		}
		if self.config.debug {
			let scale = self.window.as_ref().map_or(0.0, |w| w.scale_factor());
			eprintln!(
				"gterm: resize: window {}x{} px, scale {scale}, grid {}x{}, cell {:?}",
				w,
				h,
				self.screen.cols(),
				self.screen.rows(),
				self.cell_px()
			);
		}
		if let Some(Backend::Soft(soft)) = &mut self.backend {
			soft.surface.resize(w, h).map_err(AppError::Surface)?;
		}
		let Some(backend) = &mut self.backend else {
			return Ok(());
		};
		backend
			.renderer_mut()
			.resize(size.width, size.height)
			.map_err(AppError::Render)?;
		Ok(())
	}

	/// Провести смену DPI (FR-24, ADR-0005): новый размер растеризации, пересборка глифов и
	/// перезаливка страницы атласа.
	///
	/// Предложенный системой размер окна принимается как есть: иначе окно «прыгает» при переносе
	/// между мониторами. Сетка в `AutoGrid` пересчитывается от новой ячейки: ячейка крупнее —
	/// ячеек в том же окне меньше.
	pub(super) fn apply_scale_factor(
		&mut self,
		event_loop: &ActiveEventLoop,
		scale_factor: f64,
		inner_size_writer: &mut winit::event::InnerSizeWriter,
	) {
		let Some(window) = self.window.clone() else {
			return;
		};
		if let Err(e) = inner_size_writer.request_inner_size(window.inner_size()) {
			eprintln!("gterm: cannot keep window size on DPI change: {e}");
		}
		let wanted = glyph_px_for(self.options.font_size_px, FONT_SCALE, scale_factor);
		if wanted == self.glyph_px {
			return;
		}
		let Some(backend) = &mut self.backend else {
			return;
		};
		if let Err(e) = backend.set_size(wanted) {
			eprintln!("gterm: {e}");
			return;
		}
		let was = self.glyph_px;
		self.glyph_px = wanted;
		eprintln!("gterm: dpi {scale_factor} → glyph_px {wanted} (было {was})");
		if let Err(e) = self.resize_window(window.inner_size(), event_loop) {
			self.stop_with(event_loop, e);
			return;
		}
		self.request_redraw();
	}
}
