//! Очистка области (ADR-0023, часть 2): сбрасывает ячейки к символу и цветам по умолчанию
//! буфера. Ставится перед виджетом, который рисуется поверх уже нарисованного, — например, перед
//! окном в том же буфере.

use gterm::buffer::ScreenBuffer;

use crate::buffer::BufferExt;
use crate::geometry::Rect;
use crate::widgets::Widget;

/// Сбросить область: символы, цвета и флаги — как в пустом буфере.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Clear;

impl Widget for Clear {
	fn render(self, area: Rect, buf: &mut ScreenBuffer) {
		let area = area.intersection(buf.area());
		if area.is_empty() {
			return;
		}
		buf.clear_region(
			usize::from(area.x),
			usize::from(area.y),
			usize::from(area.width),
			usize::from(area.height),
		);
	}
}
