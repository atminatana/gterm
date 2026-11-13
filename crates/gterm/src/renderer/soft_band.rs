//! Отрисовка полос строк сетки Software-рендерера (ADR-0016). Одни и те же функции рисуют весь
//! кадр в текущем потоке и непересекающиеся полосы строк в пуле потоков. Полоса — срез кадра или
//! текстуры наложения с первой пиксельной строки своей первой строки сетки. Всё остальное только
//! читается, поэтому полосы независимы и кадр от числа потоков не зависит.

use crate::atlas::{Atlas, EMPTY_SLOT};
use crate::buffer::ScreenBuffer;
use crate::cell::{Cell, CellFlags};
use crate::overlay::{Layer, Overlay};
use crate::renderer::pixel::{COVERAGE_FULL, blend, composite, encode_color, fade, lighten, over};

/// Основной слой кадра (§7.3) и наложение на него готовой текстуры (ADR-0014).
pub(super) struct MainPass<'a> {
	pub atlas: &'a Atlas,
	pub screen: &'a ScreenBuffer,
	/// Строки сетки, которые перерисовываются.
	pub rows_to_draw: &'a [bool],
	/// Строки сетки под наложением: на них ложится `texture`.
	pub overlay_rows: &'a [bool],
	/// Текстура наложения шириной `grid_w`, RGB предумножен.
	pub texture: &'a [u32],
	pub grid_w: usize,
	/// Ширина кадра в пикселях.
	pub width: usize,
	/// Смещение сетки по горизонтали (§7.2 `u_origin`); вертикальное задаёт начало полосы.
	pub origin_x: usize,
}

impl MainPass<'_> {
	/// Нарисовать строки сетки с `first_row` в `band`. Всё, что за концом полосы или правее
	/// кадра, обрезается (сетка может не влезать в окно, FR-23 `FixedGrid`).
	pub fn draw_band(&self, first_row: usize, band: &mut [u32]) {
		let cell_h = self.atlas.glyph_h();
		let band_rows = band.len() / self.width;
		let under = encode_color(self.screen.background());
		let last_row = (first_row + band_rows.div_ceil(cell_h)).min(self.screen.rows());
		for row in first_row..last_row {
			let y0 = (row - first_row) * cell_h;
			if self.rows_to_draw[row] {
				for col in 0..self.screen.cols() {
					self.draw_cell(band, col, y0, self.screen.cell(col, row), under);
				}
			}
			if self.overlay_rows[row] {
				self.composite_row(band, row, y0);
			}
		}
	}

	/// Ячейка по алгоритму §7.3 (ADR-0013): фон ячейки поверх подложки `under` по своей альфе,
	/// затем символ поверх фона по покрытию, умноженному на альфу символа.
	///
	/// Подложка однотонная (фон буфера), поэтому фон ячейки смешивается с ней один раз на
	/// ячейку, а не на пиксель: на пиксель остаётся одно смешивание, как без альфы.
	fn draw_cell(&self, band: &mut [u32], col: usize, y0: usize, cell: Cell, under: u32) {
		let mut fg = encode_color(cell.fg);
		let bg = blend(under, encode_color(cell.bg), cell.bg.a());
		if cell.flags.contains(CellFlags::BOLD) {
			fg = lighten(fg);
		}
		let fg_alpha = cell.fg.a();
		let underline = cell.flags.contains(CellFlags::UNDERLINE);
		let style = cell.style();
		// Глифы подготовлены в `prepare_glyphs`; здесь только чтение (горячий путь).
		let slot = self.atlas.slot_of(cell.ch).unwrap_or(EMPTY_SLOT);
		let (glyph_w, glyph_h) = (self.atlas.glyph_w(), self.atlas.glyph_h());
		let band_rows = band.len() / self.width;
		let cell_x = self.origin_x + col * glyph_w;
		for gy in 0..glyph_h {
			let y = y0 + gy;
			if y >= band_rows {
				return;
			}
			let line = &mut band[y * self.width..(y + 1) * self.width];
			for gx in 0..glyph_w {
				let x = cell_x + gx;
				if x >= self.width {
					break;
				}
				let mut a = self.atlas.pixel(slot, gx, gy, style);
				if underline && gy == glyph_h - 1 {
					a = COVERAGE_FULL;
				}
				line[x] = blend(bg, fg, fade(a, fg_alpha));
			}
		}
	}

	/// Наложить строку сетки `row` текстуры на полосу с обрезкой по кадру.
	fn composite_row(&self, band: &mut [u32], row: usize, y0: usize) {
		let cell_h = self.atlas.glyph_h();
		let band_rows = band.len() / self.width;
		let visible = self.grid_w.min(self.width.saturating_sub(self.origin_x));
		for dy in 0..cell_h {
			let y = y0 + dy;
			if y >= band_rows {
				return;
			}
			let src = (row * cell_h + dy) * self.grid_w;
			let dst = y * self.width + self.origin_x;
			for x in 0..visible {
				let t = self.texture[src + x];
				if t != 0 {
					band[dst + x] = composite(band[dst + x], t);
				}
			}
		}
	}
}

/// Сборка текстуры наложения (ADR-0014): записи по порядку, каждая ячейка — фон, затем символ.
pub(super) struct OverlayPass<'a> {
	pub atlas: &'a Atlas,
	pub overlay: &'a Overlay,
	pub grid_cols: usize,
	pub grid_rows: usize,
	/// Строки под наложением в этом и прошлом кадре: их часть текстуры очищается перед записью.
	pub overlay_rows: &'a [bool],
	pub prev_overlay_rows: &'a [bool],
	/// Текстура только что очищена целиком — построчная очистка не нужна.
	pub is_cleared: bool,
}

impl OverlayPass<'_> {
	/// Собрать строки сетки с `first_row` в полосу текстуры `band` (целое число строк сетки).
	pub fn build_band(&self, first_row: usize, band: &mut [u32]) {
		let row_len = self.atlas.glyph_h() * self.grid_cols * self.atlas.glyph_w();
		let band_rows = first_row..first_row + band.len() / row_len;
		if !self.is_cleared {
			for row in band_rows.clone() {
				let before = self.prev_overlay_rows.get(row).copied().unwrap_or(false);
				if before || self.overlay_rows[row] {
					let local = (row - first_row) * row_len;
					band[local..local + row_len].fill(0);
				}
			}
		}
		for layer in self.overlay.layers() {
			let rows = layer.grid_rows(self.grid_rows);
			for row in rows.start.max(band_rows.start)..rows.end.min(band_rows.end) {
				for col in layer.grid_cols(self.grid_cols) {
					let cell = layer_cell(&layer, col, row);
					self.draw_cell(band, col, row - first_row, cell);
				}
			}
		}
	}

	/// Ячейка записи в текстуру: фон, затем символ — оба `over` (ADR-0014 п.3).
	fn draw_cell(&self, band: &mut [u32], col: usize, local_row: usize, cell: Cell) {
		if cell.bg.a() == 0 && cell.fg.a() == 0 {
			return;
		}
		let mut fg = encode_color(cell.fg);
		if cell.flags.contains(CellFlags::BOLD) {
			fg = lighten(fg);
		}
		let bg = encode_color(cell.bg);
		let (fg_alpha, bg_alpha) = (cell.fg.a(), cell.bg.a());
		let underline = cell.flags.contains(CellFlags::UNDERLINE);
		let style = cell.style();
		let slot = self.atlas.slot_of(cell.ch).unwrap_or(EMPTY_SLOT);
		let (glyph_w, glyph_h) = (self.atlas.glyph_w(), self.atlas.glyph_h());
		let width = self.grid_cols * glyph_w;
		let x0 = col * glyph_w;
		let y0 = local_row * glyph_h;
		// Фон ячейки поверх пустого пикселя одинаков для всей ячейки — частый случай первой записи.
		let bg_over_empty = over(0, bg, bg_alpha);
		for gy in 0..glyph_h {
			for gx in 0..glyph_w {
				let mut a = self.atlas.pixel(slot, gx, gy, style);
				if underline && gy == glyph_h - 1 {
					a = COVERAGE_FULL;
				}
				let at = (y0 + gy) * width + x0 + gx;
				let under = band[at];
				let t = if under == 0 {
					bg_over_empty
				} else {
					over(under, bg, bg_alpha)
				};
				// Нулевая доля символа `t` не меняет (`over` с `α = 0`).
				let a = fade(a, fg_alpha);
				band[at] = if a == 0 { t } else { over(t, fg, a) };
			}
		}
	}
}

/// Ячейка записи под ячейкой `(col, row)` основной сетки; координаты — внутри видимой части.
pub(super) fn layer_cell(layer: &Layer<'_>, col: usize, row: usize) -> Cell {
	let local_col = (col as i64 - i64::from(layer.col)) as usize;
	let local_row = (row as i64 - i64::from(layer.row)) as usize;
	layer.cell(local_col, local_row)
}
