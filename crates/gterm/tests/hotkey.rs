//! Комбинации клавиш (ADR-0028, ТЗ К1–К4): запись и разбор, канонизация модификаторов,
//! образцы с `|`, трекер по сценариям владельца, клавиши из физических кодов.

use gterm::hotkey::{Hotkey, HotkeyEvent, HotkeyTracker, Key, ParseErrorKind};
use winit::event::ElementState;
use winit::keyboard::KeyCode;

fn parse(text: &str) -> Hotkey {
	text.parse().unwrap_or_else(|e| panic!("{text:?}: {e}"))
}

const K: Key = Key::Code(KeyCode::KeyK);
const U: Key = Key::Code(KeyCode::KeyU);
const S: Key = Key::Code(KeyCode::KeyS);
const SPACE: Key = Key::Code(KeyCode::Space);

#[test]
fn examples_from_the_statement_round_trip_in_canonical_form() {
	for text in [
		"Ctrl+K",
		"Ctrl+[K,U]",
		"Space+[Ctrl+[K,U]]",
		"K|U",
		"K,U",
		"Ctrl+Shift+K",
		"Ctrl+[K|U]",
		"Ctrl+S|Super+S",
		"Ctrl+[K,K]",
		"Alt+F4",
		"Ctrl+BracketLeft",
		"Shift+Comma,Esc",
		"Ctrl",
	] {
		let hotkey = parse(text);
		assert_eq!(hotkey.to_string(), text, "канонический вид");
		assert_eq!(parse(&hotkey.to_string()), hotkey, "круговое свойство");
	}
}

#[test]
fn names_are_case_insensitive_and_modifiers_are_canonically_ordered() {
	assert_eq!(parse("ctrl + k").to_string(), "Ctrl+K");
	assert_eq!(parse("Shift+Ctrl+K"), parse("Ctrl+Shift+K"));
	assert_eq!(
		parse("Super+Shift+Alt+Ctrl+A").to_string(),
		"Ctrl+Alt+Shift+Super+A"
	);
	assert_eq!(parse("Shift+Ctrl").to_string(), "Ctrl+Shift");
	assert_eq!(
		parse("Shift+Ctrl+[K,U]").to_string(),
		"Ctrl+Shift+[K,U]",
		"модификаторы перед группой тоже упорядочены"
	);
	// Порядок прочих клавиш значим.
	assert_ne!(parse("K,U"), parse("U,K"));
	assert_ne!(parse("Ctrl+[K,U]"), parse("Ctrl+[U,K]"));
	// Имена из таблицы `KeyCode`.
	assert_eq!(parse("digit1").to_string(), "1");
	assert_eq!(parse("ArrowUp"), Hotkey::key(Key::Code(KeyCode::ArrowUp)));
	assert_eq!(parse("Up").to_string(), "Up");
	assert_eq!(parse("Escape").to_string(), "Esc");
	assert_eq!(parse("NumpadAdd").to_string(), "NumpadAdd");
}

#[test]
fn parse_errors_carry_the_position() {
	let cases = [
		("Ctrl+", 5, ParseErrorKind::ExpectedKey),
		(",K", 0, ParseErrorKind::ExpectedKey),
		("Ctrl+[]", 6, ParseErrorKind::ExpectedKey),
		("Ctrl+[K,U", 5, ParseErrorKind::UnclosedBracket),
		("K]", 1, ParseErrorKind::UnexpectedChar(']')),
		("K-U", 1, ParseErrorKind::UnexpectedChar('-')),
		("Ctrl+Ж", 5, ParseErrorKind::UnknownKey("Ж".to_owned())),
		(
			"Ctrl+Plus",
			5,
			ParseErrorKind::UnknownKey("Plus".to_owned()),
		),
		("", 0, ParseErrorKind::ExpectedKey),
		("K||U", 2, ParseErrorKind::ExpectedKey),
	];
	for (text, position, kind) in cases {
		let error = text.parse::<Hotkey>().expect_err(text);
		assert_eq!((error.position, error.kind), (position, kind), "{text:?}");
	}
	assert!(
		"Ctrl+Ж"
			.parse::<Hotkey>()
			.unwrap_err()
			.to_string()
			.contains("Ж"),
		"сообщение называет клавишу"
	);
}

#[test]
fn matching_handles_alternatives_at_any_level_and_ignores_modifier_order() {
	let recorded = parse("Ctrl+[K,U]");
	assert!(recorded.matches(&parse("Ctrl+[K,U]")));
	assert!(recorded.matches(&parse("Ctrl+[K,U]|Alt+X")));
	assert!(
		recorded.matches(&parse("Ctrl+[K,U|Y]")),
		"альтернатива внутри группы"
	);
	assert!(
		!recorded.matches(&parse("Ctrl+[U,K]")),
		"порядок клавиш значим"
	);
	assert!(!recorded.matches(&parse("Ctrl+K")));
	assert!(!recorded.matches(&parse("Ctrl+[K,U,Y]")));

	assert!(Hotkey::chain(&[Key::Shift, Key::Ctrl, K]).matches(&parse("Ctrl+Shift+K")));
	assert!(Hotkey::chain(&[K]).matches(&parse("K|U")));
	assert!(Hotkey::chain(&[U]).matches(&parse("K|U")));
	assert!(!Hotkey::chain(&[S]).matches(&parse("K|U")));
	assert!(Hotkey::empty().matches(&Hotkey::empty()));
	assert!(!Hotkey::empty().matches(&parse("K")));
	assert_eq!(parse("Ctrl+Shift+K").keys(), vec![Key::Ctrl, Key::Shift, K]);
}

/// Нажать или отпустить без автоповтора.
fn press(tracker: &mut HotkeyTracker, key: Key) -> Option<HotkeyEvent> {
	tracker.feed_key(key, ElementState::Pressed, false)
}

fn release(tracker: &mut HotkeyTracker, key: Key) -> Option<HotkeyEvent> {
	tracker.feed_key(key, ElementState::Released, false)
}

fn event(text: &str, complete: bool) -> Option<HotkeyEvent> {
	Some(HotkeyEvent {
		hotkey: parse(text),
		complete,
	})
}

#[test]
fn tracker_follows_the_owner_scenario_step_by_step() {
	let mut tracker = HotkeyTracker::new();
	assert!(!tracker.is_active());
	assert!(tracker.full().is_empty() && tracker.current().is_empty());

	assert_eq!(press(&mut tracker, Key::Ctrl), None);
	assert_eq!(tracker.full().to_string(), "Ctrl");
	assert_eq!(tracker.current().to_string(), "Ctrl");
	assert!(tracker.is_active());

	assert_eq!(press(&mut tracker, K), None);
	assert_eq!(tracker.full().to_string(), "Ctrl+K");
	assert_eq!(tracker.current().to_string(), "Ctrl+K");

	assert_eq!(release(&mut tracker, K), event("Ctrl+K", false));
	assert_eq!(tracker.full().to_string(), "Ctrl+K");
	assert_eq!(tracker.current().to_string(), "Ctrl");

	assert_eq!(press(&mut tracker, U), None);
	assert_eq!(tracker.full().to_string(), "Ctrl+[K,U]");
	assert_eq!(tracker.current().to_string(), "Ctrl+U");

	assert_eq!(release(&mut tracker, Key::Ctrl), event("Ctrl+U", false));
	assert_eq!(tracker.full().to_string(), "Ctrl+[K,U]");
	assert_eq!(tracker.current().to_string(), "U");

	assert_eq!(release(&mut tracker, U), event("Ctrl+[K,U]", true));
	assert!(!tracker.is_active());
	assert!(tracker.full().is_empty(), "после завершения трекер пуст");

	// Следующее нажатие — новая жизнь.
	press(&mut tracker, S);
	assert_eq!(tracker.full().to_string(), "S");
	assert_eq!(release(&mut tracker, S), event("S", true));
}

#[test]
fn nested_holds_repeats_and_noise_are_recorded_as_specified() {
	let mut tracker = HotkeyTracker::new();
	press(&mut tracker, SPACE);
	press(&mut tracker, Key::Ctrl);
	press(&mut tracker, K);
	assert_eq!(release(&mut tracker, K), event("Space+Ctrl+K", false));
	press(&mut tracker, U);
	assert_eq!(tracker.full().to_string(), "Space+[Ctrl+[K,U]]");
	assert_eq!(release(&mut tracker, U), event("Space+Ctrl+U", false));
	assert_eq!(release(&mut tracker, Key::Ctrl), event("Space+Ctrl", false));
	assert_eq!(
		release(&mut tracker, SPACE),
		event("Space+[Ctrl+[K,U]]", true)
	);

	// Повтор той же клавиши записывается как есть (О2); модификаторы — в каноническом порядке.
	press(&mut tracker, Key::Shift);
	press(&mut tracker, Key::Ctrl);
	press(&mut tracker, K);
	release(&mut tracker, K);
	press(&mut tracker, K);
	release(&mut tracker, K);
	release(&mut tracker, Key::Ctrl);
	assert_eq!(
		release(&mut tracker, Key::Shift),
		event("Ctrl+Shift+[K,K]", true)
	);

	// Автоповтор, повторное нажатие удержанной и отпускание чужой клавиши ничего не меняют.
	press(&mut tracker, Key::Ctrl);
	assert_eq!(
		tracker.feed_key(K, ElementState::Pressed, true),
		None,
		"автоповтор"
	);
	assert_eq!(tracker.full().to_string(), "Ctrl");
	press(&mut tracker, K);
	assert_eq!(press(&mut tracker, K), None, "повторное нажатие удержанной");
	assert_eq!(tracker.full().to_string(), "Ctrl+K");
	assert_eq!(release(&mut tracker, U), None, "чужое отпускание");
	assert_eq!(tracker.current().to_string(), "Ctrl+K");
	tracker.reset();
	assert!(!tracker.is_active() && tracker.full().is_empty());
	assert_eq!(
		release(&mut tracker, K),
		None,
		"после сброса отпускания не считаются"
	);
}

#[test]
fn keys_come_from_physical_codes_and_merge_left_and_right_modifiers() {
	assert_eq!(Key::from_code(KeyCode::ControlLeft), Key::Ctrl);
	assert_eq!(Key::from_code(KeyCode::ControlRight), Key::Ctrl);
	assert_eq!(Key::from_code(KeyCode::AltRight), Key::Alt);
	assert_eq!(Key::from_code(KeyCode::ShiftLeft), Key::Shift);
	assert_eq!(Key::from_code(KeyCode::SuperRight), Key::Super);
	assert_eq!(Key::from_code(KeyCode::KeyK), K);
	assert!(Key::Ctrl.is_modifier() && !K.is_modifier());
	assert_eq!(K.name(), "K");
	assert_eq!(Key::Code(KeyCode::Digit7).name(), "7");
	assert_eq!(Key::Code(KeyCode::ArrowLeft).name(), "Left");
	assert_eq!(Key::Code(KeyCode::Escape).name(), "Esc");
	assert_eq!(Key::Code(KeyCode::Comma).name(), "Comma");
	assert_eq!(Key::parse_name("comma"), Some(Key::Code(KeyCode::Comma)));
	assert_eq!(Key::parse_name("super"), Some(Key::Super));
	assert_eq!(Key::parse_name("Ж"), None);
}
