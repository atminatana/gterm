//! Стиль — частичное изменение ячейки: цвет символа и фона (с альфой, ADR-0013), добавляемые и
//! снимаемые флаги. `None` — оставить как есть; `patch` накладывает один стиль на другой.

use gterm::cell::{Cell, CellFlags, Color};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
	pub fg: Option<Color>,
	pub bg: Option<Color>,
	/// Флаги, которые стиль включает.
	pub add: CellFlags,
	/// Флаги, которые стиль снимает (сильнее `add`).
	pub remove: CellFlags,
}

impl Style {
	pub const fn new() -> Self {
		Self {
			fg: None,
			bg: None,
			add: CellFlags::empty(),
			remove: CellFlags::empty(),
		}
	}

	pub const fn fg(mut self, color: Color) -> Self {
		self.fg = Some(color);
		self
	}

	pub const fn bg(mut self, color: Color) -> Self {
		self.bg = Some(color);
		self
	}

	pub const fn add(mut self, flags: CellFlags) -> Self {
		self.add = self.add.union(flags);
		self.remove = self.remove.difference(flags);
		self
	}

	pub const fn remove(mut self, flags: CellFlags) -> Self {
		self.remove = self.remove.union(flags);
		self.add = self.add.difference(flags);
		self
	}

	pub const fn bold(self) -> Self {
		self.add(CellFlags::BOLD)
	}

	pub const fn italic(self) -> Self {
		self.add(CellFlags::ITALIC)
	}

	pub const fn underlined(self) -> Self {
		self.add(CellFlags::UNDERLINE)
	}

	pub const fn not_bold(self) -> Self {
		self.remove(CellFlags::BOLD)
	}

	/// Наложить `other` поверх: заданные в нём цвета и флаги побеждают.
	pub const fn patch(mut self, other: Style) -> Self {
		if other.fg.is_some() {
			self.fg = other.fg;
		}
		if other.bg.is_some() {
			self.bg = other.bg;
		}
		self.add = self.add.difference(other.remove).union(other.add);
		self.remove = self.remove.difference(other.add).union(other.remove);
		self
	}

	/// Применить к ячейке: незаданное остаётся прежним.
	pub fn apply(&self, cell: &mut Cell) {
		if let Some(fg) = self.fg {
			cell.fg = fg;
		}
		if let Some(bg) = self.bg {
			cell.bg = bg;
		}
		cell.flags = (cell.flags | self.add) - self.remove;
	}
}
