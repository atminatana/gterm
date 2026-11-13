//! Выделение в поле ввода (ADR-0030, ТЗ К1–К3, К6): `Shift` с клавишами движения, снятие
//! движением, замена выделенного набором и удалением, протяжка мышью с захватом и прокруткой
//! за краем, `Shift` + нажатие, отрисовка выделения только под фокусом.

mod common;

use common::{BG, FG, Msg, key_msg, left_down, left_up, moved, screen};
use gterm::cell::Color;
use gterm_widgets::input::{Handled, Messages, MouseMsg};
use gterm_widgets::widgets::{TextInput, TextInputState};
use gterm_widgets::{
	Constraint, Control, Layout, Rect, StatefulWidget, Style, Ui, WidgetEvent, WidgetEvents,
};
use winit::keyboard::{Key, ModifiersState, NamedKey};

const ACCENT: Color = Color::from_rgb(0xFF, 0x80, 0x00);
const FIELD: Rect = Rect::new(0, 0, 10, 1);

fn press(state: &mut TextInputState, key: NamedKey, modifiers: ModifiersState) -> Handled {
	let key = Key::Named(key);
	state.key(&key_msg(&key, modifiers), &mut Messages::<Msg>::new())
}

fn plain(state: &mut TextInputState, key: NamedKey) {
	assert_eq!(press(state, key, ModifiersState::empty()), Handled::Yes);
}

fn shift(state: &mut TextInputState, key: NamedKey) {
	assert_eq!(press(state, key, ModifiersState::SHIFT), Handled::Yes);
}

fn type_text(state: &mut TextInputState, text: &str) {
	let handled = Control::<Msg>::text(state, text, &mut Messages::new());
	assert_eq!(handled, Handled::Yes);
}

fn changed(value: &str) -> Option<WidgetEvent> {
	Some(WidgetEvent::Changed(value.to_owned()))
}

/// Поле шириной 10 ячеек в дереве: мышь идёт через маршрутизацию с захватом.
fn ui(state: &mut TextInputState) -> Ui<'_, Msg> {
	Ui::split(
		Layout::vertical([Constraint::Length(1)]),
		vec![Ui::stateful(TextInput::new(), state)],
	)
}

fn draw(state: &mut TextInputState) {
	ui(state).render(FIELD, &mut screen(10, 1));
}

#[test]
fn shift_with_movement_keys_extends_the_selection_and_plain_movement_drops_it() {
	use NamedKey::{ArrowLeft, ArrowRight, End, Home};
	let mut state = TextInputState::with_value("abcdef");
	assert_eq!(state.selection(), None);
	assert_eq!(state.selected_text(), "");

	shift(&mut state, ArrowLeft);
	shift(&mut state, ArrowLeft);
	assert_eq!(state.selection(), Some((4, 6)));
	assert_eq!(state.selected_text(), "ef");
	assert_eq!(state.cursor, 4);

	// Якорь на месте: выделение растёт и сжимается вслед за курсором.
	shift(&mut state, Home);
	assert_eq!(state.selection(), Some((0, 6)));
	shift(&mut state, ArrowRight);
	assert_eq!(state.selection(), Some((1, 6)));

	// Без `Shift`: `←` — в начало выделения, `→` — в конец.
	plain(&mut state, ArrowLeft);
	assert_eq!((state.cursor, state.selection()), (1, None));
	shift(&mut state, End);
	assert_eq!(state.selected_text(), "bcdef");
	plain(&mut state, ArrowRight);
	assert_eq!((state.cursor, state.selection()), (6, None));

	// У края расширять некуда — выделения нет.
	shift(&mut state, ArrowRight);
	assert_eq!((state.cursor, state.selection()), (6, None));

	state.select_all();
	assert_eq!((state.cursor, state.selection()), (6, Some((0, 6))));
	plain(&mut state, Home);
	assert_eq!((state.cursor, state.selection()), (0, None));
	state.select_all();
	state.clear_selection();
	assert_eq!(state.selection(), None);
}

#[test]
fn selection_is_counted_in_characters_and_a_disabled_field_ignores_everything() {
	let mut state = TextInputState::with_value("яблоко");
	shift(&mut state, NamedKey::ArrowLeft);
	shift(&mut state, NamedKey::ArrowLeft);
	assert_eq!(state.selected_text(), "ко");

	state.enabled = false;
	assert_eq!(
		press(&mut state, NamedKey::Home, ModifiersState::SHIFT),
		Handled::No
	);
	assert_eq!(state.selected_text(), "ко");
}

#[test]
fn typing_and_deleting_replace_the_selection_with_a_single_change_event() {
	use NamedKey::{ArrowLeft, Backspace, Delete, Enter};
	let mut state = TextInputState::with_value("hello world");
	for _ in 0..5 {
		shift(&mut state, ArrowLeft);
	}
	assert_eq!(state.selected_text(), "world");
	state.take_event();

	type_text(&mut state, "X");
	assert_eq!(state.value(), "hello X");
	assert_eq!((state.cursor, state.selection()), (7, None));
	assert_eq!(state.take_event(), changed("hello X"));
	assert_eq!(state.take_event(), None, "одно событие на замену");

	state.select_all();
	plain(&mut state, Backspace);
	assert_eq!((state.value(), state.cursor), ("", 0));
	assert_eq!(state.take_event(), changed(""));

	state.set_value("abcd");
	shift(&mut state, ArrowLeft);
	shift(&mut state, ArrowLeft);
	plain(&mut state, Delete);
	assert_eq!((state.value(), state.cursor), ("ab", 2));
	assert_eq!(state.take_event(), changed("ab"));
	// Без выделения `Delete` и `Backspace` работают как раньше.
	plain(&mut state, Backspace);
	assert_eq!(state.value(), "a");

	// `Enter` выделение не трогает; программная замена значения — снимает.
	state.select_all();
	plain(&mut state, Enter);
	assert_eq!(state.selection(), Some((0, 1)));
	state.set_value("new");
	assert_eq!(state.selection(), None);
	assert!(!state.delete_selection());
}

#[test]
fn dragging_with_the_mouse_selects_and_scrolls_past_the_edges() {
	let mut state = TextInputState::with_value("abcdefghijklmnop");
	state.cursor = 0;
	draw(&mut state);
	assert_eq!(state.scroll, 0);

	// Движение без нажатия поле не принимает.
	let routed = ui(&mut state).route_mouse(&moved(3, 0), FIELD);
	assert_eq!(routed.handled, Handled::No);
	assert_eq!(state.selection(), None);

	ui(&mut state).route_mouse(&left_down(2, 0), FIELD);
	assert_eq!((state.cursor, state.selection()), (2, None));
	assert!(state.node.captured);
	let routed = ui(&mut state).route_mouse(&moved(5, 0), FIELD);
	assert_eq!(routed.handled, Handled::Yes);
	assert_eq!(state.selected_text(), "cde");

	// Указатель правее поля: курсор уходит за видимый край, отрисовка прокручивает.
	ui(&mut state).route_mouse(&moved(12, 0), FIELD);
	assert_eq!(state.selection(), Some((2, 10)));
	draw(&mut state);
	assert_eq!(state.scroll, 1);
	ui(&mut state).route_mouse(&moved(12, 0), FIELD);
	assert_eq!(state.selection(), Some((2, 11)));

	// Указатель левее поля: на символ левее первого видимого.
	ui(&mut state).route_mouse(&moved(-3, 0), FIELD);
	assert_eq!(state.selection(), Some((0, 2)));

	ui(&mut state).route_mouse(&left_up(-3, 0), FIELD);
	assert!(!state.node.captured);
	assert_eq!(
		state.selected_text(),
		"ab",
		"отпускание выделение сохраняет"
	);
}

#[test]
fn a_click_drops_the_selection_and_shift_click_extends_it() {
	let mut state = TextInputState::with_value("abcdefgh");
	state.select_all();
	draw(&mut state);

	ui(&mut state).route_mouse(&left_down(4, 0), FIELD);
	ui(&mut state).route_mouse(&left_up(4, 0), FIELD);
	assert_eq!((state.cursor, state.selection()), (4, None));

	let shift_click = MouseMsg {
		modifiers: ModifiersState::SHIFT,
		..left_down(7, 0)
	};
	ui(&mut state).route_mouse(&shift_click, FIELD);
	ui(&mut state).route_mouse(&left_up(7, 0), FIELD);
	assert_eq!(state.selected_text(), "efg");

	// Набор после клика без протяжки ничего лишнего не удаляет.
	ui(&mut state).route_mouse(&left_down(1, 0), FIELD);
	ui(&mut state).route_mouse(&left_up(1, 0), FIELD);
	type_text(&mut state, "X");
	assert_eq!(state.value(), "aXbcdefgh");
}

#[test]
fn the_selection_is_drawn_only_under_focus_and_replaces_the_cursor_cell() {
	let colors = |state: &mut TextInputState, widget: TextInput<'_>| {
		let mut s = screen(8, 1);
		widget.render(Rect::new(0, 0, 8, 1), &mut s, state);
		(0..8)
			.map(|col| (s.cell(col, 0).fg, s.cell(col, 0).bg))
			.collect::<Vec<_>>()
	};
	let normal = (FG, BG);
	let swapped = (BG, FG);

	let mut state = TextInputState::with_value("abcdef");
	state.node.focused = true;
	plain(&mut state, NamedKey::Home);
	plain(&mut state, NamedKey::ArrowRight);
	shift(&mut state, NamedKey::ArrowRight);
	shift(&mut state, NamedKey::ArrowRight);
	assert_eq!(state.selection(), Some((1, 3)));

	// Ячейки 1 и 2 — выделение; ячейка курсора (3) отдельно не рисуется.
	let cells = colors(&mut state, TextInput::new());
	assert_eq!(cells[..5], [normal, swapped, swapped, normal, normal]);

	let cells = colors(
		&mut state,
		TextInput::new().selection_style(Style::new().bg(ACCENT)),
	);
	assert_eq!(cells[..4], [normal, (FG, ACCENT), (FG, ACCENT), normal]);

	state.node.focused = false;
	let cells = colors(&mut state, TextInput::new());
	assert!(cells.iter().all(|&cell| cell == normal), "{cells:?}");

	// Без выделения под фокусом — курсор, как раньше.
	state.node.focused = true;
	state.clear_selection();
	let cells = colors(&mut state, TextInput::new());
	assert_eq!(cells[..5], [normal, normal, normal, swapped, normal]);
}
