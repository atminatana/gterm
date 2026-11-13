//! Абзац: многострочный текст с переносом по словам, выравниванием и прокруткой, внутри
//! необязательного [`Block`].

use gterm::buffer::ScreenBuffer;

use crate::buffer::BufferExt;
use crate::geometry::Rect;
use crate::style::Style;
use crate::text::{Alignment, StyledLine, Text, wrap_line};
use crate::widgets::{Block, Widget};

/// Перенос строк по ширине области; `trim` — убирать пробелы по краям получившихся строк.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wrap {
	pub trim: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Paragraph<'a> {
	text: Text<'a>,
	block: Option<Block<'a>>,
	style: Style,
	wrap: Option<Wrap>,
	alignment: Alignment,
	/// Прокрутка: строк сверху, символов слева (горизонтальная — только без переноса).
	scroll: (u16, u16),
}

impl<'a> Paragraph<'a> {
	pub fn new<T: Into<Text<'a>>>(text: T) -> Self {
		Self {
			text: text.into(),
			..Self::default()
		}
	}

	pub fn block(mut self, block: Block<'a>) -> Self {
		self.block = Some(block);
		self
	}

	/// Стиль всей области абзаца; стили текста накладываются поверх.
	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	pub fn wrap(mut self, wrap: Wrap) -> Self {
		self.wrap = Some(wrap);
		self
	}

	pub fn alignment(mut self, alignment: Alignment) -> Self {
		self.alignment = alignment;
		self
	}

	pub fn left_aligned(self) -> Self {
		self.alignment(Alignment::Left)
	}

	pub fn centered(self) -> Self {
		self.alignment(Alignment::Center)
	}

	pub fn right_aligned(self) -> Self {
		self.alignment(Alignment::Right)
	}

	/// `(строк сверху, символов слева)`.
	pub fn scroll(mut self, offset: (u16, u16)) -> Self {
		self.scroll = offset;
		self
	}

	/// Строки к выводу при ширине `width`: с переносом или без.
	fn lines(&self, width: u16) -> Vec<StyledLine> {
		let base = self.style.patch(self.text.style);
		let mut out = Vec::new();
		for line in &self.text.lines {
			let alignment = line
				.alignment
				.or(self.text.alignment)
				.unwrap_or(self.alignment);
			let chars: Vec<_> = line.styled_chars(base).collect();
			match self.wrap {
				Some(wrap) => {
					for piece in wrap_line(&chars, usize::from(width), wrap.trim) {
						out.push(StyledLine {
							chars: piece,
							alignment,
						});
					}
				}
				None => out.push(StyledLine { chars, alignment }),
			}
		}
		out
	}

	/// Сколько строк займёт текст при ширине `width` (с учётом переноса) — для прокрутки.
	pub fn line_count(&self, width: u16) -> usize {
		self.lines(width).len()
	}
}

impl Widget for Paragraph<'_> {
	fn render(self, area: Rect, buf: &mut ScreenBuffer) {
		let area = area.intersection(buf.area());
		if area.is_empty() {
			return;
		}
		buf.set_style(area, self.style);
		let inner = match &self.block {
			Some(block) => {
				let inner = block.inner(area);
				block.clone().render(area, buf);
				inner
			}
			None => area,
		};
		if inner.is_empty() {
			return;
		}
		let skip_rows = usize::from(self.scroll.0);
		let skip_cols = if self.wrap.is_some() {
			0
		} else {
			usize::from(self.scroll.1)
		};
		for (i, line) in self
			.lines(inner.width)
			.iter()
			.skip(skip_rows)
			.take(usize::from(inner.height))
			.enumerate()
		{
			let row = Rect::new(inner.x, inner.y + i as u16, inner.width, 1);
			buf.set_line(row, line, skip_cols);
		}
	}
}
