//! Выпадающий список (ADR-0027, ТЗ К4): закрытое поле с маркером, раскрытие клавишами и
//! кликом, слой под полем (над — при нехватке места), выбор закрывает и публикует `on_select`
//! только при изменении, `Esc` и клик мимо сворачивают без сообщений, клик по строке.

mod common;

use common::{CallLog, Msg, Stub, assert_screen, call_log, key_msg, left_down, left_up, screen};
use gterm_widgets::input::Handled;
use gterm_widgets::widgets::{ComboBox, ComboBoxState};
use gterm_widgets::{Constraint, Control, Layout, Rect, StatefulWidget, Ui};
use winit::keyboard::{Key, ModifiersState, NamedKey};

const KINDS: [&str; 4] = ["файл", "каталог", "ссылка", "устройство"];
const SCREEN: Rect = Rect::new(0, 0, 20, 10);

struct App {
	kind: ComboBoxState,
	other: Stub,
	/// Поле внизу экрана — слою не хватает места снизу.
	at_bottom: bool,
}

impl App {
	fn new(log: &CallLog) -> Self {
		Self {
			kind: ComboBoxState::new(),
			other: Stub::new("o", true, log),
			at_bottom: false,
		}
	}

	fn ui(&mut self) -> Ui<'_, Msg> {
		let combo = Ui::combo_box(
			ComboBox::new(&KINDS).placeholder("—").rows(3),
			&mut self.kind,
			Msg::Picked,
		);
		let other = Ui::control(&mut self.other);
		if self.at_bottom {
			Ui::split(
				Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]),
				vec![other, combo],
			)
		} else {
			Ui::split(
				Layout::vertical([Constraint::Length(1), Constraint::Length(1)]),
				vec![combo, other],
			)
		}
	}

	/// Раскладка экрана и слоя (как кадр): области полей и списка записаны в состояние.
	fn draw(&mut self) -> Option<Rect> {
		self.ui().render(SCREEN, &mut screen(20, 10));
		self.ui().modal_buffer(SCREEN).map(|(area, _)| area)
	}

	fn key(&mut self, key: NamedKey) -> (Handled, Vec<Msg>) {
		let key = Key::Named(key);
		let routed = self.ui().route_key(&key_msg(&key, ModifiersState::empty()));
		(routed.handled, routed.messages)
	}
}

#[test]
fn closed_field_shows_the_value_or_placeholder_and_a_marker() {
	let area = Rect::new(0, 0, 12, 1);
	let mut state = ComboBoxState::new();
	let mut s = screen(12, 1);
	StatefulWidget::render(
		ComboBox::new(&KINDS).placeholder("—"),
		area,
		&mut s,
		&mut state,
	);
	assert_screen(&s, &["—          ▾"]);

	let mut state = ComboBoxState::with_selected(Some(1));
	let mut s = screen(12, 1);
	StatefulWidget::render(ComboBox::new(&KINDS), area, &mut s, &mut state);
	assert_screen(&s, &["каталог    ▾"]);

	state.open_list();
	let mut s = screen(12, 1);
	StatefulWidget::render(ComboBox::new(&KINDS), area, &mut s, &mut state);
	assert_screen(&s, &["каталог    ▴"]);
	assert!(Control::<Msg>::focusable(&state));
}

#[test]
fn opens_by_keys_or_click_lists_below_the_field_and_selection_closes_it() {
	let log = call_log();
	let mut app = App::new(&log);
	assert_eq!(app.draw(), None, "закрыт — слоя нет");
	assert_eq!(app.kind.node.area, Rect::new(0, 0, 20, 1));

	// Enter раскрывает: слой под полем, три строки списка плюс рамка.
	assert_eq!(app.key(NamedKey::Enter), (Handled::Yes, vec![]));
	assert!(app.kind.open);
	let (area, buffer) = app.ui().modal_buffer(SCREEN).expect("слой раскрыт");
	assert_eq!(area, Rect::new(0, 1, 20, 5));
	assert_screen(
		&buffer,
		&[
			"┌──────────────────┐",
			"│файл              │",
			"│каталог           │",
			"│ссылка            │",
			"└──────────────────┘",
		],
	);

	// Стрелка и Enter в слое: выбор публикуется, список закрыт.
	assert_eq!(app.key(NamedKey::ArrowDown), (Handled::Yes, vec![]));
	assert_eq!(
		app.key(NamedKey::Enter),
		(Handled::Yes, vec![Msg::Picked(0)])
	);
	assert!(!app.kind.open);
	assert_eq!(app.kind.selected(), Some(0));
	assert!(app.draw().is_none());

	// Пробел раскрывает со старта на текущем значении; ↓ → следующий; Enter → новое значение.
	assert_eq!(app.key(NamedKey::Space), (Handled::Yes, vec![]));
	assert_eq!(app.kind.list.selected(), Some(0));
	app.key(NamedKey::ArrowDown);
	assert_eq!(
		app.key(NamedKey::Enter),
		(Handled::Yes, vec![Msg::Picked(1)])
	);

	// Клик по полю раскрывает; Enter на том же значении — закрытие без сообщения.
	let routed = app.ui().route_mouse(&left_down(3, 0), SCREEN);
	assert_eq!(routed.handled, Handled::Yes);
	assert!(app.kind.open);
	assert_eq!(app.key(NamedKey::Enter), (Handled::Yes, vec![]));
	assert!(!app.kind.open);
	assert_eq!(app.kind.selected(), Some(1));
	assert!(
		log.borrow().is_empty(),
		"соседний контрол ничего не получал: {:?}",
		log.borrow()
	);
}

#[test]
fn escape_and_click_outside_collapse_the_list_silently() {
	let log = call_log();
	let mut app = App::new(&log);
	app.draw();
	app.key(NamedKey::ArrowDown);
	assert!(app.kind.open);
	assert_eq!(app.key(NamedKey::Escape), (Handled::Yes, vec![]));
	assert!(!app.kind.open);

	app.key(NamedKey::Enter);
	assert!(app.kind.open);
	let routed = app.ui().route_mouse(&left_down(10, 9), SCREEN);
	assert_eq!(routed.handled, Handled::Yes, "клик мимо поглощён слоем");
	assert!(routed.messages.is_empty());
	assert!(!app.kind.open);
	assert!(log.borrow().is_empty(), "экран под слоем клик не получил");
	// Следующий ввод снова идёт экрану: Tab переводит фокус на соседа.
	app.key(NamedKey::Tab);
	assert!(app.other.node.focused);
}

/// Поле принимает нажатие и получает захват, а парное отпускание уходит уже слою списка. Захват не
/// должен пережить список: иначе следующее нажатие где угодно достанется полю и раскроет его заново.
#[test]
fn a_field_opened_by_a_click_does_not_keep_the_pointer_after_the_list_is_closed() {
	let log = call_log();
	let mut app = App::new(&log);
	app.draw();
	app.ui().route_mouse(&left_down(1, 0), SCREEN);
	app.draw();
	app.ui().route_mouse(&left_up(1, 0), SCREEN);
	assert!(app.kind.open, "клик по полю раскрыл список");
	assert!(
		!app.kind.node.captured,
		"пока список раскрыт, поле указатель не держит"
	);

	// Закрытие клавишей: следующее нажатие мимо поля достаётся соседу, список закрыт.
	app.key(NamedKey::Escape);
	app.draw();
	app.ui().route_mouse(&left_down(1, 1), SCREEN);
	assert!(!app.kind.open, "нажатие мимо поля не раскрывает список");
	assert!(
		!log.borrow().is_empty(),
		"нажатие получил контрол под указателем"
	);
	app.ui().route_mouse(&left_up(1, 1), SCREEN);

	// То же после закрытия кликом мимо слоя.
	app.ui().route_mouse(&left_down(1, 0), SCREEN);
	app.draw();
	app.ui().route_mouse(&left_up(1, 0), SCREEN);
	assert!(app.kind.open);
	app.ui().route_mouse(&left_down(10, 9), SCREEN);
	app.ui().route_mouse(&left_up(10, 9), SCREEN);
	assert!(!app.kind.open);
	app.draw();
	log.borrow_mut().clear();
	app.ui().route_mouse(&left_down(1, 1), SCREEN);
	assert!(!app.kind.open);
	assert!(!log.borrow().is_empty());
}

#[test]
fn click_on_a_row_selects_it_and_the_list_goes_above_when_there_is_no_room_below() {
	let log = call_log();
	let mut app = App::new(&log);
	app.draw();
	app.key(NamedKey::Enter);
	let area = app.draw().expect("слой раскрыт");
	assert_eq!(area, Rect::new(0, 1, 20, 5));
	assert!(
		app.kind.node.focused && !app.other.node.focused,
		"кадр при открытом слое не трогает фокус экрана"
	);
	// Третья строка списка: сетка (3, 4) → локальная (3, 3) → пункт 2.
	let routed = app.ui().route_mouse(&left_down(3, 4), SCREEN);
	assert_eq!(routed.messages, vec![Msg::Picked(2)]);
	assert!(!app.kind.open);
	app.draw();
	assert!(
		app.kind.node.focused && !app.other.node.focused,
		"после выбора фокус остаётся на поле списка"
	);

	// Поле у нижнего края: слой над ним.
	app.at_bottom = true;
	app.draw();
	assert_eq!(app.kind.node.area, Rect::new(0, 9, 20, 1));
	app.ui().route_mouse(&left_down(3, 9), SCREEN);
	assert!(app.kind.open);
	let (area, buffer) = app.ui().modal_buffer(SCREEN).expect("слой раскрыт");
	assert_eq!(area, Rect::new(0, 4, 20, 5));
	assert_eq!(buffer.rows(), 5);
}

#[test]
fn disabled_field_does_not_focus_or_open() {
	let mut state = ComboBoxState::new();
	state.enabled = false;
	assert!(!Control::<Msg>::focusable(&state));
	let key = Key::Named(NamedKey::Enter);
	let mut out = gterm_widgets::Messages::<Msg>::new();
	assert_eq!(
		state.key(&key_msg(&key, ModifiersState::empty()), &mut out),
		Handled::No
	);
	assert!(!state.open);
}
