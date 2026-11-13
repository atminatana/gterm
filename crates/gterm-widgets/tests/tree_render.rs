//! Отрисовка через описание экрана (ADR-0026, ТЗ К5): кадр дерева совпадает с ручным рендером
//! тех же виджетов, области записаны в состояния, лишние дети пусты, стиль фокуса у группы и
//! листа — только под фокусом.

mod common;

use common::{FG, Msg, Stub, assert_screen, call_log, rows, screen};
use gterm::cell::Color;
use gterm_widgets::widgets::{Block, Paragraph};
use gterm_widgets::{Constraint, Layout, Rect, Style, Ui, Widget};

const ACCENT: Color = Color::from_rgb(0xFF, 0x80, 0x00);

#[test]
fn tree_render_equals_manual_render_of_the_same_widgets() {
	let log = call_log();
	let mut stub = Stub::new("x", false, &log);
	let area = Rect::new(0, 0, 12, 4);

	let mut by_tree = screen(12, 4);
	Ui::<Msg>::split(
		Layout::horizontal([Constraint::Length(7), Constraint::Fill(1)]),
		vec![
			Ui::group(
				Block::bordered().title("T"),
				Layout::vertical([Constraint::Fill(1)]),
				vec![Ui::widget(Paragraph::new("ab"))],
			),
			Ui::control(&mut stub),
		],
	)
	.render(area, &mut by_tree);

	let mut by_hand = screen(12, 4);
	let cols = Layout::horizontal([Constraint::Length(7), Constraint::Fill(1)]).split(area);
	let block = Block::bordered().title("T");
	let inner = block.inner(cols[0]);
	block.render(cols[0], &mut by_hand);
	Paragraph::new("ab").render(inner, &mut by_hand);
	let mut stub_by_hand = Stub::new("x", false, &log);
	gterm_widgets::Drawable::draw(&mut stub_by_hand, cols[1], &mut by_hand);

	assert_eq!(rows(&by_tree), rows(&by_hand));
	assert_screen(
		&by_tree,
		&[
			"┌T────┐xxxxx",
			"│ab   │xxxxx",
			"│     │xxxxx",
			"└─────┘xxxxx",
		],
	);
	assert_eq!(stub.node.area, cols[1], "область записана в состояние");
}

#[test]
fn extra_children_get_an_empty_area_and_are_not_drawn() {
	let log = call_log();
	let mut shown = Stub::new("s", false, &log);
	let mut extra = Stub::new("e", false, &log);
	let mut buf = screen(6, 2);
	Ui::<Msg>::split(
		Layout::vertical([Constraint::Length(2)]),
		vec![Ui::control(&mut shown), Ui::control(&mut extra)],
	)
	.render(Rect::new(0, 0, 6, 2), &mut buf);
	assert_screen(&buf, &["ssssss", "ssssss"]);
	assert_eq!(extra.node.area, Rect::default());
}

#[test]
fn focus_style_applies_to_the_group_border_and_the_leaf_only_under_focus() {
	let log = call_log();
	let mut inside = Stub::new("i", true, &log);
	let mut outside = Stub::new("o", true, &log);
	let area = Rect::new(0, 0, 8, 3);
	fn ui<'a>(inside: &'a mut Stub, outside: &'a mut Stub) -> Ui<'a, Msg> {
		Ui::split(
			Layout::horizontal([Constraint::Length(4), Constraint::Fill(1)]),
			vec![
				Ui::group(
					Block::bordered(),
					Layout::vertical([Constraint::Fill(1)]),
					vec![Ui::control(inside)],
				)
				.focus_style(Style::new().fg(ACCENT)),
				Ui::control(outside),
			],
		)
	}

	// Первое обращение отдаёт фокус первому кандидату — он внутри группы.
	let mut buf = screen(8, 3);
	ui(&mut inside, &mut outside).render(area, &mut buf);
	assert_screen(&buf, &["┌──┐oooo", "│II│oooo", "└──┘oooo"]);
	assert_eq!(
		buf.cell(0, 0).fg,
		ACCENT,
		"рамка группы подсвечена: фокус внутри"
	);

	// Фокус ушёл наружу — рамка в обычном стиле, лист внутри без фокуса.
	ui(&mut inside, &mut outside).focus_next();
	let mut buf = screen(8, 3);
	ui(&mut inside, &mut outside).render(area, &mut buf);
	assert_screen(&buf, &["┌──┐OOOO", "│ii│OOOO", "└──┘OOOO"]);
	assert_eq!(buf.cell(0, 0).fg, FG);
}
