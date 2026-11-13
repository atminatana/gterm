//! Блок: рамка из процедурной псевдографики (ADR-0002, ADR-0022), заголовки, отступы, фон.
//! Внутренняя область для содержимого — [`Block::inner`].

use bitflags::bitflags;
use gterm::buffer::ScreenBuffer;

use crate::buffer::BufferExt;
use crate::geometry::Rect;
use crate::style::Style;
use crate::text::{Alignment, Line, StyledLine};
use crate::widgets::Widget;

bitflags! {
	/// Стороны рамки.
	#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
	pub struct Borders: u8 {
		const TOP    = 0b0001;
		const RIGHT  = 0b0010;
		const BOTTOM = 0b0100;
		const LEFT   = 0b1000;
		const NONE   = 0;
		const ALL    = Self::TOP.bits() | Self::RIGHT.bits() | Self::BOTTOM.bits() | Self::LEFT.bits();
	}
}

/// Вид линий рамки — знаки U+2500–U+257F, стыкующиеся между соседними блоками.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BorderType {
	#[default]
	Plain,
	Rounded,
	Double,
	Thick,
}

/// Набор знаков рамки: горизонталь, вертикаль, четыре угла.
struct BorderSet {
	horizontal: char,
	vertical: char,
	top_left: char,
	top_right: char,
	bottom_left: char,
	bottom_right: char,
}

impl BorderType {
	fn set(self) -> BorderSet {
		match self {
			BorderType::Plain => BorderSet {
				horizontal: '─',
				vertical: '│',
				top_left: '┌',
				top_right: '┐',
				bottom_left: '└',
				bottom_right: '┘',
			},
			BorderType::Rounded => BorderSet {
				horizontal: '─',
				vertical: '│',
				top_left: '╭',
				top_right: '╮',
				bottom_left: '╰',
				bottom_right: '╯',
			},
			BorderType::Double => BorderSet {
				horizontal: '═',
				vertical: '║',
				top_left: '╔',
				top_right: '╗',
				bottom_left: '╚',
				bottom_right: '╝',
			},
			BorderType::Thick => BorderSet {
				horizontal: '━',
				vertical: '┃',
				top_left: '┏',
				top_right: '┓',
				bottom_left: '┗',
				bottom_right: '┛',
			},
		}
	}
}

/// Отступы между рамкой и содержимым.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Padding {
	pub left: u16,
	pub right: u16,
	pub top: u16,
	pub bottom: u16,
}

impl Padding {
	pub const fn new(left: u16, right: u16, top: u16, bottom: u16) -> Self {
		Self {
			left,
			right,
			top,
			bottom,
		}
	}

	pub const fn uniform(n: u16) -> Self {
		Self::new(n, n, n, n)
	}

	pub const fn horizontal(n: u16) -> Self {
		Self::new(n, n, 0, 0)
	}

	pub const fn vertical(n: u16) -> Self {
		Self::new(0, 0, n, n)
	}
}

#[derive(Clone, Debug, Default)]
pub struct Block<'a> {
	titles_top: Vec<Line<'a>>,
	titles_bottom: Vec<Line<'a>>,
	title_alignment: Alignment,
	borders: Borders,
	border_type: BorderType,
	border_style: Style,
	style: Style,
	padding: Padding,
}

impl<'a> Block<'a> {
	pub fn new() -> Self {
		Self::default()
	}

	/// Блок с рамкой со всех сторон.
	pub fn bordered() -> Self {
		Self::new().borders(Borders::ALL)
	}

	/// Заголовок в верхней рамке (несколько — через пробел, по выравниванию блока или строки).
	pub fn title<T: Into<Line<'a>>>(mut self, title: T) -> Self {
		self.titles_top.push(title.into());
		self
	}

	pub fn title_bottom<T: Into<Line<'a>>>(mut self, title: T) -> Self {
		self.titles_bottom.push(title.into());
		self
	}

	pub fn title_alignment(mut self, alignment: Alignment) -> Self {
		self.title_alignment = alignment;
		self
	}

	pub fn borders(mut self, borders: Borders) -> Self {
		self.borders = borders;
		self
	}

	pub fn border_type(mut self, border_type: BorderType) -> Self {
		self.border_type = border_type;
		self
	}

	pub fn border_style(mut self, style: Style) -> Self {
		self.border_style = style;
		self
	}

	/// Наложить стиль на стиль рамки (подсветка фокуса у контейнера дерева, ADR-0026).
	pub(crate) fn patch_border_style(&mut self, style: Style) {
		self.border_style = self.border_style.patch(style);
	}

	/// Стиль всей области блока (фон под содержимым).
	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	pub fn padding(mut self, padding: Padding) -> Self {
		self.padding = padding;
		self
	}

	/// Область для содержимого: без рамки и отступов; при нехватке места — пустая.
	pub fn inner(&self, area: Rect) -> Rect {
		let mut inner = area;
		let shrink_left = u16::from(self.borders.contains(Borders::LEFT)) + self.padding.left;
		let shrink_right = u16::from(self.borders.contains(Borders::RIGHT)) + self.padding.right;
		let shrink_top =
			u16::from(self.borders.contains(Borders::TOP) || !self.titles_top.is_empty())
				+ self.padding.top;
		let shrink_bottom =
			u16::from(self.borders.contains(Borders::BOTTOM) || !self.titles_bottom.is_empty())
				+ self.padding.bottom;
		if inner.width <= shrink_left + shrink_right || inner.height <= shrink_top + shrink_bottom {
			return Rect::default();
		}
		inner.x += shrink_left;
		inner.width -= shrink_left + shrink_right;
		inner.y += shrink_top;
		inner.height -= shrink_top + shrink_bottom;
		inner
	}

	fn render_borders(&self, area: Rect, buf: &mut ScreenBuffer) {
		let set = self.border_type.set();
		let style = self.border_style;
		let (left, top) = (area.left(), area.top());
		let (right, bottom) = (area.right() - 1, area.bottom() - 1);
		if self.borders.contains(Borders::LEFT) {
			for y in top..=bottom {
				buf.set_styled(left, y, set.vertical, style, area);
			}
		}
		if self.borders.contains(Borders::RIGHT) {
			for y in top..=bottom {
				buf.set_styled(right, y, set.vertical, style, area);
			}
		}
		if self.borders.contains(Borders::TOP) {
			for x in left..=right {
				buf.set_styled(x, top, set.horizontal, style, area);
			}
		}
		if self.borders.contains(Borders::BOTTOM) {
			for x in left..=right {
				buf.set_styled(x, bottom, set.horizontal, style, area);
			}
		}
		if self.borders.contains(Borders::TOP | Borders::LEFT) {
			buf.set_styled(left, top, set.top_left, style, area);
		}
		if self.borders.contains(Borders::TOP | Borders::RIGHT) {
			buf.set_styled(right, top, set.top_right, style, area);
		}
		if self.borders.contains(Borders::BOTTOM | Borders::LEFT) {
			buf.set_styled(left, bottom, set.bottom_left, style, area);
		}
		if self.borders.contains(Borders::BOTTOM | Borders::RIGHT) {
			buf.set_styled(right, bottom, set.bottom_right, style, area);
		}
	}

	/// Заголовки в строке `y`: между левой и правой рамками, по выравниванию каждого заголовка
	/// (или блока), несколько заголовков одного выравнивания — через пробел.
	fn render_titles(&self, titles: &[Line<'a>], y: u16, area: Rect, buf: &mut ScreenBuffer) {
		let margin_left = u16::from(self.borders.contains(Borders::LEFT));
		let margin_right = u16::from(self.borders.contains(Borders::RIGHT));
		if area.width <= margin_left + margin_right {
			return;
		}
		let row = Rect::new(
			area.x + margin_left,
			y,
			area.width - margin_left - margin_right,
			1,
		);
		for alignment in [Alignment::Left, Alignment::Center, Alignment::Right] {
			let mut chars = Vec::new();
			for title in titles
				.iter()
				.filter(|t| t.alignment.unwrap_or(self.title_alignment) == alignment)
			{
				if !chars.is_empty() {
					chars.push((' ', self.style.patch(self.border_style)));
				}
				chars.extend(title.styled_chars(self.style.patch(self.border_style)));
			}
			if chars.is_empty() {
				continue;
			}
			chars.truncate(usize::from(row.width));
			buf.set_line(row, &StyledLine { chars, alignment }, 0);
		}
	}
}

impl Widget for Block<'_> {
	fn render(self, area: Rect, buf: &mut ScreenBuffer) {
		let area = area.intersection(buf.area());
		if area.is_empty() {
			return;
		}
		buf.set_style(area, self.style);
		self.render_borders(area, buf);
		if !self.titles_top.is_empty() {
			self.render_titles(&self.titles_top, area.top(), area, buf);
		}
		if !self.titles_bottom.is_empty() {
			self.render_titles(&self.titles_bottom, area.bottom() - 1, area, buf);
		}
	}
}
