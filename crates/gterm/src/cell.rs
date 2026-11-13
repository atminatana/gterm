/// Цвет в формате 0xAARRGGBB (FR-2, ADR-0013): `A = 255` — непрозрачный, `A = 0` — прозрачный.
/// `Color::default()` — прозрачный чёрный: такая ячейка показывает то, что под ней.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Color(pub u32);

impl Color {
	/// Непрозрачный цвет из RGB компонентов.
	pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
		Self::from_rgba(r, g, b, 0xFF)
	}

	/// Цвет из RGBA компонентов; `a` — непрозрачность 0…255.
	pub const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
		Self((a as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | b as u32)
	}

	/// Полностью прозрачный цвет.
	pub const TRANSPARENT: Self = Self(0);
	/// Чёрный цвет.
	pub const BLACK: Self = Self(0xFF00_0000);
	/// Белый цвет.
	pub const WHITE: Self = Self(0xFFFF_FFFF);
	/// Зелёный цвет.
	pub const GREEN: Self = Self(0xFF00_FF00);
	/// Пурпурный цвет.
	pub const MAGENTA: Self = Self(0xFFFF_00FF);
	/// Циановый цвет.
	pub const CYAN: Self = Self(0xFF00_FFFF);

	/// Непрозрачность (0-255).
	pub const fn a(&self) -> u8 {
		(self.0 >> 24) as u8
	}

	/// Красный канал (0-255).
	pub const fn r(&self) -> u8 {
		(self.0 >> 16) as u8
	}

	/// Зелёный канал (0-255).
	pub const fn g(&self) -> u8 {
		(self.0 >> 8) as u8
	}

	/// Синий канал (0-255).
	pub const fn b(&self) -> u8 {
		self.0 as u8
	}
}

bitflags::bitflags! {
	/// Флаги ячейки терминала (FR-3).
	#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
	pub struct CellFlags: u16 {
		/// Жирный глиф + осветление цвета символа (ADR-0003).
		const BOLD      = 1 << 0;
		/// Курсивный глиф (ADR-0003).
		const ITALIC    = 1 << 1;
		const UNDERLINE = 1 << 2;
	}
}

/// Начертание глифа — канал атласа (ADR-0003): R / G / B / A.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Style {
	#[default]
	Regular = 0,
	Bold = 1,
	Italic = 2,
	BoldItalic = 3,
}

impl Style {
	/// Все начертания в порядке каналов.
	pub const ALL: [Style; 4] = [
		Style::Regular,
		Style::Bold,
		Style::Italic,
		Style::BoldItalic,
	];

	/// Начертание по флагам ячейки.
	pub const fn from_flags(flags: CellFlags) -> Self {
		match (
			flags.contains(CellFlags::BOLD),
			flags.contains(CellFlags::ITALIC),
		) {
			(false, false) => Style::Regular,
			(true, false) => Style::Bold,
			(false, true) => Style::Italic,
			(true, true) => Style::BoldItalic,
		}
	}

	/// Начертание из признаков шрифта (`OS/2.fsSelection` / вес).
	pub const fn from_traits(is_bold: bool, is_italic: bool) -> Self {
		match (is_bold, is_italic) {
			(false, false) => Style::Regular,
			(true, false) => Style::Bold,
			(false, true) => Style::Italic,
			(true, true) => Style::BoldItalic,
		}
	}

	/// Номер канала атласа.
	pub const fn channel(self) -> usize {
		self as usize
	}

	pub const fn is_bold(self) -> bool {
		matches!(self, Style::Bold | Style::BoldItalic)
	}

	pub const fn is_italic(self) -> bool {
		matches!(self, Style::Italic | Style::BoldItalic)
	}
}

/// Одна ячейка консольной сетки.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cell {
	/// Кодовая точка Unicode (ADR-0009); `'\0'` — пустая ячейка.
	pub ch: char,
	/// Цвет символа (0x00RRGGBB).
	pub fg: Color,
	/// Цвет фона (0x00RRGGBB).
	pub bg: Color,
	/// Флаги ячейки.
	pub flags: CellFlags,
}

impl Cell {
	/// Создать ячейку.
	pub fn new(ch: char, fg: Color, bg: Color, flags: CellFlags) -> Self {
		Self { ch, fg, bg, flags }
	}

	/// Пустая ячейка.
	pub fn blank() -> Self {
		Self::default()
	}

	/// Начертание глифа ячейки.
	pub const fn style(&self) -> Style {
		Style::from_flags(self.flags)
	}

	/// Упаковка ячейки в массив из 4 u32 (RGBA32UI текстура).
	///
	/// Расположение (ADR-0009 п.4; в текстуру данных GPU рендереры кладут не `ch`, а слот — §6.1):
	///   [0]  R = ch (кодовая точка целиком)
	///   [1]  G = fg цвет
	///   [2]  B = bg цвет
	///   [3]  A = flags
	pub fn to_packed(&self) -> [u32; 4] {
		[
			u32::from(self.ch),
			self.fg.0,
			self.bg.0,
			u32::from(self.flags.bits()),
		]
	}

	/// Распаковка ячейки из массива из 4 u32. Недопустимая кодовая точка (суррогат,
	/// > U+10FFFF) даёт пустую ячейку `'\0'`.
	pub fn from_packed(v: &[u32; 4]) -> Self {
		Self {
			ch: char::from_u32(v[0]).unwrap_or('\0'),
			fg: Color(v[1]),
			bg: Color(v[2]),
			flags: CellFlags::from_bits_truncate(v[3] as u16),
		}
	}
}
