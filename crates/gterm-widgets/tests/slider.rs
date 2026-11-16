//! Ползунок (ADR-0036, ТЗ `2026-11-16-double-click-slider` П1–П4): клавиши и границы, клик по
//! дорожке, протяжка под захватом, отпускание и уход указателя, колесо, неактивный ползунок,
//! программная установка без события, зажатие значения при смене границ, вид и стили.

mod common;

use common::{BG, FG, Msg, assert_screen, key_msg, left_down, left_up, mouse, moved, screen};
use gterm::cell::Color;
use gterm_widgets::input::{Handled, MouseKind, MouseMsg, Routed};
use gterm_widgets::widgets::{Slider, SliderState};
use gterm_widgets::{Control, Rect, StatefulWidget, Style, Ui, WidgetEvents};
use winit::event::MouseButton;
use winit::keyboard::{Key, ModifiersState, NamedKey};

const ACCENT: Color = Color::from_rgb(0xFF, 0x80, 0x00);
const DIM: Color = Color::from_rgb(0x80, 0x80, 0x80);
/// Дорожка в 11 ячеек: при границах 0…100 колонка `c` — значение `10·c`.
const AREA: Rect = Rect::new(0, 0, 11, 1);

fn value(value: i64) -> Msg {
	Msg::Picked(usize::try_from(value).unwrap_or(usize::MAX))
}

fn ui(state: &mut SliderState) -> Ui<'_, Msg> {
	Ui::stateful(Slider::new(), state).on_value(value)
}

/// Ползунок 0…100 с шагом 5, разложенный в [`AREA`].
fn slider() -> SliderState {
	let mut state = SliderState::new(0, 100).with_step(5);
	ui(&mut state).layout(AREA);
	state
}

fn key(state: &mut SliderState, named: NamedKey) -> Routed<Msg> {
	let key = Key::Named(named);
	ui(state).route_key(&key_msg(&key, ModifiersState::empty()))
}

fn route(state: &mut SliderState, msg: MouseMsg) -> Routed<Msg> {
	ui(state).route_mouse(&msg, AREA)
}

fn wheel(lines: f32) -> MouseMsg {
	mouse(MouseKind::Wheel { lines }, None, 4, 0)
}

#[test]
fn arrows_step_home_and_end_jump_to_the_bounds_and_a_bound_gives_no_event() {
	let mut state = slider().with_value(50);
	assert!(Control::<Msg>::focusable(&state));

	let routed = key(&mut state, NamedKey::ArrowRight);
	assert_eq!(routed.handled, Handled::Yes);
	assert_eq!(routed.messages, vec![value(55)]);
	assert_eq!(
		key(&mut state, NamedKey::ArrowLeft).messages,
		vec![value(50)]
	);
	assert_eq!(key(&mut state, NamedKey::End).messages, vec![value(100)]);

	// В границе клавиша принята, значение то же, сообщений нет.
	for named in [NamedKey::End, NamedKey::ArrowRight] {
		let routed = key(&mut state, named);
		assert_eq!(routed.handled, Handled::Yes, "{named:?}");
		assert!(routed.messages.is_empty(), "{named:?}");
	}
	assert_eq!(key(&mut state, NamedKey::Home).messages, vec![value(0)]);
	assert!(key(&mut state, NamedKey::ArrowLeft).messages.is_empty());
	assert_eq!(state.value(), 0);

	// Прочие клавиши — приложению.
	for named in [NamedKey::ArrowUp, NamedKey::Enter, NamedKey::Escape] {
		assert_eq!(key(&mut state, named).handled, Handled::No, "{named:?}");
	}
}

#[test]
fn a_step_that_overshoots_a_bound_stops_at_the_bound() {
	let mut state = SliderState::new(0, 10).with_step(4).with_value(9);
	assert_eq!(
		key(&mut state, NamedKey::ArrowRight).messages,
		vec![value(10)]
	);
	assert_eq!(state.value(), 10);
}

#[test]
fn a_press_on_the_track_sets_the_value_by_the_pointer_position() {
	let mut state = slider().with_value(30);
	// Середина, оба края; нажатие берёт захват.
	for (col, expected) in [(5, 50), (0, 0), (10, 100), (3, 30)] {
		let routed = route(&mut state, left_down(col, 0));
		assert_eq!(routed.handled, Handled::Yes);
		assert_eq!(state.value(), expected, "колонка {col}");
		assert!(state.node.captured);
		route(&mut state, left_up(col, 0));
		assert!(!state.node.captured);
	}
	// Нажатие туда, где значение уже такое, сообщения не даёт.
	let routed = route(&mut state, left_down(3, 0));
	assert!(routed.messages.is_empty());
	route(&mut state, left_up(3, 0));

	// Правая кнопка ползунок не двигает.
	let right = mouse(MouseKind::Down, Some(MouseButton::Right), 8, 0);
	assert_eq!(route(&mut state, right).handled, Handled::No);
	assert_eq!(state.value(), 30);
}

#[test]
fn the_value_rounds_to_the_nearest_column_when_the_range_is_wider_than_the_track() {
	let mut state = SliderState::new(-50, 50);
	ui(&mut state).layout(Rect::new(2, 0, 4, 1));
	// Четыре ячейки на 100 единиц: колонки 2, 3, 4, 5 → −50, −17, 17, 50.
	let values: Vec<i64> = (2..6)
		.map(|col| {
			ui(&mut state).route_mouse(&left_down(col, 0), AREA);
			ui(&mut state).route_mouse(&left_up(col, 0), AREA);
			state.value()
		})
		.collect();
	assert_eq!(values, [-50, -17, 17, 50]);
}

#[test]
fn dragging_under_capture_follows_the_pointer_until_release() {
	let mut state = slider();
	let routed = route(&mut state, left_down(2, 0));
	assert_eq!(routed.messages, vec![value(20)]);

	// Движение под захватом — новое значение на каждую новую колонку, за краем — граница.
	assert_eq!(route(&mut state, moved(6, 0)).messages, vec![value(60)]);
	assert!(
		route(&mut state, moved(6, 3)).messages.is_empty(),
		"та же колонка"
	);
	assert_eq!(route(&mut state, moved(40, 5)).messages, vec![value(100)]);
	assert_eq!(route(&mut state, moved(-3, 0)).messages, vec![value(0)]);

	let routed = route(&mut state, left_up(7, 0));
	assert_eq!(routed.handled, Handled::Yes);
	assert!(routed.messages.is_empty(), "отпускание значения не меняет");
	assert!(!state.node.captured);
	assert_eq!(state.value(), 0);

	// Без захвата движение ползунку не адресовано.
	assert_eq!(route(&mut state, moved(5, 0)).handled, Handled::No);
	assert_eq!(state.value(), 0);
}

#[test]
fn leaving_the_window_ends_the_drag_and_keeps_the_value_reached() {
	let mut state = slider();
	route(&mut state, left_down(1, 0));
	route(&mut state, moved(8, 0));
	let routed = route(&mut state, mouse(MouseKind::Cancel, None, -1, -1));
	assert_eq!(routed.handled, Handled::Yes);
	assert!(routed.messages.is_empty());
	assert!(!state.node.captured);
	assert_eq!(state.value(), 80);
}

#[test]
fn the_wheel_changes_the_value_by_the_step() {
	let mut state = slider().with_value(50);
	assert_eq!(route(&mut state, wheel(1.0)).messages, vec![value(55)]);
	assert_eq!(route(&mut state, wheel(-2.0)).messages, vec![value(45)]);
	// Меньше половины строки (попиксельная прокрутка) — ни шага, ни сообщения.
	let routed = route(&mut state, wheel(0.3));
	assert_eq!(routed.handled, Handled::Yes);
	assert!(routed.messages.is_empty());
	state.set_value(100);
	assert!(route(&mut state, wheel(1.0)).messages.is_empty(), "граница");
}

#[test]
fn a_disabled_slider_takes_no_focus_and_no_input() {
	let mut state = slider().with_value(50);
	state.enabled = false;
	assert!(!Control::<Msg>::focusable(&state));
	assert_eq!(key(&mut state, NamedKey::ArrowRight).handled, Handled::No);
	assert_eq!(route(&mut state, left_down(9, 0)).handled, Handled::No);
	assert!(!state.node.captured);
	assert_eq!(route(&mut state, wheel(1.0)).handled, Handled::No);
	assert_eq!(state.value(), 50);
	assert!(!state.node.focused, "клик фокуса не дал");
}

#[test]
fn programmatic_changes_give_no_event_and_bounds_clamp_the_value() {
	let mut state = slider().with_value(70);
	state.set_value(150);
	assert_eq!(state.value(), 100, "зажато сверху");
	state.set_value(-5);
	assert_eq!(state.value(), 0, "зажато снизу");
	state.set_value(70);

	state.set_range(0, 40);
	assert_eq!((state.min(), state.max(), state.value()), (0, 40, 40));
	state.set_range(50, 60);
	assert_eq!(state.value(), 50);
	// Верхняя граница ниже нижней поднимается до неё.
	state.set_range(10, 5);
	assert_eq!((state.min(), state.max(), state.value()), (10, 10, 10));
	state.set_step(0);
	assert_eq!(state.step(), 1);

	// Умолчание — проценты.
	let default = SliderState::default();
	assert_eq!(
		(
			default.min(),
			default.max(),
			default.value(),
			default.step()
		),
		(0, 100, 0, 1)
	);
	assert!(default.enabled);

	assert_eq!(
		state.take_event(),
		None,
		"программные изменения событий не дают"
	);
}

#[test]
fn the_track_shows_the_filled_part_the_thumb_and_the_styles_of_the_state() {
	let render = |state: &mut SliderState, widget: Slider| {
		let mut s = screen(11, 3);
		widget.render(Rect::new(0, 0, 11, 3), &mut s, state);
		s
	};
	let styled = || {
		Slider::new()
			.filled_style(Style::new().fg(ACCENT))
			.focus_style(Style::new().bg(ACCENT))
			.disabled_style(Style::new().fg(DIM))
	};

	let mut state = SliderState::new(0, 100).with_value(50);
	let s = render(&mut state, styled());
	// Дорожка — в средней строке области.
	assert_screen(&s, &["", "━━━━━█─────", ""]);
	assert_eq!(s.cell(0, 1).fg, ACCENT, "заполненная часть");
	assert_eq!(s.cell(8, 1).fg, FG, "незаполненная — обычный стиль");
	assert_eq!(s.cell(8, 1).bg, BG);

	state.set_value(0);
	assert_screen(&render(&mut state, styled()), &["", "█──────────", ""]);
	state.set_value(100);
	assert_screen(&render(&mut state, styled()), &["", "━━━━━━━━━━█", ""]);

	// Под фокусом — фон фокуса на всей области; неактивный — свой стиль вместо фокуса.
	state.node.focused = true;
	let s = render(&mut state, styled());
	assert_eq!((s.cell(3, 0).bg, s.cell(3, 1).bg), (ACCENT, ACCENT));
	state.enabled = false;
	let s = render(&mut state, styled());
	assert_eq!(s.cell(3, 1).fg, DIM);
	assert_eq!(s.cell(3, 1).bg, BG);

	// Свои знаки.
	let mut state = SliderState::new(0, 10).with_value(5);
	let s = render(
		&mut state,
		Slider::new()
			.filled_symbol('=')
			.thumb_symbol('O')
			.unfilled_symbol('.'),
	);
	assert_screen(&s, &["", "=====O.....", ""]);
}
