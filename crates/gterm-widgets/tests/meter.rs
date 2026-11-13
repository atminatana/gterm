//! Индикатор уровня `Meter` (ADR-0034; ТЗ `docs/spec/2026-11-13-tick-meter.md`, К6–К8): рендер в
//! `ScreenBuffer`, текстовые снимки, цвета ячеек по зонам, отметка пика, обрезка по области.

mod common;

use common::{BG, FG, assert_screen, rows, screen};
use gterm::buffer::ScreenBuffer;
use gterm::cell::Color;
use gterm_widgets::widgets::{Block, Meter};
use gterm_widgets::{Rect, Style, Widget};

const GREEN: Color = Color::from_rgb(0x00, 0xC0, 0x00);
const AMBER: Color = Color::from_rgb(0xFF, 0xC0, 0x00);
const RED: Color = Color::from_rgb(0xE0, 0x00, 0x00);
const PLAIN: Color = Color::from_rgb(0xA0, 0xA0, 0xFF);
const DIM: Color = Color::from_rgb(0x40, 0x40, 0x40);
const TRACK: Color = Color::from_rgb(0x20, 0x20, 0x40);

/// Зоны на шкале из десяти ячеек: зелёная — ячейки 0–4, янтарная — 5–7, красная — 8–9.
const ZONES: [(f64, Style); 3] = [
	(0.0, Style::new().fg(GREEN)),
	(0.5, Style::new().fg(AMBER)),
	(0.8, Style::new().fg(RED)),
];

fn colors(s: &ScreenBuffer, col: usize, row: usize) -> (Color, Color) {
	(s.cell(col, row).fg, s.cell(col, row).bg)
}

fn meter_row(meter: Meter<'_>, width: usize) -> String {
	let mut s = screen(width, 1);
	meter.render(Rect::new(0, 0, width as u16, 1), &mut s);
	rows(&s).remove(0)
}

/// Индикатор с зонами и своим стилем незаполненной части на экране шириной десять ячеек.
fn zoned(ratio: f64) -> ScreenBuffer {
	let mut s = screen(10, 1);
	Meter::new()
		.ratio(ratio)
		.zones(&ZONES)
		.unfilled_style(Style::new().fg(DIM).bg(TRACK))
		.render(Rect::new(0, 0, 10, 1), &mut s);
	s
}

#[test]
fn meter_fills_by_ratio_in_half_cell_steps_and_clamps_the_ratio() {
	assert_eq!(meter_row(Meter::new().ratio(0.5), 10), "█████░░░░░");
	// 0,55 от десяти ячеек — пять с половиной: шестая ячейка заполнена наполовину.
	assert_eq!(meter_row(Meter::new().ratio(0.55), 10), "█████▌░░░░");
	// Округление до ближайшей половины: 0,52 — ровно пять, 0,53 — пять с половиной.
	assert_eq!(meter_row(Meter::new().ratio(0.52), 10), "█████░░░░░");
	assert_eq!(meter_row(Meter::new().ratio(0.53), 10), "█████▌░░░░");
	assert_eq!(meter_row(Meter::new().ratio(0.05), 10), "▌░░░░░░░░░");
	assert_eq!(meter_row(Meter::new().ratio(0.95), 10), "█████████▌");

	assert_eq!(meter_row(Meter::new().ratio(0.0), 4), "░░░░");
	assert_eq!(meter_row(Meter::new().ratio(1.0), 4), "████");
	assert_eq!(meter_row(Meter::new().ratio(1.7), 4), "████");
	assert_eq!(meter_row(Meter::new().ratio(-3.0), 4), "░░░░");
	assert_eq!(meter_row(Meter::new().ratio(f64::NAN), 4), "░░░░");
	assert_eq!(meter_row(Meter::new(), 4), "░░░░", "без уровня — пусто");
}

#[test]
fn meter_cells_take_the_colour_of_their_zone_by_position() {
	// Полная шкала: цвет ячейки задаёт её место, а не уровень.
	let s = zoned(1.0);
	assert_screen(&s, &["██████████"]);
	for col in 0..5 {
		assert_eq!(s.cell(col, 0).fg, GREEN, "ячейка {col} — зелёная зона");
	}
	for col in 5..8 {
		assert_eq!(s.cell(col, 0).fg, AMBER, "ячейка {col} — янтарная зона");
	}
	for col in 8..10 {
		assert_eq!(s.cell(col, 0).fg, RED, "ячейка {col} — красная зона");
	}

	// Уровень в янтарной зоне: зелёные ячейки остаются зелёными, красные не горят.
	let s = zoned(0.7);
	assert_screen(&s, &["███████░░░"]);
	assert_eq!(s.cell(4, 0).fg, GREEN);
	assert_eq!(s.cell(6, 0).fg, AMBER);
	assert_eq!(
		colors(&s, 7, 0),
		(DIM, TRACK),
		"незаполненная — своим стилем"
	);
	assert_eq!(colors(&s, 9, 0), (DIM, TRACK));

	// Половинная ячейка: левая половина — цветом зоны, правая — фоном незаполненной части.
	let s = zoned(0.55);
	assert_screen(&s, &["█████▌░░░░"]);
	assert_eq!(colors(&s, 5, 0), (AMBER, TRACK));

	// Заполненные ячейки фон незаполненной части не получают.
	assert_eq!(s.cell(0, 0).bg, BG);
}

#[test]
fn meter_without_zones_uses_the_filled_style_and_zones_patch_it() {
	let mut s = screen(4, 1);
	Meter::new()
		.ratio(0.5)
		.filled_style(Style::new().fg(PLAIN).bg(TRACK))
		.render(Rect::new(0, 0, 4, 1), &mut s);
	assert_eq!(colors(&s, 0, 0), (PLAIN, TRACK));
	assert_eq!(colors(&s, 3, 0), (FG, BG), "незаполненная — цвета буфера");

	// Зона меняет только то, что в ней задано: фон остаётся от `filled_style`.
	let late_zone = [(0.5, Style::new().fg(RED))];
	let mut s = screen(4, 1);
	Meter::new()
		.ratio(1.0)
		.filled_style(Style::new().fg(PLAIN).bg(TRACK))
		.zones(&late_zone)
		.render(Rect::new(0, 0, 4, 1), &mut s);
	assert_eq!(
		colors(&s, 1, 0),
		(PLAIN, TRACK),
		"до первого порога — без зоны"
	);
	assert_eq!(colors(&s, 2, 0), (RED, TRACK));
}

#[test]
fn meter_marks_the_peak_in_its_zone_colour_inside_and_beyond_the_fill() {
	let peaked = |ratio: f64, peak: Option<f64>| {
		let mut s = screen(10, 1);
		Meter::new()
			.ratio(ratio)
			.peak(peak)
			.zones(&ZONES)
			.peak_symbol('┃')
			.unfilled_style(Style::new().fg(DIM).bg(TRACK))
			.render(Rect::new(0, 0, 10, 1), &mut s);
		s
	};

	// Пик правее заполненной части: одна ячейка в цвете своей зоны.
	let s = peaked(0.3, Some(0.9));
	assert_screen(&s, &["███░░░░░┃░"]);
	assert_eq!(s.cell(8, 0).fg, RED);
	assert_eq!(
		s.cell(8, 0).bg,
		BG,
		"пик — заполненная ячейка, без фона пустой части"
	);

	// Пик внутри заполненной части и на половинной ячейке.
	assert_screen(&peaked(0.8, Some(0.5)), &["████┃███░░"]);
	assert_eq!(peaked(0.8, Some(0.5)).cell(4, 0).fg, GREEN);
	assert_screen(&peaked(0.55, Some(0.55)), &["█████┃░░░░"]);

	// Границы: единица — последняя ячейка, сверх неё и «не число» обрезаются, ноль не рисуется.
	assert_screen(&peaked(0.0, Some(1.0)), &["░░░░░░░░░┃"]);
	assert_screen(&peaked(0.0, Some(4.0)), &["░░░░░░░░░┃"]);
	assert_screen(&peaked(0.0, Some(0.01)), &["┃░░░░░░░░░"]);
	assert_screen(&peaked(0.0, Some(0.0)), &["░░░░░░░░░░"]);
	assert_screen(&peaked(0.0, Some(f64::NAN)), &["░░░░░░░░░░"]);
	assert_screen(&peaked(0.2, None), &["██░░░░░░░░"]);

	// По умолчанию знак пика — полная ячейка.
	assert_eq!(
		meter_row(Meter::new().ratio(0.25).peak(Some(1.0)), 4),
		"█░░█"
	);
}

#[test]
fn meter_symbols_are_configurable() {
	let meter = Meter::new()
		.ratio(0.625)
		.filled_symbol('#')
		.half_symbol('+')
		.unfilled_symbol('.');
	assert_eq!(meter_row(meter, 4), "##+.");
}

#[test]
fn meter_draws_inside_its_block_and_never_outside_the_area() {
	let mut s = screen(8, 3);
	Meter::new()
		.ratio(0.5)
		.block(Block::bordered().title("M"))
		.render(Rect::new(0, 0, 6, 3), &mut s);
	assert_screen(&s, &["┌M───┐", "│██░░│", "└────┘"]);

	// Высота больше строки — полоса на всю высоту области; стиль области — только внутри неё.
	let mut s = screen(10, 3);
	Meter::new()
		.ratio(0.5)
		.style(Style::new().bg(DIM))
		.render(Rect::new(2, 0, 4, 2), &mut s);
	assert_screen(&s, &["  ██░░", "  ██░░", ""]);
	assert_eq!(s.cell(2, 0).bg, DIM);
	assert_eq!(colors(&s, 1, 0), (FG, BG));
	assert_eq!(colors(&s, 6, 0), (FG, BG));
	assert_eq!(colors(&s, 2, 2), (FG, BG));

	// Область шире буфера обрезается без паники: уровень считается от видимой ширины.
	let mut s = screen(3, 1);
	Meter::new()
		.ratio(1.0)
		.peak(Some(1.0))
		.render(Rect::new(1, 0, 40, 5), &mut s);
	assert_screen(&s, &[" ██"]);

	// Пустая область и область вне буфера — ничего не рисуется.
	let mut s = screen(4, 1);
	Meter::new()
		.ratio(1.0)
		.render(Rect::new(0, 0, 0, 1), &mut s);
	Meter::new()
		.ratio(1.0)
		.render(Rect::new(9, 9, 4, 1), &mut s);
	assert_screen(&s, &[""]);
}
