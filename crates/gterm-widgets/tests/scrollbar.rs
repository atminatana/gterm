//! Полоса прокрутки (ADR-0032): размер и положение ползунка по состоянию, стрелки, обе
//! ориентации, свои знаки и стили, отрисовка только в своей колонке или строке.

mod common;

use common::{BG, FG, assert_screen, screen};
use gterm::buffer::ScreenBuffer;
use gterm::cell::Color;
use gterm_widgets::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
use gterm_widgets::{Rect, Style, Widget};

const ACCENT: Color = Color::from_rgb(0xFF, 0x80, 0x00);
const DIM: Color = Color::from_rgb(0x80, 0x80, 0x80);

/// Вертикальная полоса высотой `height` — её знаки сверху вниз одной строкой.
fn vertical(scrollbar: Scrollbar, state: ScrollbarState, height: usize) -> String {
	let mut s = screen(1, height);
	scrollbar
		.state(state)
		.render(Rect::new(0, 0, 1, height as u16), &mut s);
	column(&s, 0)
}

fn column(s: &ScreenBuffer, col: usize) -> String {
	(0..s.rows())
		.map(|row| match s.cell(col, row).ch {
			'\0' => ' ',
			ch => ch,
		})
		.collect()
}

fn state(content: usize, position: usize, viewport: usize) -> ScrollbarState {
	ScrollbarState::new(content)
		.position(position)
		.viewport_length(viewport)
}

#[test]
fn the_thumb_size_and_position_follow_the_state() {
	let bar = Scrollbar::new(ScrollbarOrientation::Vertical);
	// Дорожка 6 ячеек между стрелками; видно 6 из 20 — ползунок 2 ячейки.
	assert_eq!(vertical(bar, state(20, 0, 6), 8), "▲██││││▼");
	assert_eq!(vertical(bar, state(20, 7, 6), 8), "▲││██││▼");
	assert_eq!(vertical(bar, state(20, 14, 6), 8), "▲││││██▼");
	// Смещение дальше последнего возможного подрезается.
	assert_eq!(vertical(bar, state(20, 99, 6), 8), "▲││││██▼");

	// Содержимое помещается целиком — ползунок во всю дорожку.
	assert_eq!(vertical(bar, state(5, 0, 6), 8), "▲██████▼");
	assert_eq!(vertical(bar, state(0, 0, 0), 8), "▲██████▼");
	// Длинное содержимое: ползунок не меньше одной ячейки и доходит до краёв.
	assert_eq!(vertical(bar, state(10_000, 0, 10), 8), "▲█│││││▼");
	assert_eq!(vertical(bar, state(10_000, 9_990, 10), 8), "▲│││││█▼");
	// Нулевая видимая часть считается единицей.
	assert_eq!(vertical(bar, state(12, 11, 0), 8), "▲│││││█▼");
}

#[test]
fn arrows_are_optional_and_dropped_on_a_short_bar() {
	let bar = Scrollbar::new(ScrollbarOrientation::Vertical);
	assert_eq!(vertical(bar.arrows(false), state(16, 0, 4), 8), "██││││││");
	assert_eq!(vertical(bar.arrows(false), state(16, 12, 4), 8), "││││││██");
	// Короче трёх ячеек стрелки не помещаются — вся полоса отдаётся дорожке.
	assert_eq!(vertical(bar, state(4, 0, 2), 2), "█│");
	assert_eq!(vertical(bar, state(4, 2, 2), 2), "│█");
	assert_eq!(vertical(bar, state(9, 4, 3), 3), "▲█▼");
}

#[test]
fn a_horizontal_bar_uses_the_last_row_and_custom_symbols_and_styles() {
	let mut s = screen(8, 2);
	Scrollbar::new(ScrollbarOrientation::Horizontal)
		.style(Style::new().fg(DIM))
		.thumb_style(Style::new().fg(ACCENT))
		.state(state(20, 14, 6))
		.render(Rect::new(0, 0, 8, 2), &mut s);
	assert_screen(&s, &["", "◀────██▶"]);
	assert_eq!(s.cell(0, 1).fg, DIM, "стрелка — стилем полосы");
	assert_eq!(s.cell(1, 1).fg, DIM, "дорожка — стилем полосы");
	assert_eq!(s.cell(5, 1).fg, ACCENT, "ползунок — своим стилем");
	assert_eq!((s.cell(0, 0).fg, s.cell(0, 0).bg), (FG, BG));

	let mut s = screen(6, 1);
	Scrollbar::new(ScrollbarOrientation::Horizontal)
		.arrows(false)
		.track_symbol('.')
		.thumb_symbol('#')
		.state(state(12, 0, 4))
		.render(Rect::new(0, 0, 6, 1), &mut s);
	assert_screen(&s, &["##...."]);
}

#[test]
fn a_vertical_bar_touches_only_the_last_column_of_its_area() {
	let mut s = screen(6, 4);
	Scrollbar::new(ScrollbarOrientation::Vertical)
		.state(state(8, 0, 2))
		.render(Rect::new(1, 0, 3, 4), &mut s);
	assert_screen(&s, &["   ▲", "   █", "   │", "   ▼"]);

	// Область за краем буфера обрезается, пустая — ничего не рисует.
	let mut s = screen(2, 3);
	Scrollbar::new(ScrollbarOrientation::Vertical)
		.state(state(8, 0, 2))
		.render(Rect::new(0, 0, 9, 9), &mut s);
	assert_eq!(column(&s, 1), "▲█▼");
	assert_eq!(column(&s, 0), "   ");
	Scrollbar::new(ScrollbarOrientation::Vertical).render(Rect::new(0, 0, 0, 3), &mut s);
	assert_eq!(column(&s, 0), "   ");
}
