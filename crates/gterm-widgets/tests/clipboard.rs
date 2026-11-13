//! Буфер обмена и команды правки (ADR-0030, ТЗ К4, К5): распознавание по физической клавише,
//! копирование, вырезание и вставка через `MemoryClipboard`, классические сочетания, вставка
//! многострочного текста, поглощение команд полем (карта сочетаний молчит), поле без
//! подключённого буфера, поле в модальном слое.

mod common;

use common::{CallLog, Msg, Stub, call_log};
use gterm::hotkey::{HotkeyTracker, Key};
use gterm_widgets::input::{Handled, KeyMsg, Routed};
use gterm_widgets::widgets::{TextInput, TextInputState};
use gterm_widgets::{
	Clipboard, Constraint, EditCommand, HotkeyMap, Layout, MemoryClipboard, Modal, Ui,
};
use winit::event::ElementState;
use winit::keyboard::{Key as LogicalKey, ModifiersState, NamedKey};

const CTRL: ModifiersState = ModifiersState::CONTROL;
const SHIFT: ModifiersState = ModifiersState::SHIFT;

/// Экран: поле ввода (фокус по умолчанию) и свой контрол без поддержки правки; карта экрана
/// привязывает те же сочетания, что и команды правки. По желанию — поле в модальном слое и
/// описание без подключённого буфера.
struct App {
	tracker: HotkeyTracker,
	keys: HotkeyMap<Msg>,
	clipboard: MemoryClipboard,
	has_clipboard: bool,
	field: TextInputState,
	stub: Stub,
	dialog: bool,
	dialog_field: TextInputState,
}

impl App {
	fn new(log: &CallLog) -> Self {
		let keys = ["Ctrl+C", "Ctrl+X", "Ctrl+V", "Ctrl+A"]
			.into_iter()
			.fold(HotkeyMap::new(), |map, pattern| {
				map.bind_str(pattern, Msg::Custom(pattern)).unwrap()
			});
		Self {
			tracker: HotkeyTracker::new(),
			keys,
			clipboard: MemoryClipboard::new(),
			has_clipboard: true,
			field: TextInputState::new(),
			stub: Stub::new("stub", true, log),
			dialog: false,
			dialog_field: TextInputState::new(),
		}
	}

	fn ui(&mut self) -> Ui<'_, Msg> {
		let mut screen = Ui::split(
			Layout::vertical([Constraint::Length(1), Constraint::Length(1)]),
			vec![
				Ui::stateful(TextInput::new(), &mut self.field).on_change(Msg::Text),
				Ui::control(&mut self.stub),
			],
		)
		.hotkeys(&mut self.tracker, &self.keys);
		if self.has_clipboard {
			screen = screen.clipboard(&mut self.clipboard);
		}
		if !self.dialog {
			return screen;
		}
		screen.modal(
			Modal::centered(8, 1).content(
				Ui::stateful(TextInput::new(), &mut self.dialog_field).on_change(Msg::Text),
			),
		)
	}

	fn event(
		&mut self,
		physical: &str,
		state: ElementState,
		modifiers: ModifiersState,
	) -> Routed<Msg> {
		// Логическая клавиша — кириллица: команды распознаются по физической.
		let logical = match physical {
			"Ctrl" => LogicalKey::Named(NamedKey::Control),
			"Shift" => LogicalKey::Named(NamedKey::Shift),
			"Insert" => LogicalKey::Named(NamedKey::Insert),
			"Delete" => LogicalKey::Named(NamedKey::Delete),
			_ => LogicalKey::Character("ы".into()),
		};
		let msg = KeyMsg {
			key: &logical,
			text: None,
			modifiers,
		};
		let key = Key::parse_name(physical).unwrap();
		self.ui().route_key_event(Some(key), state, false, &msg)
	}

	/// Сочетание целиком: модификатор, клавиша, отпускания; ответ — на нажатие клавиши.
	fn combo(&mut self, modifier: &str, key: &str) -> Routed<Msg> {
		let modifiers = if modifier == "Ctrl" { CTRL } else { SHIFT };
		self.event(modifier, ElementState::Pressed, modifiers);
		let routed = self.event(key, ElementState::Pressed, modifiers);
		self.event(key, ElementState::Released, modifiers);
		self.event(modifier, ElementState::Released, ModifiersState::empty());
		routed
	}

	fn focus_stub(&mut self) {
		self.ui().focus_first();
		self.ui().focus_next();
		assert!(self.stub.node.focused);
	}
}

fn absorbed(routed: &Routed<Msg>) {
	assert_eq!(routed.handled, Handled::Yes);
	assert!(routed.messages.is_empty(), "{:?}", routed.messages);
}

#[test]
fn commands_come_from_physical_keys_with_exact_modifiers() {
	let key = |name: &str| Key::parse_name(name).unwrap();
	let cases = [
		("C", CTRL, Some(EditCommand::Copy)),
		("Insert", CTRL, Some(EditCommand::Copy)),
		("X", CTRL, Some(EditCommand::Cut)),
		("Delete", SHIFT, Some(EditCommand::Cut)),
		("V", CTRL, Some(EditCommand::Paste)),
		("Insert", SHIFT, Some(EditCommand::Paste)),
		("A", CTRL, Some(EditCommand::SelectAll)),
		("C", ModifiersState::empty(), None),
		("C", CTRL | SHIFT, None),
		("C", CTRL | ModifiersState::ALT, None),
		("Delete", CTRL, None),
		("Insert", ModifiersState::empty(), None),
		("Ctrl", CTRL, None),
	];
	for (name, modifiers, expected) in cases {
		assert_eq!(
			EditCommand::from_key(key(name), modifiers),
			expected,
			"{name} {modifiers:?}"
		);
	}
}

#[test]
fn copy_cut_and_paste_go_through_the_clipboard_and_report_changes() {
	let log = call_log();
	let mut app = App::new(&log);
	app.field.set_value("hello world");

	absorbed(&app.combo("Ctrl", "A"));
	assert_eq!(app.field.selection(), Some((0, 11)));

	absorbed(&app.combo("Ctrl", "C"));
	assert_eq!(app.clipboard.text(), Some("hello world"));
	assert_eq!(app.field.value(), "hello world");

	let routed = app.combo("Ctrl", "X");
	assert_eq!(routed.handled, Handled::Yes);
	assert_eq!(routed.messages, vec![Msg::Text(String::new())]);
	assert_eq!(app.field.value(), "");

	let routed = app.combo("Ctrl", "V");
	assert_eq!(routed.messages, vec![Msg::Text("hello world".to_owned())]);
	assert_eq!((app.field.value(), app.field.cursor), ("hello world", 11));

	// Вставка заменяет выделенное.
	app.clipboard.set("X");
	app.combo("Ctrl", "A");
	let routed = app.combo("Ctrl", "V");
	assert_eq!(routed.messages, vec![Msg::Text("X".to_owned())]);

	// Пустое выделение: буфер не трогается, команда всё равно поглощена.
	app.clipboard.set("keep");
	absorbed(&app.combo("Ctrl", "C"));
	absorbed(&app.combo("Ctrl", "X"));
	assert_eq!(app.clipboard.text(), Some("keep"));
	assert_eq!(app.field.value(), "X");
}

#[test]
fn classic_combinations_work_and_pasted_line_breaks_become_spaces() {
	let log = call_log();
	let mut app = App::new(&log);
	app.field.set_value("abc");
	app.field.select_all();

	absorbed(&app.combo("Ctrl", "Insert"));
	assert_eq!(app.clipboard.text(), Some("abc"));
	app.combo("Shift", "Delete");
	assert_eq!(app.field.value(), "");
	app.combo("Shift", "Insert");
	assert_eq!(app.field.value(), "abc");

	app.field.clear();
	app.clipboard.set("a\r\nb\tc\u{7}d\ne");
	app.combo("Ctrl", "V");
	assert_eq!(app.field.value(), "a b cd e");
}

#[test]
fn the_field_absorbs_edit_commands_and_other_controls_let_them_reach_the_map() {
	let log = call_log();
	let mut app = App::new(&log);
	app.field.set_value("text");

	// Фокус в поле: привязки карты с теми же сочетаниями молчат.
	for key in ["C", "X", "V", "A"] {
		absorbed(&app.combo("Ctrl", key));
	}

	// Контрол без поддержки правки: команда не принята, срабатывает привязка.
	app.focus_stub();
	for (key, pattern) in [
		("C", "Ctrl+C"),
		("X", "Ctrl+X"),
		("V", "Ctrl+V"),
		("A", "Ctrl+A"),
	] {
		let routed = app.combo("Ctrl", key);
		assert_eq!(routed.messages, vec![Msg::Custom(pattern)], "{key}");
	}
	assert_eq!(app.field.value(), "text");
}

#[test]
fn without_a_connected_clipboard_the_field_still_absorbs_and_keeps_its_text() {
	let log = call_log();
	let mut app = App::new(&log);
	app.has_clipboard = false;
	app.clipboard.set("outside");
	app.field.set_value("text");

	absorbed(&app.combo("Ctrl", "A"));
	assert_eq!(app.field.selected_text(), "text");
	absorbed(&app.combo("Ctrl", "C"));
	// Вырезание без буфера текст не удаляет — иначе он пропал бы.
	absorbed(&app.combo("Ctrl", "X"));
	absorbed(&app.combo("Ctrl", "V"));
	assert_eq!(app.field.value(), "text");
	assert_eq!(app.clipboard.text(), Some("outside"));
}

#[test]
fn a_field_in_a_modal_layer_uses_the_clipboard_of_the_screen() {
	let log = call_log();
	let mut app = App::new(&log);
	app.field.set_value("screen");
	app.dialog = true;
	app.dialog_field.set_value("layer");

	absorbed(&app.combo("Ctrl", "A"));
	absorbed(&app.combo("Ctrl", "C"));
	assert_eq!(app.clipboard.text(), Some("layer"));
	app.dialog_field.move_end();
	let routed = app.combo("Ctrl", "V");
	assert_eq!(routed.messages, vec![Msg::Text("layerlayer".to_owned())]);
	assert_eq!(
		app.field.value(),
		"screen",
		"поле экрана под слоем не тронуто"
	);
	assert_eq!(app.field.selection(), None);
}
