//! Процедурная псевдографика — диспетчер (ADR-0002, ADR-0022): рамки и блоки
//! [`crate::box_drawing`], мозаики [`crate::block_mosaic`], фигуры [`crate::shapes`]. В режиме
//! `Procedural` все они перекрывают глифы шрифта, в `Combined` служат запасным путём (ADR-0018).

use crate::{block_mosaic, box_drawing, shapes};

/// Рисуется ли кодовая точка процедурно.
pub fn is_procedural(ch: char) -> bool {
	box_drawing::is_box_drawing(ch) || block_mosaic::is_mosaic(ch) || shapes::is_shape(ch)
}

/// Все процедурные диапазоны (включительно), по модулям — для перебора в примерах и тестах.
pub fn ranges() -> impl Iterator<Item = (char, char)> {
	std::iter::once((box_drawing::BOX_FIRST, box_drawing::BOX_LAST))
		.chain(block_mosaic::RANGES.iter().copied())
		.chain(shapes::RANGES.iter().copied())
}

/// Нарисовать знак в буфер покрытия `w × h` (row-major). `false` — не процедурный знак или
/// негодный буфер, буфер не тронут.
pub fn draw(ch: char, w: usize, h: usize, out: &mut [u8]) -> bool {
	if box_drawing::is_box_drawing(ch) {
		return box_drawing::draw(ch, w, h, out);
	}
	if block_mosaic::is_mosaic(ch) {
		return block_mosaic::draw(ch, w, h, out);
	}
	shapes::draw(ch, w, h, out)
}

/// Холст одной ячейки: буфер покрытия `w × h`, общий для всех процедурных модулей.
pub(crate) struct Canvas<'a> {
	pub(crate) w: usize,
	pub(crate) h: usize,
	pub(crate) out: &'a mut [u8],
}

impl<'a> Canvas<'a> {
	/// Обнулённый холст поверх `out`; `None` — буфер мал или размер нулевой.
	pub(crate) fn cleared(w: usize, h: usize, out: &'a mut [u8]) -> Option<Self> {
		if out.len() < w * h || w == 0 || h == 0 {
			return None;
		}
		out[..w * h].fill(0);
		Some(Self { w, h, out })
	}

	/// Прямоугольник `[x0, x1) × [y0, y1)` с обрезкой по ячейке; координаты могут выходить за края.
	pub(crate) fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, value: u8) {
		let x0 = (x0.max(0) as usize).min(self.w);
		let y0 = (y0.max(0) as usize).min(self.h);
		let x1 = (x1.max(0) as usize).min(self.w);
		let y1 = (y1.max(0) as usize).min(self.h);
		for y in y0..y1 {
			self.out[y * self.w + x0.min(x1)..y * self.w + x1].fill(value);
		}
	}

	pub(crate) fn set(&mut self, x: i32, y: i32, value: u8) {
		if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
			self.out[y as usize * self.w + x as usize] = value;
		}
	}
}

/// Входит ли кодовая точка в один из диапазонов (включительно).
pub(crate) fn in_ranges(ch: char, ranges: &[(char, char)]) -> bool {
	ranges
		.iter()
		.any(|&(first, last)| (first..=last).contains(&ch))
}
