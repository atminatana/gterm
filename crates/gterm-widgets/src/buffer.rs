//! Рисование в [`ScreenBuffer`] с обрезкой по прямоугольнику и границам буфера (ADR-0023):
//! виджеты пишут только через эти методы, поэтому за пределы `area` не выходят.

use gterm::buffer::ScreenBuffer;
use gterm::cell::Cell;

use crate::geometry::Rect;
use crate::style::Style;
use crate::text::{Alignment, StyledLine};

/// Расширение `ScreenBuffer` для виджетов.
pub trait BufferExt {
	/// Область всего буфера.
	fn area(&self) -> Rect;

	/// Поставить символ со стилем поверх существующей ячейки; вне `clip` и буфера — ничего.
	fn set_styled(&mut self, x: u16, y: u16, ch: char, style: Style, clip: Rect);

	/// Наложить стиль на все ячейки области (символы остаются).
	fn set_style(&mut self, area: Rect, style: Style);

	/// Заполнить область символом со стилем поверх существующих ячеек.
	fn fill(&mut self, area: Rect, ch: char, style: Style);

	/// Вывести строку слева направо от `(x, y)` внутри `clip`; стиль — поверх `base`.
	/// Возвращает число выведенных символов.
	fn set_string(&mut self, x: u16, y: u16, text: &str, style: Style, clip: Rect) -> u16;

	/// Вывести готовую строку в области `area` (одна строка высотой) с её выравниванием и
	/// горизонтальной прокруткой `skip` символов.
	fn set_line(&mut self, area: Rect, line: &StyledLine, skip: usize);
}

impl BufferExt for ScreenBuffer {
	fn area(&self) -> Rect {
		Rect::of_screen(self)
	}

	fn set_styled(&mut self, x: u16, y: u16, ch: char, style: Style, clip: Rect) {
		let clip = clip.intersection(self.area());
		if !clip.contains(x, y) {
			return;
		}
		let (col, row) = (usize::from(x), usize::from(y));
		let mut cell = self.cell(col, row);
		cell.ch = ch;
		style.apply(&mut cell);
		self.set_cell(col, row, cell.ch, cell.fg, cell.bg, cell.flags);
	}

	fn set_style(&mut self, area: Rect, style: Style) {
		let area = area.intersection(self.area());
		for y in area.top()..area.bottom() {
			for x in area.left()..area.right() {
				let (col, row) = (usize::from(x), usize::from(y));
				let mut cell: Cell = self.cell(col, row);
				style.apply(&mut cell);
				self.set_cell(col, row, cell.ch, cell.fg, cell.bg, cell.flags);
			}
		}
	}

	fn fill(&mut self, area: Rect, ch: char, style: Style) {
		let area = area.intersection(self.area());
		for y in area.top()..area.bottom() {
			for x in area.left()..area.right() {
				self.set_styled(x, y, ch, style, area);
			}
		}
	}

	fn set_string(&mut self, x: u16, y: u16, text: &str, style: Style, clip: Rect) -> u16 {
		let mut count = 0u16;
		for (i, ch) in text.chars().enumerate() {
			let Some(cx) = x.checked_add(i.min(u16::MAX as usize) as u16) else {
				break;
			};
			if cx >= clip.right() {
				break;
			}
			self.set_styled(cx, y, ch, style, clip);
			count += 1;
		}
		count
	}

	fn set_line(&mut self, area: Rect, line: &StyledLine, skip: usize) {
		if area.is_empty() {
			return;
		}
		let width = usize::from(area.width);
		let visible = line.chars.len().saturating_sub(skip);
		let offset = match line.alignment {
			Alignment::Left => 0,
			Alignment::Center => width.saturating_sub(visible) / 2,
			Alignment::Right => width.saturating_sub(visible),
		};
		for (i, &(ch, style)) in line.chars.iter().skip(skip).enumerate() {
			let Some(x) = (usize::from(area.x) + offset + i)
				.try_into()
				.ok()
				.filter(|&x: &u16| x < area.right())
			else {
				break;
			};
			self.set_styled(x, area.y, ch, style, area);
		}
	}
}
