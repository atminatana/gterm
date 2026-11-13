//! Виджеты части 2 (ADR-0023, ТЗ 2026-10-23 К2): `Clear`, `Gauge`, `LineGauge` — рендер в
//! `ScreenBuffer`, сравнение с текстовыми снимками, стили в контрольных ячейках, обрезка по
//! области.

mod common;

use common::{BG, FG, assert_screen, rows, screen};
use gterm::buffer::ScreenBuffer;
use gterm::cell::{CellFlags, Color};
use gterm_widgets::widgets::{Block, Clear, Gauge, LineGauge};
use gterm_widgets::{BufferExt, Line, Rect, Style, Widget};

const ACCENT: Color = Color::from_rgb(0xFF, 0x80, 0x00);
const DIM: Color = Color::from_rgb(0x80, 0x80, 0x80);
const TRACK: Color = Color::from_rgb(0x20, 0x20, 0x40);

fn colors(s: &ScreenBuffer, col: usize, row: usize) -> (Color, Color) {
	(s.cell(col, row).fg, s.cell(col, row).bg)
}

fn gauge_row(gauge: Gauge<'_>, width: usize) -> String {
	let mut s = screen(width, 1);
	gauge.render(Rect::new(0, 0, width as u16, 1), &mut s);
	rows(&s).remove(0)
}

#[test]
fn clear_resets_only_its_area_to_the_buffer_defaults() {
	let mut s = screen(4, 3);
	let all = Rect::new(0, 0, 4, 3);
	s.fill(all, 'x', Style::new().fg(ACCENT).bg(DIM).bold());
	Clear.render(Rect::new(1, 1, 2, 1), &mut s);
	assert_screen(&s, &["xxxx", "x  x", "xxxx"]);
	for col in [1, 2] {
		assert_eq!(colors(&s, col, 1), (FG, BG));
		assert_eq!(s.cell(col, 1).flags, CellFlags::empty());
	}
	assert_eq!(
		colors(&s, 0, 1),
		(ACCENT, DIM),
		"ячейки вне области не тронуты"
	);

	// Область за краем буфера обрезается, пустая — ничего не делает.
	Clear.render(Rect::new(3, 2, 10, 10), &mut s);
	Clear.render(Rect::new(0, 0, 0, 3), &mut s);
	assert_screen(&s, &["xxxx", "x  x", "xxx "]);
}

#[test]
fn gauge_fills_by_ratio_with_eighth_blocks_and_clamps_the_ratio() {
	let bare = || Gauge::new().label("");
	assert_eq!(gauge_row(bare().ratio(0.5), 10), "█████     ");
	// 0,33 × 10 ячеек = 26 восьмых: три полные и блок в 2/8.
	assert_eq!(gauge_row(bare().ratio(0.33), 10), "███▎      ");
	assert_eq!(
		gauge_row(bare().ratio(0.33).use_unicode(false), 10),
		"███       "
	);
	// 7/8 и 1/8 — крайние знаки ряда восьмушек.
	assert_eq!(gauge_row(bare().ratio(7.0 / 8.0), 1), "▉");
	assert_eq!(gauge_row(bare().ratio(1.0 / 8.0), 1), "▏");

	assert_eq!(gauge_row(bare().ratio(0.0), 4), "    ");
	assert_eq!(gauge_row(bare().ratio(1.0), 4), "████");
	assert_eq!(gauge_row(bare().ratio(1.7), 4), "████");
	assert_eq!(gauge_row(bare().ratio(-3.0), 4), "    ");
	assert_eq!(gauge_row(bare().ratio(f64::NAN), 4), "    ");
	assert_eq!(gauge_row(bare().percent(250), 4), "████");
	assert_eq!(gauge_row(bare().percent(25), 4), "█   ");
}

#[test]
fn gauge_label_is_centered_and_inverted_over_the_filled_part() {
	let mut s = screen(10, 3);
	Gauge::new()
		.ratio(0.5)
		.gauge_style(Style::new().fg(ACCENT).bg(TRACK))
		.render(Rect::new(0, 0, 10, 3), &mut s);
	// Заливка на всю высоту, подпись по умолчанию — проценты, в средней строке.
	assert_screen(&s, &["█████     ", "███50%    ", "█████     "]);
	assert_eq!(colors(&s, 0, 0), (ACCENT, TRACK), "заливка — цветом полосы");
	assert_eq!(colors(&s, 9, 2), (ACCENT, TRACK), "фон незалитой части");
	assert_eq!(
		colors(&s, 3, 1),
		(TRACK, ACCENT),
		"символ над заливкой обращён"
	);
	assert_eq!(
		colors(&s, 5, 1),
		(ACCENT, TRACK),
		"символ за заливкой — обычный"
	);

	// Своя подпись; длиннее области — обрезается.
	assert_eq!(
		gauge_row(Gauge::new().ratio(0.0).label("готово"), 10),
		"  готово  "
	);
	assert_eq!(
		gauge_row(Gauge::new().ratio(0.0).label("загрузка"), 4),
		"загр"
	);
	assert_eq!(gauge_row(Gauge::new().ratio(0.256), 6), "█26%  ");
}

#[test]
fn gauge_draws_inside_its_block_and_never_outside_the_area() {
	let mut s = screen(8, 3);
	Gauge::new()
		.ratio(1.0)
		.label("")
		.block(Block::bordered().title("G"))
		.render(Rect::new(0, 0, 6, 3), &mut s);
	assert_screen(&s, &["┌G───┐", "│████│", "└────┘"]);

	let mut s = screen(10, 2);
	Gauge::new()
		.ratio(1.0)
		.label("")
		.style(Style::new().bg(DIM))
		.render(Rect::new(2, 0, 4, 1), &mut s);
	assert_screen(&s, &["  ████", ""]);
	assert_eq!(colors(&s, 1, 0), (FG, BG));
	assert_eq!(colors(&s, 6, 0), (FG, BG));
	assert_eq!(colors(&s, 2, 1), (FG, BG));
	assert_eq!(s.cell(2, 0).bg, DIM, "стиль области — внутри неё");

	// Область шире буфера обрезается без паники.
	let mut s = screen(3, 1);
	Gauge::new()
		.ratio(1.0)
		.label("")
		.render(Rect::new(1, 0, 50, 5), &mut s);
	assert_screen(&s, &[" ██"]);
}

#[test]
fn line_gauge_draws_label_and_a_line_split_by_ratio() {
	let render = |gauge: LineGauge<'_>, width: usize| {
		let mut s = screen(width, 2);
		gauge.render(Rect::new(0, 0, width as u16, 2), &mut s);
		s
	};
	let s = render(
		LineGauge::new()
			.ratio(0.5)
			.filled_style(Style::new().fg(ACCENT))
			.unfilled_style(Style::new().fg(DIM)),
		12,
	);
	// Одна строка: подпись, пробел, линия из 8 ячеек — половина заполнена; вторая строка пуста.
	assert_screen(&s, &["50% ━━━━────", ""]);
	assert_eq!(s.cell(4, 0).fg, ACCENT);
	assert_eq!(s.cell(7, 0).fg, ACCENT);
	assert_eq!(s.cell(8, 0).fg, DIM);
	assert_eq!(s.cell(0, 0).fg, FG, "подпись — стилем области");

	let s = render(
		LineGauge::new()
			.ratio(0.25)
			.label(Line::raw("диск"))
			.symbols('#', '.'),
		13,
	);
	assert_screen(&s, &["диск ##......", ""]);

	assert_eq!(rows(&render(LineGauge::new().ratio(0.0), 8))[0], "0% ─────");
	assert_eq!(
		rows(&render(LineGauge::new().ratio(9.0), 9))[0],
		"100% ━━━━"
	);
	// Места на линию нет — только подпись, обрезанная шириной.
	assert_eq!(rows(&render(LineGauge::new().ratio(1.0), 4))[0], "100%");
	assert_eq!(rows(&render(LineGauge::new().ratio(1.0), 3))[0], "100");

	let mut s = screen(12, 3);
	LineGauge::new()
		.ratio(1.0)
		.label("")
		.block(Block::bordered())
		.render(Rect::new(0, 0, 8, 3), &mut s);
	assert_screen(&s, &["┌──────┐", "│ ━━━━━│", "└──────┘"]);
}
