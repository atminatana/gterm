//! Параметры окна и цикла, сетка и ошибки `App`: [`AppOptions`], [`Grid`], [`GridMode`],
//! [`AppError`], чистые [`auto_grid`] и [`centered_origin`].

use std::fmt;

use crate::font::{FONT_SIZE_PX, FontError};
use crate::renderer::RenderError;

/// Ошибки окна и цикла событий; все ведут к завершению с сообщением и кодом ≠ 0 (§8, NFR-7).
#[derive(Debug)]
pub enum AppError {
	/// Не создан цикл событий или он завершился ошибкой.
	EventLoop(winit::error::EventLoopError),
	/// Не создано окно.
	Window(winit::error::OsError),
	/// `softbuffer` не смог создать или обновить поверхность.
	Surface(softbuffer::SoftBufferError),
	/// Ошибка рендерера, после которой уровней для отката не осталось.
	Render(RenderError),
	/// Шрифт не подошёл для запрошенного размера растеризации.
	Font(FontError),
}

impl fmt::Display for AppError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::EventLoop(e) => write!(f, "event loop: {e}"),
			Self::Window(e) => write!(f, "window: {e}"),
			Self::Surface(e) => write!(f, "softbuffer surface: {e}"),
			Self::Render(e) => write!(f, "{e}"),
			Self::Font(e) => write!(f, "font: {e}"),
		}
	}
}

impl std::error::Error for AppError {}

impl From<RenderError> for AppError {
	fn from(e: RenderError) -> Self {
		Self::Render(e)
	}
}

impl From<FontError> for AppError {
	fn from(e: FontError) -> Self {
		Self::Font(e)
	}
}

/// Размер сетки в ячейках.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grid {
	/// Ячеек по горизонтали.
	pub cols: usize,
	/// Ячеек по вертикали.
	pub rows: usize,
}

impl Grid {
	pub const fn new(cols: usize, rows: usize) -> Self {
		Self { cols, rows }
	}
}

/// Как сетка следует за размером окна (FR-23). Выбирается один раз при создании [`AppOptions`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GridMode {
	/// Сетка пересчитывается по окну; размер шрифта под неё не подбирается (FR-28, нет зума).
	/// Запрос сетки от приложения в этом режиме не действует (см. [`Request::grid`](super::Request::grid)).
	#[default]
	AutoGrid,
	/// Сетка постоянна; не помещающаяся в окно часть центрируется и обрезается по границе окна.
	/// Сетку задаёт приложение через [`Request::grid`](super::Request::grid).
	FixedGrid,
}

/// Параметры окна и цикла (FR-22, FR-23). Всё, что меняется по ходу работы, приложение
/// меняет через [`Request`](super::Request), а не пересоздаёт окно.
#[derive(Clone, Copy, Debug)]
pub struct AppOptions {
	/// Как сетка следует за размером окна.
	pub grid_mode: GridMode,
	/// Разрешить пользователю менять размер окна.
	pub resizable: bool,
	/// Кадровая синхронизация на GL-бэкендах. `false` — только для замера `--bench` (ADR-0006):
	/// иначе ожидание монитора попадает в CPU-часть следующего кадра и прячет её настоящую цену.
	pub vsync: bool,
	/// Логический размер шрифта, px при `scale_factor = 1` (FR-8, ADR-0008 п.8); физический —
	/// `glyph_px_for(font_size_px, FONT_SCALE, scale_factor)`.
	pub font_size_px: u16,
}

impl Default for AppOptions {
	fn default() -> Self {
		Self {
			grid_mode: GridMode::AutoGrid,
			resizable: true,
			vsync: true,
			font_size_px: FONT_SIZE_PX,
		}
	}
}

/// Число ячеек, помещающихся в окно, но не меньше 1×1 (FR-23 `AutoGrid`).
///
/// Чистая функция: размер ячейки приходит из метрик растеризации, масштаба нет (ADR-0005).
pub fn auto_grid(window_px: (u32, u32), cell_px: (usize, usize)) -> Grid {
	let cell_w = (cell_px.0.max(1)) as u32;
	let cell_h = (cell_px.1.max(1)) as u32;
	Grid {
		cols: (window_px.0 / cell_w).max(1) as usize,
		rows: (window_px.1 / cell_h).max(1) as usize,
	}
}

/// Смещение сетки в окне при центрировании (§7.3): не меньше нуля, поэтому сетка больше окна
/// обрезается по границе окна, а не уезжает за него (FR-23 `FixedGrid`).
pub fn centered_origin(grid_px: (u32, u32), window_px: (u32, u32)) -> (u32, u32) {
	(
		window_px.0.saturating_sub(grid_px.0) / 2,
		window_px.1.saturating_sub(grid_px.1) / 2,
	)
}

/// Ячейка сетки под точкой окна (ADR-0026): физические координаты минус смещение сетки
/// ([`centered_origin`]), делённые на размер ячейки с округлением вниз. Слева и сверху от сетки
/// получаются отрицательные значения, справа и снизу — не меньше размера сетки: «вне сетки»
/// распознаётся сравнением с ней. Негодные координаты (`NaN`, бесконечность) дают `(-1, -1)`.
pub fn pointer_cell(
	position: (f64, f64),
	origin_px: (u32, u32),
	cell_px: (u32, u32),
) -> (i32, i32) {
	fn axis(position: f64, origin: u32, cell: u32) -> i32 {
		let shifted = (position - f64::from(origin)) / f64::from(cell.max(1));
		if !shifted.is_finite() {
			return -1;
		}
		// Усечение `as` насыщает на границах `i32`, поэтому ячейка далеко за окном остаётся
		// просто «вне сетки».
		shifted.floor() as i32
	}
	(
		axis(position.0, origin_px.0, cell_px.0),
		axis(position.1, origin_px.1, cell_px.1),
	)
}
