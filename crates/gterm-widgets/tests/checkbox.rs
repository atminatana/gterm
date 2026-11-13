//! Флажок (ADR-0031, ТЗ К1–К6): переключение пробелом, `Enter` и отпусканием мыши внутри области,
//! отмена отпусканием снаружи и уходом указателя, событие `Toggled` и привязка `on_toggle`,
//! программная смена без события, неактивный флажок, метки и стили, флажки в группе.

mod common;

use common::{
	BG, CallLog, FG, Msg, Stub, assert_screen, call_log, key_msg, left_down, left_up, mouse, screen,
};
use gterm::cell::Color;
use gterm_widgets::input::{Handled, Messages, MouseKind, Routed};
use gterm_widgets::widgets::{Block, Checkbox, CheckboxState};
use gterm_widgets::{
	Constraint, Control, Layout, Rect, StatefulWidget, Style, Ui, WidgetEvent, WidgetEvents,
};
use winit::keyboard::{Key, ModifiersState, NamedKey};

const ACCENT: Color = Color::from_rgb(0xFF, 0x80, 0x00);
const DIM: Color = Color::from_rgb(0x80, 0x80, 0x80);
const AREA: Rect = Rect::new(0, 0, 12, 2);

fn press(state: &mut CheckboxState, key: &Key) -> Handled {
	state.key(
		&key_msg(key, ModifiersState::empty()),
		&mut Messages::<Msg>::new(),
	)
}

fn toggled(checked: bool) -> Msg {
	Msg::Custom(if checked { "on" } else { "off" })
}

/// Флажок в первой строке области 12×2 с привязкой `on_toggle`; вторая строка пуста.
fn ui(state: &mut CheckboxState) -> Ui<'_, Msg> {
	Ui::split(
		Layout::vertical([Constraint::Length(1), Constraint::Length(1)]),
		vec![Ui::stateful(Checkbox::new("Опция"), state).on_toggle(toggled)],
	)
}

fn click(state: &mut CheckboxState, down: (i32, i32), up: (i32, i32)) -> Routed<Msg> {
	ui(state).route_mouse(&left_down(down.0, down.1), AREA);
	ui(state).route_mouse(&left_up(up.0, up.1), AREA)
}

#[test]
fn space_and_enter_toggle_and_other_keys_are_left_to_the_app() {
	let mut state = CheckboxState::new();
	assert!(Control::<Msg>::focusable(&state));
	assert!(!state.is_checked());

	assert_eq!(
		press(&mut state, &Key::Named(NamedKey::Space)),
		Handled::Yes
	);
	assert!(state.is_checked());
	assert_eq!(state.take_event(), Some(WidgetEvent::Toggled(true)));
	assert_eq!(state.take_event(), None, "одно событие на действие");

	// Пробел как символьная клавиша — то же.
	assert_eq!(press(&mut state, &Key::Character(" ".into())), Handled::Yes);
	assert_eq!(state.take_event(), Some(WidgetEvent::Toggled(false)));

	// `Enter` — то же, что пробел (решение владельца 2026-11-12).
	assert_eq!(
		press(&mut state, &Key::Named(NamedKey::Enter)),
		Handled::Yes
	);
	assert!(state.is_checked());
	assert_eq!(state.take_event(), Some(WidgetEvent::Toggled(true)));
	assert_eq!(
		press(&mut state, &Key::Named(NamedKey::Enter)),
		Handled::Yes
	);
	assert_eq!(state.take_event(), Some(WidgetEvent::Toggled(false)));

	// Буквы и прочие клавиши флажок не берёт.
	for key in [Key::Character("x".into()), Key::Named(NamedKey::Escape)] {
		assert_eq!(press(&mut state, &key), Handled::No, "{key:?}");
	}
	assert!(!state.is_checked());
	assert_eq!(state.take_event(), None);
}

#[test]
fn releasing_inside_toggles_and_releasing_outside_or_leaving_cancels() {
	let mut state = CheckboxState::new();
	ui(&mut state).layout(AREA);
	assert_eq!(state.node.area, Rect::new(0, 0, 12, 1));

	// Нажатие даёт захват и вид «нажат»; значение меняется только на отпускании внутри.
	ui(&mut state).route_mouse(&left_down(8, 0), AREA);
	assert!(state.pressed && state.node.captured);
	assert!(!state.is_checked());
	let routed = ui(&mut state).route_mouse(&left_up(8, 0), AREA);
	assert_eq!(routed.handled, Handled::Yes);
	assert_eq!(routed.messages, vec![toggled(true)]);
	assert!(state.is_checked() && !state.pressed && !state.node.captured);

	// Клик по подписи (любая ячейка области) — тоже переключение.
	let routed = click(&mut state, (0, 0), (11, 0));
	assert_eq!(routed.messages, vec![toggled(false)]);

	// Отпускание вне области — отмена.
	let routed = click(&mut state, (3, 0), (3, 1));
	assert_eq!(routed.handled, Handled::Yes);
	assert!(routed.messages.is_empty());
	assert!(!state.is_checked() && !state.pressed);

	// Уход указателя из окна — отмена.
	ui(&mut state).route_mouse(&left_down(3, 0), AREA);
	let routed = ui(&mut state).route_mouse(&mouse(MouseKind::Cancel, None, -1, -1), AREA);
	assert!(routed.messages.is_empty());
	assert!(!state.is_checked() && !state.pressed && !state.node.captured);
}

#[test]
fn programmatic_changes_do_not_report_events() {
	let mut state = CheckboxState::with_checked(true);
	assert!(state.is_checked());
	state.set_checked(false);
	state.toggle();
	assert!(state.is_checked());
	assert_eq!(state.take_event(), None);
}

#[test]
fn a_disabled_checkbox_takes_no_focus_and_no_input() {
	let mut state = CheckboxState::disabled();
	assert!(!Control::<Msg>::focusable(&state));
	assert_eq!(press(&mut state, &Key::Named(NamedKey::Space)), Handled::No);
	let routed = click(&mut state, (2, 0), (2, 0));
	assert_eq!(routed.handled, Handled::No);
	assert!(routed.messages.is_empty());
	assert!(!state.is_checked() && !state.pressed);
}

#[test]
fn marks_label_and_styles_follow_the_state() {
	let render = |state: &mut CheckboxState, widget: Checkbox<'_>| {
		let mut s = screen(12, 1);
		widget.render(Rect::new(0, 0, 12, 1), &mut s, state);
		s
	};
	let styled = || {
		Checkbox::new("Опция")
			.focus_style(Style::new().fg(ACCENT))
			.pressed_style(Style::new().bg(ACCENT))
			.disabled_style(Style::new().fg(DIM))
	};
	let colors = |s: &gterm::buffer::ScreenBuffer| (s.cell(1, 0).fg, s.cell(1, 0).bg);

	let mut state = CheckboxState::new();
	let s = render(&mut state, styled());
	assert_screen(&s, &["[ ] Опция"]);
	assert_eq!(colors(&s), (FG, BG));

	state.set_checked(true);
	state.node.focused = true;
	let s = render(&mut state, styled());
	assert_screen(&s, &["[x] Опция"]);
	assert_eq!(colors(&s), (ACCENT, BG), "под фокусом — стиль фокуса");

	// Нажат мышью — сильнее фокуса; неактивный — вместо обоих.
	state.pressed = true;
	assert_eq!(colors(&render(&mut state, styled())), (FG, ACCENT));
	state.enabled = false;
	assert_eq!(colors(&render(&mut state, styled())), (DIM, BG));

	// Свои метки; подпись обрезается шириной области.
	let mut state = CheckboxState::with_checked(true);
	let s = render(&mut state, Checkbox::new("Опция").marks("+", "-"));
	assert_screen(&s, &["+ Опция"]);
	state.set_checked(false);
	let mut narrow = screen(6, 1);
	Checkbox::new("Опция").render(Rect::new(0, 0, 6, 1), &mut narrow, &mut state);
	assert_screen(&narrow, &["[ ] Оп"]);
}

/// Свой контрол снаружи и группа с рамкой из двух флажков.
struct Form {
	outside: Stub,
	first: CheckboxState,
	second: CheckboxState,
}

impl Form {
	fn new(log: &CallLog) -> Self {
		Self {
			outside: Stub::new("outside", true, log),
			first: CheckboxState::new(),
			second: CheckboxState::new(),
		}
	}

	fn ui(&mut self) -> Ui<'_, Msg> {
		let group = Ui::group(
			Block::bordered().title(" Г "),
			Layout::vertical([Constraint::Length(1), Constraint::Length(1)]),
			vec![
				Ui::stateful(Checkbox::new("раз"), &mut self.first).on_toggle(toggled),
				Ui::stateful(Checkbox::new("два"), &mut self.second),
			],
		)
		.focus_style(Style::new().fg(ACCENT));
		Ui::split(
			Layout::vertical([Constraint::Length(1), Constraint::Length(4)]),
			vec![Ui::control(&mut self.outside), group],
		)
	}

	/// Цвет угла рамки группы после отрисовки.
	fn border_color(&mut self) -> Color {
		let mut s = screen(12, 5);
		self.ui().render(Rect::new(0, 0, 12, 5), &mut s);
		s.cell(0, 1).fg
	}

	fn focused(&self) -> (bool, bool, bool) {
		(
			self.outside.node.focused,
			self.first.node.focused,
			self.second.node.focused,
		)
	}
}

#[test]
fn checkboxes_in_a_group_take_part_in_the_tab_ring_and_light_its_border() {
	let log = call_log();
	let mut form = Form::new(&log);
	assert_eq!(form.border_color(), FG, "фокус снаружи — рамка обычная");
	assert_eq!(form.focused(), (true, false, false));

	let tab = Key::Named(NamedKey::Tab);
	let plain = ModifiersState::empty();
	form.ui().route_key(&key_msg(&tab, plain));
	assert_eq!(form.focused(), (false, true, false));
	assert_eq!(
		form.border_color(),
		ACCENT,
		"фокус внутри — стиль фокуса группы"
	);

	// Пробел под фокусом — сообщение привязки флажка.
	let space = Key::Named(NamedKey::Space);
	let routed = form.ui().route_key(&key_msg(&space, plain));
	assert_eq!(routed.messages, vec![toggled(true)]);

	form.ui().route_key(&key_msg(&tab, plain));
	assert_eq!(form.focused(), (false, false, true));
	assert_eq!(form.border_color(), ACCENT);
	// Флажок без привязки переключается, сообщений нет.
	let routed = form.ui().route_key(&key_msg(&space, plain));
	assert_eq!(routed.handled, Handled::Yes);
	assert!(routed.messages.is_empty());
	assert!(form.second.is_checked());

	form.ui().route_key(&key_msg(&tab, plain));
	assert_eq!(form.focused(), (true, false, false), "кольцо замкнулось");
	assert_eq!(form.border_color(), FG);
}
