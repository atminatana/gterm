//! Обход фокуса деревом описания (ADR-0026, ТЗ К1/К2): порядок объявления в глубину, кольцо,
//! обратный порядок, прозрачные контейнеры, сброс устаревших флагов, исчезновение контрола.

mod common;

use common::{CallLog, Msg, Stub, call_log, key_msg};
use gterm_widgets::input::Handled;
use gterm_widgets::widgets::Block;
use gterm_widgets::{Constraint, Layout, Ui};
use winit::keyboard::{Key, ModifiersState, NamedKey};

/// Четыре контрола: `a`, группа с `b` и `c` (`c` не фокусируется), `d`.
struct Screen {
	a: Stub,
	b: Stub,
	c: Stub,
	d: Stub,
}

impl Screen {
	fn new(log: &CallLog) -> Self {
		Self {
			a: Stub::new("a", true, log),
			b: Stub::new("b", true, log),
			c: Stub::new("c", false, log),
			d: Stub::new("d", true, log),
		}
	}

	fn ui(&mut self) -> Ui<'_, Msg> {
		Ui::split(
			Layout::vertical([
				Constraint::Length(1),
				Constraint::Length(4),
				Constraint::Length(1),
			]),
			vec![
				Ui::control(&mut self.a),
				Ui::group(
					Block::bordered(),
					Layout::vertical([Constraint::Length(1), Constraint::Length(1)]),
					vec![Ui::control(&mut self.b), Ui::control(&mut self.c)],
				),
				Ui::control(&mut self.d),
			],
		)
	}

	/// Имена контролов под фокусом (ожидается ровно один или ни одного).
	fn focused(&self) -> Vec<&'static str> {
		[&self.a, &self.b, &self.c, &self.d]
			.into_iter()
			.filter(|s| s.node.focused)
			.map(|s| s.name)
			.collect()
	}
}

fn tab(shift: bool) -> (Key, ModifiersState) {
	let modifiers = if shift {
		ModifiersState::SHIFT
	} else {
		ModifiersState::empty()
	};
	(Key::Named(NamedKey::Tab), modifiers)
}

#[test]
fn tab_walks_leaves_depth_first_in_a_ring_and_shift_tab_walks_back() {
	let log = call_log();
	let mut screen = Screen::new(&log);
	let (key, plain) = tab(false);
	let (_, shift) = tab(true);

	// Первое же обращение к дереву отдаёт фокус первому кандидату.
	screen
		.ui()
		.route_key(&key_msg(&Key::Named(NamedKey::ArrowDown), plain));
	assert_eq!(screen.focused(), ["a"]);

	let routed = screen.ui().route_key(&key_msg(&key, plain));
	assert_eq!(routed.handled, Handled::Yes);
	assert!(routed.messages.is_empty());
	assert_eq!(screen.focused(), ["b"], "контейнер прозрачен: после a — b");
	screen.ui().route_key(&key_msg(&key, plain));
	assert_eq!(screen.focused(), ["d"], "c не фокусируется — пропущен");
	screen.ui().route_key(&key_msg(&key, plain));
	assert_eq!(screen.focused(), ["a"], "кольцо");

	screen.ui().route_key(&key_msg(&key, shift));
	assert_eq!(
		screen.focused(),
		["d"],
		"Shift+Tab — строго обратный порядок"
	);
	screen.ui().route_key(&key_msg(&key, shift));
	assert_eq!(screen.focused(), ["b"]);

	// Tab до контролов не доходит.
	assert!(
		log.borrow().iter().all(|entry| !entry.contains("Tab")),
		"{:?}",
		log.borrow()
	);
}

#[test]
fn without_candidates_there_is_no_focus_and_keys_go_to_the_app() {
	let log = call_log();
	let mut screen = Screen::new(&log);
	for stub in [&mut screen.a, &mut screen.b, &mut screen.d] {
		stub.focusable = false;
	}
	let (key, plain) = tab(false);
	let routed = screen.ui().route_key(&key_msg(&key, plain));
	assert_eq!(
		routed.handled,
		Handled::Yes,
		"Tab берёт дерево даже без кандидатов"
	);
	assert!(screen.focused().is_empty());

	let routed = screen
		.ui()
		.route_key(&key_msg(&Key::Named(NamedKey::Enter), plain));
	assert_eq!(routed.handled, Handled::No);
	assert!(
		log.borrow().is_empty(),
		"никому не доставлено: {:?}",
		log.borrow()
	);
}

#[test]
fn stale_flags_collapse_to_the_first_marked_candidate() {
	let log = call_log();
	let mut screen = Screen::new(&log);
	screen.b.node.focused = true;
	screen.d.node.focused = true;
	screen.c.node.focused = true; // не кандидат — флаг устарел

	let plain = ModifiersState::empty();
	screen
		.ui()
		.route_key(&key_msg(&Key::Named(NamedKey::Enter), plain));
	assert_eq!(screen.focused(), ["b"]);
	assert_eq!(log.borrow().as_slice(), ["b:key:Named(Enter)"]);
}

#[test]
fn focus_on_a_control_missing_from_the_description_moves_to_the_first_candidate() {
	let log = call_log();
	let mut screen = Screen::new(&log);
	screen.d.node.focused = true;
	let plain = ModifiersState::empty();

	// Экран без `d`: его флаг крейт не видит и не трогает, фокус достаётся первому.
	let mut without_d = Ui::split(
		Layout::vertical([Constraint::Length(1), Constraint::Length(1)]),
		vec![Ui::control(&mut screen.a), Ui::control(&mut screen.b)],
	);
	without_d.route_key(&key_msg(&Key::Named(NamedKey::Enter), plain));
	drop(without_d);
	assert!(screen.a.node.focused);
	assert!(!screen.b.node.focused);
	assert!(
		screen.d.node.focused,
		"флаг отсутствующего контрола остаётся у его состояния"
	);
	assert_eq!(log.borrow().as_slice(), ["a:key:Named(Enter)"]);

	// Вернулся в описание с устаревшим флагом — побеждает первый по порядку (`a`).
	screen
		.ui()
		.route_key(&key_msg(&Key::Named(NamedKey::Enter), plain));
	assert_eq!(screen.focused(), ["a"]);
}

#[test]
fn a_control_that_stops_being_focusable_loses_focus() {
	let log = call_log();
	let mut screen = Screen::new(&log);
	screen.b.node.focused = true;
	screen.b.focusable = false;
	let plain = ModifiersState::empty();
	screen
		.ui()
		.route_key(&key_msg(&Key::Named(NamedKey::Enter), plain));
	assert_eq!(screen.focused(), ["a"]);
	assert!(!screen.b.node.focused);
}

#[test]
fn explicit_focus_calls_follow_the_same_order() {
	let log = call_log();
	let mut screen = Screen::new(&log);
	screen.ui().focus_prev();
	assert_eq!(screen.focused(), ["d"], "назад без фокуса — последний");
	screen.ui().focus_first();
	assert_eq!(screen.focused(), ["a"]);
	screen.ui().focus_next();
	assert_eq!(screen.focused(), ["b"]);
}
