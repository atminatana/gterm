//! Карта «сочетание → сообщение» (ADR-0029, ТЗ К1–К5): порядок привязок и ошибка разбора,
//! срабатывание на нажатие, аккорды по полной истории, контрол под фокусом раньше карты,
//! карта модального слоя вместо карты экрана.

mod common;

use common::{CallLog, Msg, Stub, call_log};
use gterm::hotkey::{Hotkey, HotkeyTracker, Key, ParseErrorKind};
use gterm_widgets::input::{Handled, KeyMsg, Routed};
use gterm_widgets::widgets::{TextInput, TextInputState};
use gterm_widgets::{Constraint, HotkeyMap, Layout, Modal, Ui};
use winit::event::ElementState;
use winit::keyboard::{Key as LogicalKey, ModifiersState, NamedKey};

const PLAIN: ModifiersState = ModifiersState::empty();
const CTRL: ModifiersState = ModifiersState::CONTROL;

/// Экран: поле ввода (фокус по умолчанию) и свой контрол, отвечающий «нет»; по желанию —
/// модальный слой со своим контролом, картой и привязкой `Esc`.
struct App {
	tracker: HotkeyTracker,
	keys: HotkeyMap<Msg>,
	field: TextInputState,
	stub: Stub,
	dialog: bool,
	inner: Stub,
	layer_keys: Option<HotkeyMap<Msg>>,
	layer_escape: bool,
}

impl App {
	fn new(log: &CallLog, keys: HotkeyMap<Msg>) -> Self {
		Self {
			tracker: HotkeyTracker::new(),
			keys,
			field: TextInputState::new(),
			stub: Stub::new("stub", true, log),
			dialog: false,
			inner: Stub::new("inner", true, log),
			layer_keys: None,
			layer_escape: false,
		}
	}

	fn ui(&mut self) -> Ui<'_, Msg> {
		let screen = Ui::split(
			Layout::vertical([Constraint::Length(1), Constraint::Length(1)]),
			vec![
				Ui::stateful(TextInput::new(), &mut self.field).on_change(Msg::Text),
				Ui::control(&mut self.stub),
			],
		)
		.hotkeys(&mut self.tracker, &self.keys);
		if !self.dialog {
			return screen;
		}
		let mut modal = Modal::centered(6, 2).content(Ui::control(&mut self.inner));
		if let Some(keys) = &self.layer_keys {
			modal = modal.hotkeys(keys);
		}
		if self.layer_escape {
			modal = modal.on_escape(Msg::Custom("escape"));
		}
		screen.modal(modal)
	}

	fn event(
		&mut self,
		name: &str,
		state: ElementState,
		repeat: bool,
		modifiers: ModifiersState,
		text: Option<&str>,
	) -> Routed<Msg> {
		let logical = logical_key(name);
		let msg = KeyMsg {
			key: &logical,
			text,
			modifiers,
		};
		self.ui()
			.route_key_event(Some(physical_key(name)), state, repeat, &msg)
	}

	/// Фокус на свой контрол: `focus_next` без установленного фокуса дал бы первый — поле.
	fn focus_stub(&mut self) {
		self.ui().focus_first();
		self.ui().focus_next();
		assert!(self.stub.node.focused);
	}

	/// Нажатие без набираемого текста (модификатор, сочетание, именованная клавиша).
	fn down(&mut self, name: &str, modifiers: ModifiersState) -> Routed<Msg> {
		self.event(name, ElementState::Pressed, false, modifiers, None)
	}

	/// Нажатие буквы без модификаторов: несёт набираемый текст, как `KeyEvent.text`.
	fn type_key(&mut self, name: &str) -> Routed<Msg> {
		let text = name.to_lowercase();
		self.event(name, ElementState::Pressed, false, PLAIN, Some(&text))
	}

	fn up(&mut self, name: &str) -> Routed<Msg> {
		self.event(name, ElementState::Released, false, PLAIN, None)
	}

	fn release_all(&mut self, names: &[&str]) {
		for name in names {
			self.up(name);
		}
		assert!(!self.tracker.is_active(), "все клавиши отпущены");
	}
}

fn physical_key(name: &str) -> Key {
	Key::parse_name(name).unwrap_or_else(|| panic!("неизвестная клавиша {name}"))
}

fn logical_key(name: &str) -> LogicalKey {
	match name {
		"Ctrl" => LogicalKey::Named(NamedKey::Control),
		"Esc" => LogicalKey::Named(NamedKey::Escape),
		letter => LogicalKey::Character(letter.to_lowercase().into()),
	}
}

fn map(bindings: &[(&str, &'static str)]) -> HotkeyMap<Msg> {
	bindings
		.iter()
		.fold(HotkeyMap::new(), |map, (pattern, message)| {
			map.bind_str(pattern, Msg::Custom(message))
				.unwrap_or_else(|e| panic!("{pattern}: {e}"))
		})
}

fn custom(routed: &Routed<Msg>) -> Vec<Msg> {
	routed.messages.clone()
}

fn fired(message: &'static str) -> Vec<Msg> {
	vec![Msg::Custom(message)]
}

#[test]
fn map_keeps_declaration_order_and_reports_parse_errors_with_position() {
	let empty = HotkeyMap::<Msg>::new();
	assert!(empty.is_empty());
	let ctrl_s: Hotkey = "Ctrl+S".parse().unwrap();
	assert_eq!(empty.message_for(&ctrl_s, &ctrl_s), None);

	let keys = map(&[("Ctrl+S", "first"), ("Ctrl+S|Ctrl+K", "second")]);
	assert_eq!(keys.len(), 2);
	assert_eq!(
		keys.message_for(&ctrl_s, &ctrl_s),
		Some(Msg::Custom("first")),
		"из двух совпавших — объявленная раньше"
	);
	let ctrl_k: Hotkey = "Ctrl+K".parse().unwrap();
	assert_eq!(
		keys.message_for(&ctrl_k, &ctrl_k),
		Some(Msg::Custom("second"))
	);

	let error = HotkeyMap::new()
		.bind_str("Ctrl+Ж", Msg::Pressed)
		.unwrap_err();
	assert_eq!(error.position, 5);
	assert_eq!(error.kind, ParseErrorKind::UnknownKey("Ж".to_owned()));
}

#[test]
fn a_simple_combination_fires_on_press_and_again_without_releasing_the_modifier() {
	let log = call_log();
	let mut app = App::new(
		&log,
		map(&[("Ctrl+S", "save"), ("Ctrl+C", "copy"), ("Ctrl+V", "paste")]),
	);
	app.focus_stub();

	let routed = app.down("Ctrl", CTRL);
	assert_eq!(routed.handled, Handled::No, "сам модификатор — не привязка");
	assert!(routed.messages.is_empty());

	let routed = app.down("S", CTRL);
	assert_eq!(routed.handled, Handled::Yes);
	assert_eq!(custom(&routed), fired("save"));

	// Автоповтор и отпускание привязку не вызывают.
	let routed = app.event("S", ElementState::Pressed, true, CTRL, None);
	assert_eq!(routed.handled, Handled::No);
	assert!(routed.messages.is_empty());
	let routed = app.up("S");
	assert_eq!(routed.handled, Handled::No);
	assert!(routed.messages.is_empty());

	// `Ctrl` всё ещё удержан: история — `Ctrl+[S,S]`, `Ctrl+[S,S,C]` …, но цепочка удержанных
	// — простое сочетание.
	assert_eq!(custom(&app.down("S", CTRL)), fired("save"));
	app.up("S");
	assert_eq!(custom(&app.down("C", CTRL)), fired("copy"));
	app.up("C");
	assert_eq!(custom(&app.down("V", CTRL)), fired("paste"));
	assert_eq!(app.tracker.full().to_string(), "Ctrl+[S,S,C,V]");
	app.release_all(&["V", "Ctrl"]);
}

#[test]
fn a_chord_fires_on_its_last_press_and_the_full_history_wins_over_held_keys() {
	let log = call_log();
	// `Ctrl+U` объявлена раньше аккорда, но аккорд — совпадение полной истории.
	let mut app = App::new(
		&log,
		map(&[
			("Ctrl+U", "upper"),
			("Ctrl+[K,U]", "chord"),
			("K|U", "either"),
		]),
	);
	app.focus_stub();

	app.down("Ctrl", CTRL);
	let routed = app.down("K", CTRL);
	assert_eq!(routed.handled, Handled::No, "`Ctrl+K` не привязана");
	app.up("K");
	assert_eq!(custom(&app.down("U", CTRL)), fired("chord"));
	app.release_all(&["U", "Ctrl"]);

	// Без `K` перед `U` — простое сочетание.
	app.down("Ctrl", CTRL);
	assert_eq!(custom(&app.down("U", CTRL)), fired("upper"));
	app.release_all(&["U", "Ctrl"]);

	// Альтернативы образца: `K|U`.
	assert_eq!(custom(&app.down("K", PLAIN)), fired("either"));
	app.release_all(&["K"]);
	assert_eq!(custom(&app.down("U", PLAIN)), fired("either"));
	app.release_all(&["U"]);
}

#[test]
fn the_focused_control_goes_first_and_only_an_unhandled_press_reaches_the_map() {
	let log = call_log();
	let mut app = App::new(&log, map(&[("K", "k"), ("Ctrl+S", "save")]));

	// Фокус в поле ввода: буква набирается, привязка `K` молчит.
	let routed = app.type_key("K");
	assert_eq!(routed.handled, Handled::Yes);
	assert_eq!(routed.messages, vec![Msg::Text("k".to_owned())]);
	app.release_all(&["K"]);

	// `Ctrl+S` поле не принимает (при `Ctrl` текст не набирается) — срабатывает привязка.
	app.down("Ctrl", CTRL);
	let routed = app.event("S", ElementState::Pressed, false, CTRL, Some("\u{13}"));
	assert_eq!(routed.handled, Handled::Yes);
	assert_eq!(custom(&routed), fired("save"));
	assert_eq!(app.field.value(), "k");
	app.release_all(&["S", "Ctrl"]);

	// Фокус на контроле, не принимающем букву: клавиша и текст предложены ему, затем карта.
	app.focus_stub();
	let routed = app.type_key("K");
	assert_eq!(routed.handled, Handled::Yes);
	assert_eq!(custom(&routed), fired("k"));
	assert_eq!(
		log.borrow().as_slice(),
		["stub:key:Character(\"k\")", "stub:text:k"]
	);
	app.release_all(&["K"]);

	// Контрол принял нажатие — привязки нет, но трекер нажатие учёл.
	app.stub.answer = Handled::Yes;
	let routed = app.type_key("K");
	assert_eq!(routed.handled, Handled::Yes);
	assert!(routed.messages.is_empty());
	assert_eq!(app.tracker.full().to_string(), "K");
	app.release_all(&["K"]);
}

#[test]
fn under_a_modal_layer_only_its_own_map_works() {
	let log = call_log();
	let mut app = App::new(&log, map(&[("P", "open"), ("Q", "quit")]));
	app.focus_stub();
	assert_eq!(custom(&app.down("P", PLAIN)), fired("open"));
	app.release_all(&["P"]);

	// Слой без карты: карта экрана молчит, трекер кормится.
	app.dialog = true;
	let routed = app.down("P", PLAIN);
	assert_eq!(routed.handled, Handled::No);
	assert!(routed.messages.is_empty());
	assert_eq!(app.tracker.current().to_string(), "P");
	app.release_all(&["P"]);

	// Своя карта слоя: `P` — её привязка, `Q` есть только у экрана и не действует.
	app.layer_keys = Some(map(&[("P", "close"), ("Esc", "layer-esc")]));
	assert_eq!(custom(&app.down("P", PLAIN)), fired("close"));
	app.release_all(&["P"]);
	let routed = app.down("Q", PLAIN);
	assert_eq!(routed.handled, Handled::No);
	assert!(routed.messages.is_empty());
	app.release_all(&["Q"]);

	// `Esc`: без привязки `on_escape` — карта слоя, с привязкой — только она.
	assert_eq!(custom(&app.down("Esc", PLAIN)), fired("layer-esc"));
	app.release_all(&["Esc"]);
	app.layer_escape = true;
	assert_eq!(custom(&app.down("Esc", PLAIN)), fired("escape"));
	app.release_all(&["Esc"]);

	// Клавиши при слое получал его контрол, а не контролы экрана.
	assert!(log.borrow().iter().skip(1).all(|e| e.starts_with("inner:")));
}

#[test]
fn a_map_attached_to_a_nested_description_is_lifted_to_the_root() {
	let log = call_log();
	let mut tracker = HotkeyTracker::new();
	let keys = map(&[("F2", "rename")]);
	let mut stub = Stub::new("stub", true, &log);
	let f2 = LogicalKey::Named(NamedKey::F2);
	let msg = KeyMsg {
		key: &f2,
		text: None,
		modifiers: PLAIN,
	};
	let routed = Ui::split(
		Layout::vertical([Constraint::Length(1)]),
		vec![Ui::control(&mut stub).hotkeys(&mut tracker, &keys)],
	)
	.route_key_event(Some(physical_key("F2")), ElementState::Pressed, false, &msg);
	assert_eq!(custom(&routed), fired("rename"));
}

#[test]
fn without_a_map_key_routing_is_unchanged() {
	let log = call_log();
	let mut stub = Stub::new("stub", true, &log).answering(Handled::Yes);
	let enter = LogicalKey::Named(NamedKey::Enter);
	let msg = KeyMsg {
		key: &enter,
		text: None,
		modifiers: PLAIN,
	};
	let key = Some(physical_key("Enter"));
	let routed =
		Ui::<Msg>::control(&mut stub).route_key_event(key, ElementState::Released, false, &msg);
	assert_eq!(routed.handled, Handled::No);
	assert!(log.borrow().is_empty(), "отпускание контролам не идёт");
	let routed =
		Ui::<Msg>::control(&mut stub).route_key_event(key, ElementState::Pressed, false, &msg);
	assert_eq!(routed.handled, Handled::Yes);
	assert!(routed.messages.is_empty());
	assert_eq!(log.borrow().as_slice(), ["stub:key:Named(Enter)"]);
}
