//! Полоса прокрутки списка (ADR-0032): список рисует её сам, с точными значениями своего кадра
//! (ползунок не отстаёт от клавиш), и обрабатывает мышь на ней — стрелки, дорожка, протяжка
//! ползунка под захватом. Прокрутка полосой выделение не меняет и не откатывается отрисовкой.

mod common;

use common::{Msg, assert_screen, key_msg, left_down, left_up, moved, screen};
use gterm::buffer::ScreenBuffer;
use gterm_widgets::input::{Handled, Routed};
use gterm_widgets::widgets::{List, ListState, Scrollbar, ScrollbarOrientation};
use gterm_widgets::{Constraint, Layout, Rect, Ui};
use winit::keyboard::{Key, ModifiersState, NamedKey};

const WIDTH: u16 = 6;
/// Колонка полосы — последняя колонка списка.
const BAR: i32 = 5;

/// Экран со списком из `len` пунктов `п0`, `п1`, … высотой `height` строк и полосой прокрутки.
struct App {
	list: ListState,
	len: usize,
	height: u16,
}

impl App {
	fn new(len: usize, height: u16) -> Self {
		Self {
			list: ListState::default(),
			len,
			height,
		}
	}

	fn area(&self) -> Rect {
		Rect::new(0, 0, WIDTH, self.height)
	}

	fn ui(&mut self) -> Ui<'_, Msg> {
		let items = (0..self.len).map(|i| format!("п{i}"));
		let list = List::new(items).scrollbar(Scrollbar::new(ScrollbarOrientation::Vertical));
		Ui::split(
			Layout::vertical([Constraint::Fill(1)]),
			vec![Ui::stateful(list, &mut self.list).on_select(Msg::Picked)],
		)
	}

	fn draw(&mut self) -> ScreenBuffer {
		let area = self.area();
		let mut s = screen(usize::from(WIDTH), usize::from(self.height));
		self.ui().render(area, &mut s);
		s
	}

	/// Знаки полосы сверху вниз после отрисовки.
	fn bar(&mut self) -> String {
		let s = self.draw();
		(0..s.rows())
			.map(|row| s.cell(BAR as usize, row).ch)
			.collect()
	}

	fn key(&mut self, key: NamedKey) -> Routed<Msg> {
		let key = Key::Named(key);
		self.ui().route_key(&key_msg(&key, ModifiersState::empty()))
	}

	fn press(&mut self, row: i32) -> Routed<Msg> {
		let area = self.area();
		self.ui().route_mouse(&left_down(BAR, row), area)
	}

	fn release(&mut self, row: i32) {
		let area = self.area();
		self.ui().route_mouse(&left_up(BAR, row), area);
	}

	fn click(&mut self, row: i32) -> Routed<Msg> {
		let routed = self.press(row);
		self.release(row);
		routed
	}

	fn drag_to(&mut self, col: i32, row: i32) -> Routed<Msg> {
		let area = self.area();
		self.ui().route_mouse(&moved(col, row), area)
	}
}

/// Нажатие на полосе принято, сообщений (смены выделения) нет.
fn scrolled(routed: &Routed<Msg>) {
	assert_eq!(routed.handled, Handled::Yes);
	assert!(routed.messages.is_empty(), "{:?}", routed.messages);
}

#[test]
fn the_list_draws_its_bar_in_the_last_column_in_the_same_frame_as_the_key() {
	let mut app = App::new(10, 4);
	app.list.select(Some(0));
	// Пунктам — ширина без полосы; видно 4 из 10, дорожка 2 ячейки, ползунок 1.
	assert_screen(&app.draw(), &["п0   ▲", "п1   █", "п2   │", "п3   ▼"]);

	// `End`: смещение пересчитывается в отрисовке, и полоса показывает его в том же кадре.
	let routed = app.key(NamedKey::End);
	assert_eq!(routed.messages, vec![Msg::Picked(9)]);
	assert_screen(&app.draw(), &["п6   ▲", "п7   │", "п8   █", "п9   ▼"]);
	assert_eq!(app.list.offset, 6);

	app.key(NamedKey::Home);
	assert_eq!(app.bar(), "▲█│▼");
	app.key(NamedKey::PageDown);
	assert_eq!(app.bar(), "▲█│▼");
	assert_eq!((app.list.selected(), app.list.offset), (Some(4), 1));
}

#[test]
fn a_list_that_fits_shows_a_full_thumb_and_a_narrow_one_has_no_bar() {
	let mut app = App::new(3, 5);
	assert_screen(
		&app.draw(),
		&["п0   ▲", "п1   █", "п2   █", "     █", "     ▼"],
	);
	// Содержимое помещается: полоса ничего не прокручивает.
	scrolled(&app.click(4));
	scrolled(&app.click(2));
	assert_eq!(app.list.offset, 0);

	// В одну колонку полоса не помещается — список рисуется без неё.
	let mut s = screen(1, 2);
	let mut state = ListState::default();
	let list = List::new(["a", "b"]).scrollbar(Scrollbar::new(ScrollbarOrientation::Vertical));
	Ui::<Msg>::stateful(list, &mut state).render(Rect::new(0, 0, 1, 2), &mut s);
	assert_screen(&s, &["a", "b"]);
}

#[test]
fn arrows_scroll_by_one_item_and_the_track_by_a_page_without_touching_the_selection() {
	let mut app = App::new(10, 4);
	app.list.select(Some(0));
	app.draw();

	// Стрелка вниз (последняя ячейка полосы) — на пункт; выделение остаётся на п0.
	scrolled(&app.click(3));
	assert_eq!((app.list.offset, app.list.selected()), (1, Some(0)));
	assert_screen(&app.draw(), &["п1   ▲", "п2   █", "п3   │", "п4   ▼"]);
	// Отрисовка не возвращает смещение к выделенному пункту, пока выделение то же.
	app.draw();
	assert_eq!(app.list.offset, 1);

	// Дорожка за ползунком — на страницу (4 пункта), у конца смещение подрезается.
	scrolled(&app.click(2));
	assert_eq!(app.list.offset, 5);
	assert_eq!(app.bar(), "▲│█▼");
	app.click(3);
	app.click(3);
	app.click(3);
	assert_eq!(app.list.offset, 6, "дальше последней страницы не уходит");
	app.draw();

	// Дорожка перед ползунком — на страницу назад, стрелка вверх — на пункт.
	scrolled(&app.click(1));
	assert_eq!(app.list.offset, 2);
	app.draw();
	scrolled(&app.click(0));
	scrolled(&app.click(0));
	scrolled(&app.click(0));
	assert_eq!(app.list.offset, 0, "раньше начала не уходит");

	// Смена выделения клавишей возвращает смещение к выделенному пункту.
	app.click(3);
	app.click(3);
	app.draw();
	assert_eq!(app.list.offset, 2);
	app.key(NamedKey::ArrowDown);
	app.draw();
	assert_eq!((app.list.selected(), app.list.offset), (Some(1), 1));
}

#[test]
fn dragging_the_thumb_scrolls_proportionally_under_capture() {
	// Видно 8 из 20: дорожка 6 ячеек (строки 1–6), ползунок 2, ход ползунка 4 ячейки на 12 пунктов.
	let mut app = App::new(20, 8);
	assert_eq!(app.bar(), "▲██││││▼");

	scrolled(&app.press(1));
	assert!(app.list.node.captured);
	assert_eq!(
		app.list.offset, 0,
		"нажатие на ползунок само не прокручивает"
	);
	scrolled(&app.drag_to(BAR, 3));
	assert_eq!(app.list.offset, 6);
	assert_eq!(app.bar(), "▲││██││▼");
	// Под захватом указатель может уйти с полосы в сторону и за её край.
	scrolled(&app.drag_to(0, 5));
	assert_eq!(app.list.offset, 12);
	app.drag_to(BAR, 40);
	assert_eq!(app.list.offset, 12);
	assert_eq!(app.bar(), "▲││││██▼");
	app.drag_to(BAR, -7);
	assert_eq!(app.list.offset, 0);

	// После отпускания движение мыши список не прокручивает.
	app.release(-7);
	assert!(!app.list.node.captured);
	let routed = app.drag_to(BAR, 5);
	assert_eq!(routed.handled, Handled::No);
	assert_eq!(app.list.offset, 0);

	// Новое нажатие — на стрелку: прежняя протяжка не продолжается.
	app.draw();
	scrolled(&app.press(7));
	assert_eq!(app.list.offset, 1);
	let routed = app.drag_to(BAR, 5);
	assert_eq!(routed.handled, Handled::No);
	assert_eq!(
		app.list.offset, 1,
		"движение после нажатия на стрелку не тянет ползунок"
	);
	app.release(5);
	app.list.scroll_to(0);

	// Взялись за нижнюю ячейку ползунка: она и остаётся под указателем.
	app.draw();
	app.press(2);
	app.drag_to(BAR, 3);
	assert_eq!(app.list.offset, 3);
	app.release(3);
}

#[test]
fn clicks_beside_the_bar_still_select_items_and_height_changes_refit_the_selection() {
	let mut app = App::new(10, 4);
	app.draw();
	let area = app.area();
	let routed = app.ui().route_mouse(&left_down(1, 2), area);
	assert_eq!(routed.messages, vec![Msg::Picked(2)]);
	app.ui().route_mouse(&left_up(1, 2), area);

	// Список стал ниже: выделенный пункт снова подгоняется в видимую область.
	app.key(NamedKey::End);
	app.draw();
	assert_eq!(app.list.offset, 6);
	app.height = 2;
	app.draw();
	assert_eq!(app.list.offset, 8);

	// Программная прокрутка — те же правила, что у полосы.
	app.list.scroll_to(3);
	app.draw();
	assert_eq!(app.list.offset, 3);
	app.list.scroll_by(-10);
	assert_eq!(app.list.offset, 0);
	app.list.scroll_by(100);
	app.draw();
	assert_eq!(app.list.offset, 8);
	// Смещение, заданное приложением мимо методов, отрисовка подрезает до последней страницы.
	app.list.offset = 99;
	assert_screen(&app.draw(), &["п8   │", "п9   █"]);
	assert_eq!(app.list.offset, 8);
}
