//! Прямоугольник на сетке ячеек: `u16`, как у ratatui; пустой прямоугольник ничего не рисует.

use gterm::buffer::ScreenBuffer;

/// Прямоугольник `[x, x + width) × [y, y + height)` в ячейках сетки.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
	pub x: u16,
	pub y: u16,
	pub width: u16,
	pub height: u16,
}

/// Отступы внутрь прямоугольника по сторонам.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Margin {
	pub horizontal: u16,
	pub vertical: u16,
}

impl Rect {
	pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
		Self {
			x,
			y,
			width,
			height,
		}
	}

	/// Весь экран буфера.
	pub fn of_screen(screen: &ScreenBuffer) -> Self {
		Self::new(
			0,
			0,
			screen.cols().min(u16::MAX as usize) as u16,
			screen.rows().min(u16::MAX as usize) as u16,
		)
	}

	pub const fn area(self) -> u32 {
		self.width as u32 * self.height as u32
	}

	pub const fn is_empty(self) -> bool {
		self.width == 0 || self.height == 0
	}

	pub const fn left(self) -> u16 {
		self.x
	}

	/// Первая колонка справа за прямоугольником.
	pub const fn right(self) -> u16 {
		self.x.saturating_add(self.width)
	}

	pub const fn top(self) -> u16 {
		self.y
	}

	/// Первая строка под прямоугольником.
	pub const fn bottom(self) -> u16 {
		self.y.saturating_add(self.height)
	}

	/// Прямоугольник с отступами внутрь; при нехватке места — пустой.
	pub fn inner(self, margin: Margin) -> Self {
		let dx = margin.horizontal.saturating_mul(2);
		let dy = margin.vertical.saturating_mul(2);
		if self.width <= dx || self.height <= dy {
			return Self::default();
		}
		Self::new(
			self.x + margin.horizontal,
			self.y + margin.vertical,
			self.width - dx,
			self.height - dy,
		)
	}

	/// Пересечение; при отсутствии — пустой прямоугольник.
	pub fn intersection(self, other: Self) -> Self {
		let x1 = self.x.max(other.x);
		let y1 = self.y.max(other.y);
		let x2 = self.right().min(other.right());
		let y2 = self.bottom().min(other.bottom());
		if x2 <= x1 || y2 <= y1 {
			return Self::default();
		}
		Self::new(x1, y1, x2 - x1, y2 - y1)
	}

	pub const fn contains(self, x: u16, y: u16) -> bool {
		x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
	}

	/// Прямоугольник `width × height`, отцентрованный в `self` (обрезанный по нему).
	pub fn centered(self, width: u16, height: u16) -> Self {
		let width = width.min(self.width);
		let height = height.min(self.height);
		Self::new(
			self.x + (self.width - width) / 2,
			self.y + (self.height - height) / 2,
			width,
			height,
		)
	}
}
