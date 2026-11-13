//! Виджеты части 1 (ADR-0023): рендер в `ScreenBuffer` и сравнение с текстовыми снимками.

mod common;

use common::{BG, FG, assert_screen, rows, screen};
use gterm::cell::{CellFlags, Color};
use gterm_widgets::text::Alignment;
use gterm_widgets::widgets::{
	Block, BorderType, Borders, List, ListState, Padding, Paragraph, Row, Table, TableState, Wrap,
};
use gterm_widgets::{Constraint, Line, Rect, StatefulWidget, Style, Text, Widget};

#[test]
fn block_border_types_titles_and_inner() {
	let mut s = screen(10, 4);
	Block::bordered()
		.title("Hi")
		.render(Rect::new(0, 0, 10, 4), &mut s);
	assert_screen(
		&s,
		&["┌Hi──────┐", "│        │", "│        │", "└────────┘"],
	);
	for (border_type, expected) in [
		(BorderType::Rounded, "╭──╮"),
		(BorderType::Double, "╔══╗"),
		(BorderType::Thick, "┏━━┓"),
	] {
		let mut s = screen(4, 2);
		Block::bordered()
			.border_type(border_type)
			.render(Rect::new(0, 0, 4, 2), &mut s);
		assert_eq!(rows(&s)[0], expected, "{border_type:?}");
	}
	let block = Block::bordered().padding(Padding::uniform(1));
	assert_eq!(block.inner(Rect::new(0, 0, 10, 6)), Rect::new(2, 2, 6, 2));
	assert_eq!(block.inner(Rect::new(0, 0, 4, 4)), Rect::default());
	let partial = Block::new().borders(Borders::TOP | Borders::LEFT);
	assert_eq!(partial.inner(Rect::new(0, 0, 10, 6)), Rect::new(1, 1, 9, 5));

	let mut s = screen(14, 3);
	Block::bordered()
		.title("L")
		.title(Line::from("C").centered())
		.title(Line::from("R").right_aligned())
		.title_bottom("B")
		.render(Rect::new(0, 0, 14, 3), &mut s);
	assert_screen(&s, &["┌L────C─────R┐", "│            │", "└B───────────┘"]);
}

#[test]
fn block_does_not_draw_outside_area_or_screen() {
	let mut s = screen(8, 4);
	Block::bordered()
		.style(Style::new().bg(Color::GREEN))
		.render(Rect::new(2, 1, 4, 2), &mut s);
	assert_screen(&s, &["", "  ┌──┐", "  └──┘", ""]);
	assert_eq!(s.cell(1, 1).bg, BG, "вне area фон не тронут");
	assert_eq!(s.cell(2, 1).bg, Color::GREEN);
	// Частично за экраном: область обрезается по экрану (как у ratatui), рамка — по обрезанной
	// области, паники нет.
	let mut s = screen(5, 3);
	Block::bordered().render(Rect::new(3, 1, 10, 10), &mut s);
	assert_screen(&s, &["", "   ┌┐", "   └┘"]);
}

#[test]
fn paragraph_wraps_aligns_and_scrolls() {
	let mut s = screen(12, 4);
	Paragraph::new("the quick brown fox jumps")
		.wrap(Wrap { trim: true })
		.render(Rect::new(0, 0, 12, 4), &mut s);
	assert_screen(&s, &["the quick", "brown fox", "jumps", ""]);

	let mut s = screen(12, 2);
	Paragraph::new("ab")
		.alignment(Alignment::Right)
		.render(Rect::new(0, 0, 12, 1), &mut s);
	Paragraph::new("cd")
		.alignment(Alignment::Center)
		.render(Rect::new(0, 1, 12, 1), &mut s);
	assert_screen(&s, &["          ab", "     cd"]);

	let mut s = screen(6, 2);
	Paragraph::new("1\n2\n3\n4")
		.scroll((2, 0))
		.render(Rect::new(0, 0, 6, 2), &mut s);
	assert_screen(&s, &["3", "4"]);
	let mut s = screen(4, 1);
	Paragraph::new("abcdefgh")
		.scroll((0, 3))
		.render(Rect::new(0, 0, 4, 1), &mut s);
	assert_screen(&s, &["defg"]);

	let mut s = screen(10, 3);
	Paragraph::new(Text::styled("x", Style::new().fg(Color::CYAN).bold()))
		.block(Block::bordered())
		.style(Style::new().bg(Color::MAGENTA))
		.render(Rect::new(0, 0, 10, 3), &mut s);
	assert_screen(&s, &["┌────────┐", "│x       │", "└────────┘"]);
	let cell = s.cell(1, 1);
	assert_eq!((cell.fg, cell.bg), (Color::CYAN, Color::MAGENTA));
	assert!(cell.flags.contains(CellFlags::BOLD));
	assert_eq!(
		s.cell(5, 1).bg,
		Color::MAGENTA,
		"стиль абзаца — на всю область"
	);
	assert_eq!(s.cell(5, 1).fg, FG);
}

#[test]
fn list_highlights_and_scrolls_to_selection() {
	let items = ["one", "two", "three", "four", "five"];
	let mut state = ListState::default();
	let mut s = screen(8, 3);
	let list = || {
		List::new(items)
			.highlight_symbol("> ")
			.highlight_style(Style::new().fg(Color::GREEN))
	};
	StatefulWidget::render(list(), Rect::new(0, 0, 8, 3), &mut s, &mut state);
	assert_screen(&s, &["  one", "  two", "  three"]);

	state.select(Some(3));
	let mut s = screen(8, 3);
	StatefulWidget::render(list(), Rect::new(0, 0, 8, 3), &mut s, &mut state);
	assert_screen(&s, &["  two", "  three", "> four"]);
	assert_eq!(state.offset, 1);
	assert_eq!(s.cell(2, 2).fg, Color::GREEN);
	assert_eq!(s.cell(2, 1).fg, FG);

	state.select_next();
	state.select_next();
	let mut s = screen(8, 3);
	StatefulWidget::render(list(), Rect::new(0, 0, 8, 3), &mut s, &mut state);
	assert_eq!(
		state.selected(),
		Some(4),
		"выделение зажимается последним пунктом"
	);
	assert_screen(&s, &["  three", "  four", "> five"]);

	state.select(Some(0));
	let mut s = screen(8, 3);
	StatefulWidget::render(list(), Rect::new(0, 0, 8, 3), &mut s, &mut state);
	assert_screen(&s, &["> one", "  two", "  three"]);
	assert_eq!(state.offset, 0);
}

#[test]
fn list_multiline_items_in_block() {
	let mut s = screen(9, 5);
	let list = List::new([Text::raw("a\nb"), Text::raw("c")]).block(Block::bordered());
	Widget::render(list, Rect::new(0, 0, 9, 5), &mut s);
	assert_screen(
		&s,
		&[
			"┌───────┐",
			"│a      │",
			"│b      │",
			"│c      │",
			"└───────┘",
		],
	);
}

#[test]
fn table_columns_header_and_highlight() {
	let rows_data = [Row::new(["1", "alpha", "x"]), Row::new(["2", "beta", "yy"])];
	let table = Table::new(
		rows_data.clone(),
		[
			Constraint::Length(1),
			Constraint::Fill(1),
			Constraint::Length(2),
		],
	)
	.header(Row::new(["#", "name", "v"]))
	.highlight_symbol(">")
	.row_highlight_style(Style::new().bg(Color::GREEN));
	let mut state = TableState::with_selected(Some(1));
	let mut s = screen(12, 3);
	StatefulWidget::render(table, Rect::new(0, 0, 12, 3), &mut s, &mut state);
	assert_screen(&s, &[" # name   v", " 1 alpha  x", ">2 beta   yy"]);
	assert_eq!(s.cell(5, 2).bg, Color::GREEN);
	assert_eq!(s.cell(5, 1).bg, BG);

	// Прокрутка к выделенной строке при нехватке высоты (заголовок занимает строку).
	let many = (0..6).map(|i| Row::new([format!("r{i}")]));
	let table = Table::new(many, [Constraint::Fill(1)]).header(Row::new(["h"]));
	let mut state = TableState::with_selected(Some(4));
	let mut s = screen(4, 3);
	StatefulWidget::render(table, Rect::new(0, 0, 4, 3), &mut s, &mut state);
	assert_screen(&s, &["h", "r3", "r4"]);
	assert_eq!(state.offset, 3);
}
