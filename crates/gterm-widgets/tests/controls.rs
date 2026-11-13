//! Встроенные контролы (ADR-0026, ТЗ К4/К7): список и таблица — клавиши навигации, событие
//! только при изменении выделения, `PgUp`/`PgDn` по карте строк, клик по строке многострочного
//! пункта, колесо, стиль фокуса; кнопка — состояния, активация клавишей и мышью с отпусканием
//! внутри, отмена снаружи; привязки описания превращают события в сообщения.

mod common;

use common::{BG, FG, Msg, assert_screen, key_msg, left_down, left_up, mouse, screen};
use gterm::cell::Color;
use gterm_widgets::input::{Handled, Messages, MouseKind};
use gterm_widgets::widgets::{Block, Button, ButtonState, List, ListState, Row, Table, TableState};
use gterm_widgets::{
	Constraint, Control, Layout, Rect, StatefulWidget, Style, Ui, WidgetEvent, WidgetEvents,
};
use winit::keyboard::{Key, ModifiersState, NamedKey};

const ACCENT: Color = Color::from_rgb(0xFF, 0x80, 0x00);
const PRESSED: Color = Color::from_rgb(0x80, 0x00, 0x00);
const DISABLED: Color = Color::from_rgb(0x60, 0x60, 0x60);

/// Нажать именованную клавишу и забрать событие состояния.
fn press<S: Control<Msg> + WidgetEvents>(
	state: &mut S,
	key: NamedKey,
) -> (Handled, Option<WidgetEvent>) {
	let key = Key::Named(key);
	let mut out = Messages::new();
	let handled = state.key(&key_msg(&key, ModifiersState::empty()), &mut out);
	assert!(
		out.is_empty(),
		"встроенный контрол сам сообщений не публикует"
	);
	(handled, state.take_event())
}

fn click<S: Control<Msg> + WidgetEvents>(
	state: &mut S,
	msg: &gterm_widgets::MouseMsg,
) -> (Handled, Option<WidgetEvent>) {
	let mut out = Messages::new();
	let handled = state.mouse(msg, &mut out);
	(handled, state.take_event())
}

#[test]
fn list_state_navigates_with_keys_and_reports_only_changes() {
	let items = ["one", "two", "three", "four", "five"];
	let mut state = ListState::default();
	assert!(Control::<Msg>::focusable(&state));

	// До первой отрисовки граница неизвестна: вниз без выделения — первый пункт.
	assert_eq!(
		press(&mut state, NamedKey::ArrowDown),
		(Handled::Yes, Some(WidgetEvent::Selected(0)))
	);
	assert_eq!(
		press(&mut state, NamedKey::ArrowDown),
		(Handled::Yes, Some(WidgetEvent::Selected(1)))
	);

	// После отрисовки в 3 строки страница — 3, последний пункт — 4.
	let mut s = screen(8, 3);
	StatefulWidget::render(List::new(items), Rect::new(0, 0, 8, 3), &mut s, &mut state);
	assert_eq!(
		press(&mut state, NamedKey::PageDown),
		(Handled::Yes, Some(WidgetEvent::Selected(4)))
	);
	assert_eq!(
		press(&mut state, NamedKey::ArrowDown),
		(Handled::Yes, None),
		"на последнем пункте выделение не меняется — события нет"
	);
	assert_eq!(
		press(&mut state, NamedKey::Home),
		(Handled::Yes, Some(WidgetEvent::Selected(0)))
	);
	assert_eq!(press(&mut state, NamedKey::ArrowUp), (Handled::Yes, None));
	assert_eq!(
		press(&mut state, NamedKey::End),
		(Handled::Yes, Some(WidgetEvent::Selected(4)))
	);
	assert_eq!(
		press(&mut state, NamedKey::PageUp),
		(Handled::Yes, Some(WidgetEvent::Selected(1)))
	);
	assert_eq!(press(&mut state, NamedKey::Enter), (Handled::No, None));
}

#[test]
fn list_state_click_hits_multiline_items_wheel_moves_selection_and_focus_styles_the_border() {
	let area = Rect::new(2, 1, 9, 5);
	let list = || {
		List::new([gterm_widgets::Text::raw("a\nb"), "c".into(), "d".into()])
			.block(Block::bordered())
			.focus_style(Style::new().fg(ACCENT))
	};
	let mut state = ListState::default();
	state.node.area = area; // так делает раскладка дерева
	let mut s = screen(12, 7);
	StatefulWidget::render(list(), area, &mut s, &mut state);
	assert_screen(
		&s,
		&[
			"",
			"  ┌───────┐",
			"  │a      │",
			"  │b      │",
			"  │c      │",
			"  └───────┘",
			"",
		],
	);
	assert_eq!(s.cell(2, 1).fg, FG, "без фокуса рамка обычная");

	// Вторая строка двухстрочного пункта — тот же пункт 0; строка `c` — пункт 1.
	assert_eq!(
		click(&mut state, &left_down(5, 3)),
		(Handled::Yes, Some(WidgetEvent::Selected(0)))
	);
	assert_eq!(
		click(&mut state, &left_down(5, 4)),
		(Handled::Yes, Some(WidgetEvent::Selected(1)))
	);
	// Рамка и повторный клик по той же строке — без события.
	assert_eq!(click(&mut state, &left_down(5, 1)), (Handled::Yes, None));
	assert_eq!(click(&mut state, &left_down(5, 4)), (Handled::Yes, None));
	// Отпускание после принятого нажатия — принято, движение — нет.
	assert_eq!(click(&mut state, &left_up(5, 4)), (Handled::Yes, None));
	assert_eq!(
		click(&mut state, &mouse(MouseKind::Move, None, 5, 4)),
		(Handled::No, None)
	);

	// Колесо: вниз (отрицательное) — следующий пункт, вверх на 2 — к первому.
	assert_eq!(
		click(
			&mut state,
			&mouse(MouseKind::Wheel { lines: -1.0 }, None, 5, 3)
		),
		(Handled::Yes, Some(WidgetEvent::Selected(2)))
	);
	assert_eq!(
		click(
			&mut state,
			&mouse(MouseKind::Wheel { lines: 2.0 }, None, 5, 3)
		),
		(Handled::Yes, Some(WidgetEvent::Selected(0)))
	);

	state.node.focused = true;
	let mut s = screen(12, 7);
	StatefulWidget::render(list(), area, &mut s, &mut state);
	assert_eq!(s.cell(2, 1).fg, ACCENT, "под фокусом рамка в стиле фокуса");
	assert_eq!(s.cell(3, 2).ch, 'a', "содержимое не изменилось");
}

#[test]
fn table_state_skips_the_header_and_navigates_rows() {
	let area = Rect::new(0, 0, 12, 3);
	let table = Table::new(
		[Row::new(["1", "alpha"]), Row::new(["2", "beta"])],
		[Constraint::Length(1), Constraint::Fill(1)],
	)
	.header(Row::new(["#", "name"]));
	let mut state = TableState::default();
	state.node.area = area;
	let mut s = screen(12, 3);
	StatefulWidget::render(table, area, &mut s, &mut state);

	assert_eq!(
		click(&mut state, &left_down(3, 0)),
		(Handled::Yes, None),
		"заголовок"
	);
	assert_eq!(
		click(&mut state, &left_down(3, 2)),
		(Handled::Yes, Some(WidgetEvent::Selected(1)))
	);
	assert_eq!(
		press(&mut state, NamedKey::ArrowUp),
		(Handled::Yes, Some(WidgetEvent::Selected(0)))
	);
	assert_eq!(press(&mut state, NamedKey::ArrowUp), (Handled::Yes, None));
	assert_eq!(press(&mut state, NamedKey::Tab), (Handled::No, None));
}

#[test]
fn button_draws_its_states_and_activates_by_key_or_release_inside() {
	let button = || {
		Button::new("OK")
			.focus_style(Style::new().fg(ACCENT))
			.pressed_style(Style::new().bg(PRESSED))
			.disabled_style(Style::new().fg(DISABLED))
	};
	let area = Rect::new(0, 0, 6, 1);
	let mut state = ButtonState::new();
	state.node.area = area;

	let mut s = screen(6, 1);
	StatefulWidget::render(button(), area, &mut s, &mut state);
	assert_screen(&s, &["  OK  "]);
	assert_eq!((s.cell(2, 0).fg, s.cell(2, 0).bg), (FG, BG));

	state.node.focused = true;
	let mut s = screen(6, 1);
	StatefulWidget::render(button(), area, &mut s, &mut state);
	assert_eq!(s.cell(2, 0).fg, ACCENT);

	state.pressed = true;
	let mut s = screen(6, 1);
	StatefulWidget::render(button(), area, &mut s, &mut state);
	assert_eq!(s.cell(0, 0).bg, PRESSED, "нажатие сильнее фокуса");
	state.pressed = false;

	// Клавиши.
	assert_eq!(
		press(&mut state, NamedKey::Enter),
		(Handled::Yes, Some(WidgetEvent::Activated))
	);
	assert_eq!(
		press(&mut state, NamedKey::Space),
		(Handled::Yes, Some(WidgetEvent::Activated))
	);
	assert_eq!(press(&mut state, NamedKey::ArrowDown), (Handled::No, None));

	// Мышь: активация отпусканием внутри; снаружи и при отмене — нет.
	assert_eq!(click(&mut state, &left_down(1, 0)), (Handled::Yes, None));
	assert!(state.pressed);
	assert_eq!(
		click(&mut state, &left_up(1, 0)),
		(Handled::Yes, Some(WidgetEvent::Activated))
	);
	assert!(!state.pressed);
	click(&mut state, &left_down(1, 0));
	assert_eq!(click(&mut state, &left_up(9, 9)), (Handled::Yes, None));
	assert!(!state.pressed);
	click(&mut state, &left_down(1, 0));
	assert_eq!(
		click(&mut state, &mouse(MouseKind::Cancel, None, -1, -1)),
		(Handled::Yes, None)
	);
	assert!(!state.pressed);
	assert_eq!(
		click(&mut state, &left_up(1, 0)),
		(Handled::No, None),
		"без нажатия"
	);

	// Неактивная: не фокусируется, не реагирует, рисуется своим стилем.
	let mut disabled = ButtonState::disabled();
	disabled.node.area = area;
	assert!(!Control::<Msg>::focusable(&disabled));
	assert_eq!(press(&mut disabled, NamedKey::Enter), (Handled::No, None));
	assert_eq!(click(&mut disabled, &left_down(1, 0)), (Handled::No, None));
	let mut s = screen(6, 1);
	StatefulWidget::render(button(), area, &mut s, &mut disabled);
	assert_eq!(s.cell(2, 0).fg, DISABLED);
}

#[test]
fn bindings_turn_state_events_into_application_messages() {
	struct App {
		list: ListState,
		ok: ButtonState,
	}
	impl App {
		fn ui(&mut self) -> Ui<'_, Msg> {
			Ui::split(
				Layout::vertical([Constraint::Length(3), Constraint::Length(1)]),
				vec![
					Ui::stateful(List::new(["one", "two", "three"]), &mut self.list)
						.on_select(Msg::Picked),
					Ui::stateful(Button::new("OK"), &mut self.ok).on_activate(Msg::Pressed),
				],
			)
		}
	}
	let mut app = App {
		list: ListState::default(),
		ok: ButtonState::new(),
	};
	let plain = ModifiersState::empty();
	let down = Key::Named(NamedKey::ArrowDown);
	let enter = Key::Named(NamedKey::Enter);
	let tab = Key::Named(NamedKey::Tab);

	let routed = app.ui().route_key(&key_msg(&down, plain));
	assert_eq!(routed.handled, Handled::Yes);
	assert_eq!(routed.messages, vec![Msg::Picked(0)]);
	let routed = app.ui().route_key(&key_msg(&down, plain));
	assert_eq!(routed.messages, vec![Msg::Picked(1)]);

	// Enter список не берёт — приложению, без сообщений.
	let routed = app.ui().route_key(&key_msg(&enter, plain));
	assert_eq!(routed.handled, Handled::No);
	assert!(routed.messages.is_empty());

	app.ui().route_key(&key_msg(&tab, plain));
	assert!(app.ok.node.focused);
	let routed = app.ui().route_key(&key_msg(&enter, plain));
	assert_eq!(routed.messages, vec![Msg::Pressed]);
	// Привязка задаётся заново каждым описанием — активируется и во второй раз.
	let routed = app.ui().route_key(&key_msg(&enter, plain));
	assert_eq!(routed.messages, vec![Msg::Pressed]);

	// Клик по строке списка через дерево: фокус — списку, сообщение — по привязке.
	let mut s = screen(10, 4);
	app.ui().render(Rect::new(0, 0, 10, 4), &mut s);
	let routed = app
		.ui()
		.route_mouse(&left_down(2, 2), Rect::new(0, 0, 10, 4));
	assert_eq!(routed.messages, vec![Msg::Picked(2)]);
	assert!(app.list.node.focused && !app.ok.node.focused);
}
