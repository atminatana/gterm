//! Текст: ширина по символам, стили фрагментов поверх строки, перенос по словам.

use gterm::cell::{CellFlags, Color};
use gterm_widgets::text::wrap_line;
use gterm_widgets::{Line, Span, Style, Text};

fn words(lines: &[Vec<(char, Style)>]) -> Vec<String> {
	lines
		.iter()
		.map(|l| l.iter().map(|(c, _)| *c).collect())
		.collect()
}

#[test]
fn widths_count_code_points() {
	assert_eq!(Span::raw("Привет").width(), 6);
	assert_eq!(
		Line::from(vec![Span::raw("ab"), Span::raw("cde")]).width(),
		5
	);
	let text = Text::raw("a\nbbb\ncc");
	assert_eq!(text.height(), 3);
	assert_eq!(text.width(), 3);
}

#[test]
fn span_style_overrides_line_and_base() {
	let base = Style::new().fg(Color::WHITE).bg(Color::BLACK);
	let line = Line::from(vec![
		Span::raw("a"),
		Span::styled("b", Style::new().fg(Color::GREEN).bold()),
	])
	.style(Style::new().underlined());
	let chars: Vec<_> = line.styled_chars(base).collect();
	assert_eq!(chars[0].1, base.underlined());
	assert_eq!(chars[1].1.fg, Some(Color::GREEN));
	assert_eq!(chars[1].1.bg, Some(Color::BLACK));
	assert!(
		chars[1]
			.1
			.add
			.contains(CellFlags::BOLD | CellFlags::UNDERLINE)
	);
}

#[test]
fn style_patch_and_apply() {
	let a = Style::new().fg(Color::WHITE).bold();
	let b = Style::new().bg(Color::BLACK).not_bold().italic();
	let patched = a.patch(b);
	assert_eq!(patched.fg, Some(Color::WHITE));
	assert_eq!(patched.bg, Some(Color::BLACK));
	assert_eq!(patched.add, CellFlags::ITALIC);
	assert_eq!(patched.remove, CellFlags::BOLD);
	let mut cell = gterm::cell::Cell::new('x', Color::CYAN, Color::MAGENTA, CellFlags::BOLD);
	patched.apply(&mut cell);
	assert_eq!(
		(cell.fg, cell.bg, cell.flags),
		(Color::WHITE, Color::BLACK, CellFlags::ITALIC)
	);
}

fn chars(s: &str) -> Vec<(char, Style)> {
	s.chars().map(|c| (c, Style::new())).collect()
}

#[test]
fn wrap_by_words() {
	assert_eq!(
		words(&wrap_line(&chars("the quick brown fox"), 10, true)),
		["the quick", "brown fox"]
	);
	assert_eq!(
		words(&wrap_line(&chars("the quick brown fox"), 10, false)),
		["the quick ", "brown fox"]
	);
	assert_eq!(
		words(&wrap_line(&chars("abcdefgh"), 3, true)),
		["abc", "def", "gh"]
	);
	assert_eq!(
		words(&wrap_line(&chars("ab abcdefg"), 4, true)),
		["ab", "abcd", "efg"]
	);
	assert_eq!(words(&wrap_line(&chars(""), 5, true)), [""]);
	assert_eq!(words(&wrap_line(&chars("a b"), 0, true)), [""]);
	assert_eq!(
		words(&wrap_line(&chars("  leading"), 5, true)),
		["leadi", "ng"],
		"trim убирает пробелы в начале строки"
	);
}
