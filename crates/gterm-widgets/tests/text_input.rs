//! Поле ввода (ADR-0027, ТЗ К2/К3): вставка в курсор и события, управляющие символы, края,
//! `Enter`, прокрутка с видимым курсором, клик ставит курсор, курсор обменом fg/bg только под
//! фокусом, подсказка, стиль фокуса, неактивное поле; привязки `on_change`/`on_submit`.

mod common;

use common::{BG, FG, Msg, assert_screen, key_msg, left_down, screen};
use gterm::cell::Color;
use gterm_widgets::input::{Handled, Messages};
use gterm_widgets::widgets::{Block, TextInput, TextInputState};
use gterm_widgets::{
	Constraint, Control, Layout, Rect, StatefulWidget, Style, Ui, WidgetEvent, WidgetEvents,
};
use winit::keyboard::{Key, ModifiersState, NamedKey};

const DIM: Color = Color::from_rgb(0x80, 0x80, 0x80);
const ACCENT: Color = Color::from_rgb(0xFF, 0x80, 0x00);

fn press(state: &mut TextInputState, key: NamedKey) -> (Handled, Option<WidgetEvent>) {
	let key = Key::Named(key);
	let mut out = Messages::<Msg>::new();
	let handled = state.key(&key_msg(&key, ModifiersState::empty()), &mut out);
	(handled, state.take_event())
}

fn type_text(state: &mut TextInputState, text: &str) -> (Handled, Option<WidgetEvent>) {
	let mut out = Messages::<Msg>::new();
	let handled = Control::<Msg>::text(state, text, &mut out);
	(handled, state.take_event())
}

fn changed(value: &str) -> Option<WidgetEvent> {
	Some(WidgetEvent::Changed(value.to_owned()))
}

#[test]
fn text_is_inserted_at_the_cursor_and_reported_only_when_it_changes() {
	use NamedKey::{ArrowLeft, ArrowRight, Backspace, Delete, End, Enter, Home};
	let mut state = TextInputState::new();
	assert!(Control::<Msg>::focusable(&state));

	assert_eq!(type_text(&mut state, "ab"), (Handled::Yes, changed("ab")));
	assert_eq!(state.cursor, 2);
	assert_eq!(press(&mut state, ArrowLeft), (Handled::Yes, None));
	assert_eq!(type_text(&mut state, "X"), (Handled::Yes, changed("aXb")));
	assert_eq!(state.cursor, 2);
	// Управляющие символы отбрасываются целиком — события нет, но текст «принят».
	assert_eq!(type_text(&mut state, "\r\n\t"), (Handled::Yes, None));
	assert_eq!(
		type_text(&mut state, "ю\n"),
		(Handled::Yes, changed("aXюb"))
	);

	assert_eq!(press(&mut state, Backspace), (Handled::Yes, changed("aXb")));
	assert_eq!(press(&mut state, Delete), (Handled::Yes, changed("aX")));
	assert_eq!(
		press(&mut state, Delete),
		(Handled::Yes, None),
		"за концом нечего удалять"
	);
	assert_eq!(press(&mut state, Home), (Handled::Yes, None));
	assert_eq!(state.cursor, 0);
	assert_eq!(
		press(&mut state, Backspace),
		(Handled::Yes, None),
		"в начале нечего удалять"
	);
	assert_eq!(press(&mut state, ArrowLeft), (Handled::Yes, None));
	assert_eq!(state.cursor, 0);
	assert_eq!(press(&mut state, End), (Handled::Yes, None));
	assert_eq!(state.cursor, 2);
	assert_eq!(press(&mut state, ArrowRight), (Handled::Yes, None));
	assert_eq!(state.cursor, 2, "курсор не уходит за конец");

	assert_eq!(
		press(&mut state, Enter),
		(Handled::Yes, Some(WidgetEvent::Submitted("aX".to_owned())))
	);
	// Символьная клавиша полем не обрабатывается — текст приходит отдельно.
	let key = Key::Character("z".into());
	let mut out = Messages::<Msg>::new();
	assert_eq!(
		state.key(&key_msg(&key, ModifiersState::empty()), &mut out),
		Handled::No
	);
	assert_eq!(state.value(), "aX");
	// Tab — не клавиша поля.
	assert_eq!(press(&mut state, NamedKey::Tab), (Handled::No, None));

	state.set_value("new");
	assert_eq!(
		(state.cursor, state.take_event()),
		(3, None),
		"программная смена — без события"
	);
	state.clear();
	assert_eq!(state.take_event(), None);
	assert_eq!(state.value(), "");
}

#[test]
fn scrolling_keeps_the_cursor_visible_and_the_cursor_swaps_colors_only_under_focus() {
	let area = Rect::new(0, 0, 6, 1);
	let mut state = TextInputState::with_value("hello world");
	assert_eq!(state.cursor, 11);

	// Без фокуса: хвост строки виден, курсора нет.
	let mut s = screen(6, 1);
	StatefulWidget::render(TextInput::new(), area, &mut s, &mut state);
	assert_screen(&s, &["world "]);
	assert_eq!(state.scroll, 6);
	assert_eq!(
		(s.cell(5, 0).fg, s.cell(5, 0).bg),
		(FG, BG),
		"без фокуса обмена нет"
	);

	// Под фокусом: курсор за концом строки — пустая ячейка с обменом fg/bg.
	state.node.focused = true;
	let mut s = screen(6, 1);
	StatefulWidget::render(TextInput::new(), area, &mut s, &mut state);
	assert_screen(&s, &["world "]);
	assert_eq!((s.cell(5, 0).fg, s.cell(5, 0).bg), (BG, FG));
	assert_eq!((s.cell(4, 0).fg, s.cell(4, 0).bg), (FG, BG));

	// Home: прокрутка к началу, курсор на первом символе.
	press(&mut state, NamedKey::Home);
	let mut s = screen(6, 1);
	StatefulWidget::render(TextInput::new(), area, &mut s, &mut state);
	assert_screen(&s, &["hello "]);
	assert_eq!(state.scroll, 0);
	assert_eq!((s.cell(0, 0).fg, s.cell(0, 0).bg), (BG, FG));

	// Курсор в середине за правым краем — окно сдвигается ровно до него.
	for _ in 0..7 {
		press(&mut state, NamedKey::ArrowRight);
	}
	let mut s = screen(6, 1);
	StatefulWidget::render(TextInput::new(), area, &mut s, &mut state);
	assert_eq!(state.scroll, 2);
	assert_screen(&s, &["llo wo"]);
	assert_eq!((s.cell(5, 0).fg, s.cell(5, 0).bg), (BG, FG));
}

#[test]
fn click_puts_the_cursor_and_the_field_ignores_other_mouse() {
	let area = Rect::new(3, 1, 5, 1);
	let mut state = TextInputState::with_value("abcdefgh");
	state.node.area = area;
	press(&mut state, NamedKey::Home);
	let mut s = screen(10, 3);
	StatefulWidget::render(TextInput::new(), area, &mut s, &mut state);
	assert_screen(&s, &["", "   abcde", ""]);

	let mut out = Messages::<Msg>::new();
	assert_eq!(state.mouse(&left_down(5, 1), &mut out), Handled::Yes);
	assert_eq!(state.cursor, 2, "колонка 5 — третий символ при прокрутке 0");
	// Прокрученное поле: клик учитывает смещение.
	press(&mut state, NamedKey::End);
	StatefulWidget::render(TextInput::new(), area, &mut screen(10, 3), &mut state);
	assert_eq!(state.scroll, 4);
	state.mouse(&left_down(3, 1), &mut out);
	assert_eq!(state.cursor, 4);
	// Клик за концом текста зажимается длиной.
	state.mouse(&left_down(7, 1), &mut out);
	assert_eq!(state.cursor, 8);
	assert!(out.is_empty());
}

#[test]
fn placeholder_focus_style_block_and_disabled_field() {
	let area = Rect::new(0, 0, 10, 3);
	let input = || {
		TextInput::new()
			.block(Block::bordered())
			.placeholder("имя")
			.placeholder_style(Style::new().fg(DIM))
			.focus_style(Style::new().fg(ACCENT))
	};
	let mut state = TextInputState::new();
	let mut s = screen(10, 3);
	StatefulWidget::render(input(), area, &mut s, &mut state);
	assert_screen(&s, &["┌────────┐", "│имя     │", "└────────┘"]);
	assert_eq!(s.cell(1, 1).fg, DIM, "подсказка своим стилем");
	assert_eq!(s.cell(0, 0).fg, FG, "рамка без фокуса обычная");

	state.node.focused = true;
	let mut s = screen(10, 3);
	StatefulWidget::render(input(), area, &mut s, &mut state);
	assert_eq!(s.cell(0, 0).fg, ACCENT, "рамка под фокусом — стилем фокуса");
	assert_eq!(s.cell(5, 1).fg, ACCENT, "поле под фокусом — стилем фокуса");
	// Курсор на первой ячейке поверх подсказки — обмен цветов.
	assert_eq!(s.cell(1, 1).bg, DIM);

	type_text(&mut state, "ab");
	let mut s = screen(10, 3);
	StatefulWidget::render(input(), area, &mut s, &mut state);
	assert_screen(&s, &["┌────────┐", "│ab      │", "└────────┘"]);

	let mut disabled = TextInputState::with_value("x");
	disabled.enabled = false;
	assert!(!Control::<Msg>::focusable(&disabled));
	assert_eq!(type_text(&mut disabled, "y"), (Handled::No, None));
	assert_eq!(
		press(&mut disabled, NamedKey::Backspace),
		(Handled::No, None)
	);
	assert_eq!(disabled.value(), "x");
}

#[test]
fn bindings_publish_change_and_submit_messages_through_the_tree() {
	let mut state = TextInputState::new();
	fn ui(state: &mut TextInputState) -> Ui<'_, Msg> {
		Ui::split(
			Layout::vertical([Constraint::Length(1)]),
			vec![
				Ui::stateful(TextInput::new(), state)
					.on_change(Msg::Text)
					.on_submit(|value| Msg::Text(format!("submit:{value}"))),
			],
		)
	}
	let routed = ui(&mut state).text("hi");
	assert_eq!(routed.handled, Handled::Yes);
	assert_eq!(routed.messages, vec![Msg::Text("hi".to_owned())]);
	let routed = ui(&mut state).route_key(&key_msg(
		&Key::Named(NamedKey::Enter),
		ModifiersState::empty(),
	));
	assert_eq!(routed.messages, vec![Msg::Text("submit:hi".to_owned())]);
	let routed = ui(&mut state).route_key(&key_msg(
		&Key::Named(NamedKey::ArrowLeft),
		ModifiersState::empty(),
	));
	assert_eq!(routed.handled, Handled::Yes);
	assert!(
		routed.messages.is_empty(),
		"движение курсора — не изменение"
	);
}

/// Обычный набор `winit` отдаёт в `KeyEvent.text`, а не через IME (ADR-0027): клавиша, не
/// принятая полем, доставляется ему как текст; `Ctrl`/`Alt` и управляющие символы — нет.
#[test]
fn typed_text_of_an_unhandled_key_reaches_the_field_through_the_tree() {
	let mut state = TextInputState::new();
	fn ui(state: &mut TextInputState) -> Ui<'_, Msg> {
		Ui::split(
			Layout::vertical([Constraint::Length(1)]),
			vec![Ui::stateful(TextInput::new(), state).on_change(Msg::Text)],
		)
	}
	let x_key = Key::Character("x".into());
	let typed = |text: &'static str, modifiers: ModifiersState| gterm_widgets::KeyMsg {
		key: &x_key,
		text: Some(text),
		modifiers,
	};
	let routed = ui(&mut state).route_key(&typed("ф", ModifiersState::empty()));
	assert_eq!(routed.handled, Handled::Yes);
	assert_eq!(routed.messages, vec![Msg::Text("ф".to_owned())]);
	// Shift — обычная заглавная; Ctrl/Alt — сочетание, не текст.
	let routed = ui(&mut state).route_key(&typed("Ф", ModifiersState::SHIFT));
	assert_eq!(routed.messages, vec![Msg::Text("фФ".to_owned())]);
	for modifiers in [
		ModifiersState::CONTROL,
		ModifiersState::ALT,
		ModifiersState::SUPER,
	] {
		let routed = ui(&mut state).route_key(&typed("c", modifiers));
		assert_eq!(routed.handled, Handled::No, "{modifiers:?}");
		assert!(routed.messages.is_empty());
	}
	// Esc несёт `"\u{1b}"` — управляющий символ, поле его не глотает, приложение получает Esc.
	let escape = gterm_widgets::KeyMsg {
		key: &Key::Named(NamedKey::Escape),
		text: Some("\u{1b}"),
		modifiers: ModifiersState::empty(),
	};
	let routed = ui(&mut state).route_key(&escape);
	assert_eq!(routed.handled, Handled::No);
	assert_eq!(state.value(), "фФ");
	// Enter несёт `"\r"`, но принят как клавиша — `Submitted`, без вставки.
	let enter = gterm_widgets::KeyMsg {
		key: &Key::Named(NamedKey::Enter),
		text: Some("\r"),
		modifiers: ModifiersState::empty(),
	};
	let routed = ui(&mut state).route_key(&enter);
	assert_eq!(routed.handled, Handled::Yes);
	assert_eq!(state.value(), "фФ");
}
