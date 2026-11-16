//! Двойной клик (ADR-0035, ТЗ `2026-11-16-double-click-slider` Д3–Д5): привязка
//! `Ui::on_double_click` у любого контрола — кнопки, флажка, списка, таблицы, своего контрола и
//! контрола модального слоя; первое нажатие пары — обычный клик; полоса прокрутки, заголовок и
//! пустые строки двойного клика не дают; без привязки маршрутизация прежняя.

mod common;

use common::{
	CallLog, Msg, Stub, call_log, left_double_down, left_double_up, left_down, left_up, screen,
};
use gterm_widgets::input::{Handled, MouseMsg, Routed};
use gterm_widgets::widgets::{
	Button, ButtonState, Checkbox, CheckboxState, List, ListState, Paragraph, Row, Scrollbar,
	ScrollbarOrientation, Table, TableState,
};
use gterm_widgets::{Constraint, Layout, Modal, Rect, Ui};

const OPEN: Msg = Msg::Custom("open");
const AREA: Rect = Rect::new(0, 0, 10, 4);

/// Двойной клик в ячейке: нажатие, отпускание, второе нажатие, второе отпускание.
fn double_click(col: i32, row: i32) -> [MouseMsg; 4] {
	[
		left_down(col, row),
		left_up(col, row),
		left_double_down(col, row),
		left_double_up(col, row),
	]
}

/// Отправить сообщения мыши по очереди, каждое — в свежее описание `ui`.
fn route_all<S>(
	state: &mut S,
	ui: impl Fn(&mut S) -> Ui<'_, Msg>,
	area: Rect,
	msgs: &[MouseMsg],
) -> Vec<Routed<Msg>> {
	msgs.iter()
		.map(|msg| ui(state).route_mouse(msg, area))
		.collect()
}

/// Сообщения каждого результата.
fn messages(routed: Vec<Routed<Msg>>) -> Vec<Vec<Msg>> {
	routed.into_iter().map(|r| r.messages).collect()
}

fn button_ui(state: &mut ButtonState) -> Ui<'_, Msg> {
	Ui::stateful(Button::new("OK"), state)
		.on_activate(Msg::Pressed)
		.on_double_click(OPEN)
}

#[test]
fn a_button_publishes_the_binding_on_the_second_press_and_still_activates_on_each_release() {
	let mut state = ButtonState::new();
	button_ui(&mut state).layout(AREA);
	let routed = route_all(&mut state, button_ui, AREA, &double_click(4, 1));
	assert_eq!(routed[2].handled, Handled::Yes);
	assert_eq!(
		messages(routed),
		vec![vec![], vec![Msg::Pressed], vec![OPEN], vec![Msg::Pressed]],
		"каждое отпускание — обычная активация, второе нажатие — сообщение двойного клика"
	);
	assert!(!state.node.captured && !state.pressed);
}

#[test]
fn a_single_click_gives_no_double_click_message() {
	let mut state = ButtonState::new();
	button_ui(&mut state).layout(AREA);
	// Два одиночных клика подряд (окно двойного не насчитало) — только активации.
	let clicks = [
		left_down(4, 1),
		left_up(4, 1),
		left_down(4, 1),
		left_up(4, 1),
	];
	let routed = route_all(&mut state, button_ui, AREA, &clicks);
	assert_eq!(
		messages(routed),
		vec![vec![], vec![Msg::Pressed], vec![], vec![Msg::Pressed]]
	);
}

#[test]
fn a_disabled_button_gives_no_double_click_message() {
	let mut state = ButtonState::disabled();
	button_ui(&mut state).layout(AREA);
	let routed = route_all(&mut state, button_ui, AREA, &double_click(4, 1));
	assert!(routed.iter().all(|r| r.handled == Handled::No));
	assert_eq!(messages(routed), vec![Vec::<Msg>::new(); 4]);
}

fn checkbox_ui(state: &mut CheckboxState) -> Ui<'_, Msg> {
	Ui::stateful(Checkbox::new("Опция"), state)
		.on_toggle(|checked| Msg::Custom(if checked { "on" } else { "off" }))
		.on_double_click(OPEN)
}

#[test]
fn a_checkbox_toggles_on_each_click_and_publishes_the_binding() {
	let mut state = CheckboxState::new();
	checkbox_ui(&mut state).layout(AREA);
	let routed = route_all(&mut state, checkbox_ui, AREA, &double_click(1, 0));
	assert_eq!(
		messages(routed),
		vec![
			vec![],
			vec![Msg::Custom("on")],
			vec![OPEN],
			vec![Msg::Custom("off")]
		]
	);
}

/// Список из `len` пунктов в области 10×4 с полосой прокрутки в последней колонке; привязка
/// двойного клика — по флагу.
struct ListApp {
	list: ListState,
	len: usize,
	is_bound: bool,
}

/// Колонка полосы прокрутки списка.
const BAR: i32 = 9;

impl ListApp {
	fn new(len: usize, is_bound: bool) -> Self {
		Self {
			list: ListState::default(),
			len,
			is_bound,
		}
	}

	fn ui(&mut self) -> Ui<'_, Msg> {
		let list = List::new((0..self.len).map(|i| format!("п{i}")))
			.scrollbar(Scrollbar::new(ScrollbarOrientation::Vertical));
		let node = Ui::stateful(list, &mut self.list).on_select(Msg::Picked);
		let node = if self.is_bound {
			node.on_double_click(OPEN)
		} else {
			node
		};
		Ui::split(Layout::vertical([Constraint::Fill(1)]), vec![node])
	}

	/// Отрисовка заполняет карту строк и разметку полосы — по ним список находит место нажатия.
	fn draw(&mut self) {
		let mut s = screen(usize::from(AREA.width), usize::from(AREA.height));
		self.ui().render(AREA, &mut s);
	}

	fn route(&mut self, msgs: &[MouseMsg]) -> Vec<Routed<Msg>> {
		msgs.iter()
			.map(|msg| self.ui().route_mouse(msg, AREA))
			.collect()
	}
}

#[test]
fn in_a_list_the_row_under_the_pointer_is_selected_when_the_message_arrives() {
	let mut app = ListApp::new(6, true);
	app.draw();
	assert_eq!(app.list.selected(), None);

	let [down, up, double_down, double_up] = double_click(2, 2);
	let first = app.route(&[down, up]);
	assert_eq!(messages(first), vec![vec![Msg::Picked(2)], vec![]]);
	assert_eq!(
		app.list.selected(),
		Some(2),
		"первое нажатие — обычный клик"
	);

	let second = app.route(&[double_down]);
	assert_eq!(messages(second), vec![vec![OPEN]]);
	assert_eq!(
		app.list.selected(),
		Some(2),
		"к сообщению выделена строка под указателем"
	);
	app.route(&[double_up]);
	assert!(!app.list.node.captured);
}

#[test]
fn a_double_click_on_the_list_scrollbar_gives_no_message() {
	let mut app = ListApp::new(6, true);
	app.draw();
	// Стрелки и дорожка: каждая строка колонки полосы.
	for row in 0..4 {
		let routed = app.route(&double_click(BAR, row));
		assert!(
			routed.iter().all(|r| !r.messages.contains(&OPEN)),
			"полоса, строка {row}"
		);
		app.draw();
	}
	assert_eq!(app.list.selected(), None, "полоса выделение не трогает");
}

#[test]
fn a_double_click_below_the_last_item_gives_no_message() {
	let mut app = ListApp::new(2, true);
	app.draw();
	let routed = app.route(&double_click(1, 3));
	assert!(routed.iter().all(|r| r.messages.is_empty()));
	assert_eq!(app.list.selected(), None);
}

#[test]
fn a_list_without_the_binding_routes_a_double_click_like_two_ordinary_clicks() {
	let summary = |routed: Vec<Routed<Msg>>| {
		routed
			.into_iter()
			.map(|r| (r.handled, r.messages))
			.collect::<Vec<_>>()
	};
	let mut doubled = ListApp::new(6, false);
	doubled.draw();
	let with_count = summary(doubled.route(&double_click(2, 1)));

	let mut singles = ListApp::new(6, false);
	singles.draw();
	let clicks = [
		left_down(2, 1),
		left_up(2, 1),
		left_down(2, 1),
		left_up(2, 1),
	];
	let without_count = summary(singles.route(&clicks));

	assert_eq!(with_count, without_count);
	assert_eq!(doubled.list, singles.list, "то же состояние списка");
}

/// Таблица в области 12×5: заголовок в строке 0, строки данных 0–3 в строках экрана 1–4,
/// полоса прокрутки в колонке 11.
const TABLE_AREA: Rect = Rect::new(0, 0, 12, 5);

fn table_ui(state: &mut TableState) -> Ui<'_, Msg> {
	let rows = (0..8).map(|i| Row::new([format!("{i}"), format!("стр{i}")]));
	let table = Table::new(rows, [Constraint::Length(3), Constraint::Fill(1)])
		.header(Row::new(["№", "имя"]))
		.scrollbar(Scrollbar::new(ScrollbarOrientation::Vertical));
	Ui::stateful(table, state)
		.on_select(Msg::Picked)
		.on_double_click(OPEN)
}

fn draw_table(state: &mut TableState) {
	let mut s = screen(12, 5);
	table_ui(state).render(TABLE_AREA, &mut s);
}

#[test]
fn a_table_counts_a_double_click_on_a_row_but_not_on_the_header_or_the_scrollbar() {
	let mut state = TableState::default();
	draw_table(&mut state);

	let header = route_all(&mut state, table_ui, TABLE_AREA, &double_click(1, 0));
	assert!(messages(header).iter().all(Vec::is_empty), "заголовок");

	let bar = route_all(&mut state, table_ui, TABLE_AREA, &double_click(11, 3));
	assert!(
		messages(bar).iter().all(|m| !m.contains(&OPEN)),
		"полоса прокрутки"
	);
	assert_eq!(state.selected(), None, "полоса выделение не трогает");
	// Нажатия на дорожку пролистали строки — вернуть к началу.
	state.scroll_to(0);
	draw_table(&mut state);

	let row = route_all(&mut state, table_ui, TABLE_AREA, &double_click(4, 2));
	assert_eq!(
		messages(row),
		vec![vec![Msg::Picked(1)], vec![], vec![OPEN], vec![]]
	);
	assert_eq!(state.selected(), Some(1));
}

/// Свой контрол слева (без привязки) и свой контрол справа (с привязкой).
struct Pair {
	plain: Stub,
	bound: Stub,
}

impl Pair {
	fn new(log: &CallLog) -> Self {
		Self {
			plain: Stub::new("plain", true, log),
			bound: Stub::new("bound", true, log),
		}
	}

	fn ui(&mut self) -> Ui<'_, Msg> {
		Ui::split(
			Layout::horizontal([Constraint::Length(5), Constraint::Length(5)]),
			vec![
				Ui::control(&mut self.plain),
				Ui::control(&mut self.bound).on_double_click(OPEN),
			],
		)
	}
}

#[test]
fn an_application_control_gets_the_press_and_the_binding_adds_the_message() {
	let log = call_log();
	let mut pair = Pair::new(&log);
	pair.ui().layout(AREA);

	// Без привязки: свой контрол ответил «не обработано» — так и остаётся, сообщений нет.
	let routed = pair.ui().route_mouse(&left_double_down(1, 1), AREA);
	assert_eq!(routed.handled, Handled::No);
	assert!(routed.messages.is_empty());

	// С привязкой: нажатие доставлено, сообщение добавлено; захвата контрол не просил — его нет.
	let routed = pair.ui().route_mouse(&left_double_down(6, 1), AREA);
	assert_eq!(routed.handled, Handled::Yes);
	assert_eq!(routed.messages, vec![OPEN]);
	assert!(!pair.bound.node.captured);
	assert_eq!(
		*log.borrow(),
		vec!["plain:mouse:Down@1,1", "bound:mouse:Down@6,1"]
	);
}

/// Экран-заглушка и слой 4×2 в ячейке (2, 1) со своим контролом с привязкой.
fn layer_ui(stub: &mut Stub) -> Ui<'_, Msg> {
	Ui::widget(Paragraph::new(""))
		.modal(Modal::at(Rect::new(2, 1, 4, 2)).content(Ui::control(stub).on_double_click(OPEN)))
}

#[test]
fn a_control_in_a_modal_layer_gets_its_double_click_binding() {
	let log = call_log();
	let mut stub = Stub::new("inner", true, &log);
	// Раскладка содержимого слоя — при отрисовке его буфера.
	layer_ui(&mut stub).modal_buffer(AREA);
	let routed = layer_ui(&mut stub).route_mouse(&left_double_down(3, 1), AREA);
	assert_eq!(routed.messages, vec![OPEN]);
	assert_eq!(*log.borrow(), vec!["inner:mouse:Down@1,0"]);
}
