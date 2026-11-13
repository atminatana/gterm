//! Маршрутизация клавиатуры и мыши (ADR-0026, ТЗ К2/К3/К4): доставка фокусу, «не обработано»
//! приложению, верхний контрол под ячейкой, фокус по левому нажатию до доставки, захват,
//! листы без состояния не адресуются, публикация сообщений своим контролом.

mod common;

use common::{CallLog, Msg, Stub, call_log, key_msg, left_down, left_up, mouse, moved, screen};
use gterm_widgets::input::{Handled, MouseKind};
use gterm_widgets::widgets::Paragraph;
use gterm_widgets::{Constraint, Layout, Rect, Ui};
use winit::event::MouseButton;
use winit::keyboard::{Key, ModifiersState, NamedKey};

/// Три контрола друг под другом по две строки (`a` 0–1, `b` 2–3, `c` 4–5, не фокусируется),
/// затем абзац без состояния (строки 6–7).
struct Screen {
	a: Stub,
	b: Stub,
	c: Stub,
}

const AREA: Rect = Rect::new(0, 0, 10, 8);

impl Screen {
	fn new(log: &CallLog) -> Self {
		Self {
			a: Stub::new("a", true, log),
			b: Stub::new("b", true, log).answering(Handled::Yes),
			c: Stub::new("c", false, log).answering(Handled::Yes),
		}
	}

	fn ui(&mut self) -> Ui<'_, Msg> {
		Ui::split(
			Layout::vertical([
				Constraint::Length(2),
				Constraint::Length(2),
				Constraint::Length(2),
				Constraint::Length(2),
			]),
			vec![
				Ui::control(&mut self.a),
				Ui::control(&mut self.b),
				Ui::control(&mut self.c),
				Ui::widget(Paragraph::new("текст")),
			],
		)
	}

	/// Области записываются раскладкой: до первого кадра мышь контролы не находит.
	fn lay_out(&mut self) {
		self.ui().layout(AREA);
	}

	fn focused(&self) -> Vec<&'static str> {
		[&self.a, &self.b, &self.c]
			.into_iter()
			.filter(|s| s.node.focused)
			.map(|s| s.name)
			.collect()
	}
}

#[test]
fn key_goes_to_the_focused_control_and_its_answer_returns_to_the_app() {
	let log = call_log();
	let mut screen = Screen::new(&log);
	let plain = ModifiersState::empty();
	let enter = Key::Named(NamedKey::Enter);

	// `a` отвечает «нет» — приложение получает «не обработано».
	let routed = screen.ui().route_key(&key_msg(&enter, plain));
	assert_eq!(routed.handled, Handled::No);
	assert_eq!(log.borrow().as_slice(), ["a:key:Named(Enter)"]);

	screen.ui().focus_next();
	let routed = screen.ui().route_key(&key_msg(&enter, plain));
	assert_eq!(routed.handled, Handled::Yes);
	assert_eq!(log.borrow().last().unwrap(), "b:key:Named(Enter)");
	assert_eq!(
		log.borrow().len(),
		2,
		"клавиша доставлена ровно одному контролу"
	);
}

#[test]
fn a_custom_control_publishes_messages_through_the_queue() {
	let log = call_log();
	let mut screen = Screen::new(&log);
	screen.a = Stub::new("a", true, &log).publishing("open");
	let plain = ModifiersState::empty();
	let routed = screen
		.ui()
		.route_key(&key_msg(&Key::Named(NamedKey::Enter), plain));
	assert_eq!(routed.messages, vec![Msg::Custom("open")]);
	let collected: Vec<Msg> = routed.into_iter().collect();
	assert_eq!(collected, vec![Msg::Custom("open")]);
}

#[test]
fn mouse_goes_to_the_control_under_the_cell_and_left_down_focuses_it_first() {
	let log = call_log();
	let mut screen = Screen::new(&log);
	screen.lay_out();
	assert_eq!(screen.b.node.area, Rect::new(0, 2, 10, 2));

	let routed = screen.ui().route_mouse(&left_down(3, 3), AREA);
	assert_eq!(routed.handled, Handled::Yes);
	assert_eq!(screen.focused(), ["b"], "фокус переведён до доставки");
	assert_eq!(log.borrow().as_slice(), ["b:mouse:Down@3,3"]);
	screen.ui().route_mouse(&left_up(3, 3), AREA);

	// Нефокусируемый контрол принимает мышь, но фокус не меняет.
	let routed = screen.ui().route_mouse(&left_down(0, 5), AREA);
	assert_eq!(routed.handled, Handled::Yes);
	assert_eq!(screen.focused(), ["b"]);
	assert_eq!(log.borrow().last().unwrap(), "c:mouse:Down@0,5");
}

#[test]
fn mouse_outside_all_areas_and_over_stateless_widgets_is_not_handled() {
	let log = call_log();
	let mut screen = Screen::new(&log);
	screen.lay_out();
	for msg in [
		left_down(0, 7),
		left_down(10, 0),
		left_down(-1, 0),
		moved(50, 50),
	] {
		let routed = screen.ui().route_mouse(&msg, AREA);
		assert_eq!(routed.handled, Handled::No, "{msg:?}");
		assert!(routed.messages.is_empty());
	}
	assert!(log.borrow().is_empty(), "{:?}", log.borrow());
	// Фокус при этом выставлен первому кандидату, как при любом обращении к дереву.
	assert_eq!(screen.focused(), ["a"]);
}

#[test]
fn a_control_that_accepts_down_captures_the_pointer_until_up_or_cancel() {
	let log = call_log();
	let mut screen = Screen::new(&log);
	screen.lay_out();

	screen.ui().route_mouse(&left_down(1, 2), AREA);
	assert!(screen.b.node.captured);
	// Движение и отпускание над `a` идут захватившему `b`, отпускание снимает захват.
	screen.ui().route_mouse(&moved(1, 0), AREA);
	let routed = screen.ui().route_mouse(&left_up(1, 0), AREA);
	assert_eq!(routed.handled, Handled::Yes);
	assert!(!screen.b.node.captured);
	assert_eq!(
		log.borrow().as_slice(),
		["b:mouse:Down@1,2", "b:mouse:Move@1,0", "b:mouse:Up@1,0"]
	);
	// Без захвата движение над `a` идёт `a`.
	screen.ui().route_mouse(&moved(1, 0), AREA);
	assert_eq!(log.borrow().last().unwrap(), "a:mouse:Move@1,0");

	// Уход указателя из окна снимает захват как отмена.
	screen.ui().route_mouse(&left_down(1, 2), AREA);
	assert!(screen.b.node.captured);
	screen
		.ui()
		.route_mouse(&mouse(MouseKind::Cancel, None, -1, -1), AREA);
	assert!(!screen.b.node.captured);
	assert_eq!(log.borrow().last().unwrap(), "b:mouse:Cancel@-1,-1");

	// Контрол, ответивший «нет» на нажатие, захват не получает.
	screen.ui().route_mouse(&left_down(1, 0), AREA);
	assert!(!screen.a.node.captured);
}

#[test]
fn right_button_does_not_move_focus() {
	let log = call_log();
	let mut screen = Screen::new(&log);
	screen.lay_out();
	screen.ui().focus_first();
	screen.ui().route_mouse(
		&mouse(MouseKind::Down, Some(MouseButton::Right), 0, 2),
		AREA,
	);
	assert_eq!(screen.focused(), ["a"]);
	assert_eq!(log.borrow().last().unwrap(), "b:mouse:Down@0,2");
}

#[test]
fn rendering_lays_out_so_the_next_event_already_hits() {
	let log = call_log();
	let mut app = Screen::new(&log);
	let mut buf = screen(10, 8);
	app.ui().render(AREA, &mut buf);
	assert_eq!(app.c.node.area, Rect::new(0, 4, 10, 2));
	app.ui().route_mouse(&moved(9, 4), AREA);
	assert_eq!(log.borrow().as_slice(), ["c:mouse:Move@9,4"]);
}

#[test]
fn text_goes_to_the_focused_control_and_unhandled_returns_to_the_app() {
	let log = call_log();
	let mut app = Screen::new(&log);
	// `a` отвечает «нет» на всё — текст возвращается приложению.
	let routed = app.ui().text("ab");
	assert_eq!(routed.handled, Handled::No);
	assert_eq!(log.borrow().as_slice(), ["a:text:ab"]);

	app.ui().focus_next();
	let routed = app.ui().text("ю");
	assert_eq!(routed.handled, Handled::Yes);
	assert_eq!(log.borrow().last().unwrap(), "b:text:ю");
	assert_eq!(
		log.borrow().len(),
		2,
		"текст доставлен ровно одному контролу"
	);
}

#[test]
fn text_without_focus_candidates_is_not_handled() {
	let log = call_log();
	let mut app = Screen::new(&log);
	app.a.focusable = false;
	app.b.focusable = false;
	let routed = app.ui().text("x");
	assert_eq!(routed.handled, Handled::No);
	assert!(log.borrow().is_empty());
}
