//! Стилизованный текст: [`Span`] — фрагмент одного стиля, [`Line`] — строка из фрагментов с
//! выравниванием, [`Text`] — строки. Ширина — число символов: одна кодовая точка занимает одну
//! ячейку (модель `gterm`), графем и двухшириных символов нет. Перенос — по словам, слова длиннее
//! строки рвутся по символам.

use std::borrow::Cow;

use crate::style::Style;

/// Горизонтальное выравнивание строки в области.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Alignment {
	#[default]
	Left,
	Center,
	Right,
}

/// Фрагмент текста одного стиля.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Span<'a> {
	pub content: Cow<'a, str>,
	pub style: Style,
}

impl<'a> Span<'a> {
	pub fn raw<T: Into<Cow<'a, str>>>(content: T) -> Self {
		Self {
			content: content.into(),
			style: Style::new(),
		}
	}

	pub fn styled<T: Into<Cow<'a, str>>>(content: T, style: Style) -> Self {
		Self {
			content: content.into(),
			style,
		}
	}

	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	/// Ширина в ячейках — число символов.
	pub fn width(&self) -> usize {
		self.content.chars().count()
	}

	/// Символы фрагмента с его стилем, уже наложенным на `base`.
	pub fn styled_chars(&self, base: Style) -> impl Iterator<Item = (char, Style)> + '_ {
		let style = base.patch(self.style);
		self.content.chars().map(move |ch| (ch, style))
	}
}

impl<'a> From<&'a str> for Span<'a> {
	fn from(s: &'a str) -> Self {
		Self::raw(s)
	}
}

impl From<String> for Span<'_> {
	fn from(s: String) -> Self {
		Self::raw(s)
	}
}

/// Строка из фрагментов; стиль строки — основа для стилей фрагментов.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Line<'a> {
	pub spans: Vec<Span<'a>>,
	pub style: Style,
	pub alignment: Option<Alignment>,
}

impl<'a> Line<'a> {
	pub fn raw<T: Into<Cow<'a, str>>>(content: T) -> Self {
		Self::from(Span::raw(content))
	}

	pub fn styled<T: Into<Cow<'a, str>>>(content: T, style: Style) -> Self {
		Self::from(Span::styled(content, style))
	}

	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	pub fn alignment(mut self, alignment: Alignment) -> Self {
		self.alignment = Some(alignment);
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

	pub fn width(&self) -> usize {
		self.spans.iter().map(Span::width).sum()
	}

	/// Все символы строки со стилями: `base` → стиль строки → стиль фрагмента.
	pub fn styled_chars(&self, base: Style) -> impl Iterator<Item = (char, Style)> + '_ {
		let line_style = base.patch(self.style);
		self.spans
			.iter()
			.flat_map(move |span| span.styled_chars(line_style))
	}
}

impl<'a> From<Span<'a>> for Line<'a> {
	fn from(span: Span<'a>) -> Self {
		Self {
			spans: vec![span],
			..Self::default()
		}
	}
}

impl<'a> From<Vec<Span<'a>>> for Line<'a> {
	fn from(spans: Vec<Span<'a>>) -> Self {
		Self {
			spans,
			..Self::default()
		}
	}
}

impl<'a> From<&'a str> for Line<'a> {
	fn from(s: &'a str) -> Self {
		Self::raw(s)
	}
}

impl From<String> for Line<'_> {
	fn from(s: String) -> Self {
		Self::raw(s)
	}
}

/// Многострочный текст.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Text<'a> {
	pub lines: Vec<Line<'a>>,
	pub style: Style,
	pub alignment: Option<Alignment>,
}

impl<'a> Text<'a> {
	/// Текст из строки: разбивается по `'\n'`.
	pub fn raw<T: Into<Cow<'a, str>>>(content: T) -> Self {
		let content: Cow<'a, str> = content.into();
		let lines = match content {
			Cow::Borrowed(s) => s.lines().map(Line::raw).collect(),
			Cow::Owned(s) => s.lines().map(|l| Line::raw(l.to_owned())).collect(),
		};
		Self {
			lines,
			..Self::default()
		}
	}

	pub fn styled<T: Into<Cow<'a, str>>>(content: T, style: Style) -> Self {
		Self::raw(content).style(style)
	}

	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	pub fn alignment(mut self, alignment: Alignment) -> Self {
		self.alignment = Some(alignment);
		self
	}

	pub fn height(&self) -> usize {
		self.lines.len()
	}

	/// Ширина самой длинной строки.
	pub fn width(&self) -> usize {
		self.lines.iter().map(Line::width).max().unwrap_or(0)
	}
}

impl<'a> From<&'a str> for Text<'a> {
	fn from(s: &'a str) -> Self {
		Self::raw(s)
	}
}

impl From<String> for Text<'_> {
	fn from(s: String) -> Self {
		Self::raw(s)
	}
}

impl<'a> From<Line<'a>> for Text<'a> {
	fn from(line: Line<'a>) -> Self {
		Self {
			lines: vec![line],
			..Self::default()
		}
	}
}

impl<'a> From<Vec<Line<'a>>> for Text<'a> {
	fn from(lines: Vec<Line<'a>>) -> Self {
		Self {
			lines,
			..Self::default()
		}
	}
}

impl<'a> From<Span<'a>> for Text<'a> {
	fn from(span: Span<'a>) -> Self {
		Self::from(Line::from(span))
	}
}

/// Готовая к выводу строка: символы со стилями и выравнивание.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StyledLine {
	pub chars: Vec<(char, Style)>,
	pub alignment: Alignment,
}

impl StyledLine {
	pub fn width(&self) -> usize {
		self.chars.len()
	}
}

/// Перенос строки по ширине `width`: по пробелам, слово длиннее строки — по символам;
/// `trim` — убирать пробелы в начале и конце получившихся строк. Пустая строка даёт одну пустую.
pub fn wrap_line(chars: &[(char, Style)], width: usize, trim: bool) -> Vec<Vec<(char, Style)>> {
	if width == 0 {
		return vec![Vec::new()];
	}
	let mut lines: Vec<Vec<(char, Style)>> = Vec::new();
	let mut current: Vec<(char, Style)> = Vec::new();
	let finish = |current: &mut Vec<(char, Style)>, lines: &mut Vec<Vec<(char, Style)>>| {
		if trim {
			while current.last().is_some_and(|(c, _)| *c == ' ') {
				current.pop();
			}
		}
		lines.push(std::mem::take(current));
	};
	let mut i = 0;
	while i < chars.len() {
		if chars[i].0 == ' ' {
			if !(trim && current.is_empty()) {
				if current.len() >= width {
					finish(&mut current, &mut lines);
					if trim {
						i += 1;
						continue;
					}
				}
				current.push(chars[i]);
			}
			i += 1;
			continue;
		}
		let start = i;
		while i < chars.len() && chars[i].0 != ' ' {
			i += 1;
		}
		let word = &chars[start..i];
		if current.len() + word.len() > width && !current.is_empty() {
			finish(&mut current, &mut lines);
		}
		if word.len() > width {
			for piece in word.chunks(width) {
				if !current.is_empty() {
					finish(&mut current, &mut lines);
				}
				current.extend_from_slice(piece);
			}
		} else {
			current.extend_from_slice(word);
		}
	}
	finish(&mut current, &mut lines);
	lines
}
