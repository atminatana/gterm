//! Доработки полосы прокрутки у контрола-владельца (поправка ADR-0032): колесо прокручивает
//! содержимое, удержанная на стрелке или дорожке кнопка листает повторно по тактам `Ui::tick`,
//! у таблицы полоса та же, что у списка, и стоит рядом со строками, а не с заголовком.

mod common;

use std::time::{Duration, Instant};

use common::{Msg, assert_screen, key_msg, left_down, left_up, mouse, moved, screen};
use gterm::buffer::ScreenBuffer;
use gterm_widgets::input::{Handled, MouseKind, Routed};
use gterm_widgets::widgets::{
	List, ListState, Row, Scrollbar, ScrollbarOrientation, Table, TableState,
};
use gterm_widgets::{Constraint, Layout, Rect, Ui};
use winit::keyboard::{Key, ModifiersState, NamedKey};

const DELAY: Duration = Scrollbar::REPEAT_DELAY;
const INTERVAL: Duration = Scrollbar::REPEAT_INTERVAL;
const MS: Duration = Duration::from_millis(1);

fn bar() -> Scrollbar {
	Scrollbar::new(ScrollbarOrientation::Vertical)
}

/// Список из 30 пунктов в области 6×8 либо таблица из 20 строк с заголовком в области 8×6.
#[derive(Default)]
struct App {
	list: ListState,
	table: TableState,
	is_table: bool,
	has_bar: bool,
}

impl App {
	fn list() -> Self {
		Self {
			has_bar: true,
			..Self::default()
		}
	}

	fn table() -> Self {
		Self {
			is_table: true,
			has_bar: true,
			..Self::default()
		}
	}

	fn area(&self) -> Rect {
		if self.is_table {
			Rect::new(0, 0, 8, 6)
		} else {
			Rect::new(0, 0, 6, 8)
		}
	}

	/// Колонка полосы — последняя колонка области.
	fn bar_col(&self) -> i32 {
		i32::from(self.area().width) - 1
	}

	fn ui(&mut self) -> Ui<'_, Msg> {
		let control = if self.is_table {
			let rows = (0..20).map(|i| Row::new([format!("р{i}")]));
			let mut table = Table::new(rows, [Constraint::Fill(1)]).header(Row::new(["имя"]));
			if self.has_bar {
				table = table.scrollbar(bar());
			}
			Ui::stateful(table, &mut self.table).on_select(Msg::Picked)
		} else {
			let mut list = List::new((0..30).map(|i| format!("п{i}")));
			if self.has_bar {
				list = list.scrollbar(bar());
			}
			Ui::stateful(list, &mut self.list).on_select(Msg::Picked)
		};
		Ui::split(Layout::vertical([Constraint::Fill(1)]), vec![control])
	}

	fn draw(&mut self) -> ScreenBuffer {
		let area = self.area();
		let mut s = screen(usize::from(area.width), usize::from(area.height));
		self.ui().render(area, &mut s);
		s
	}

	fn offset(&self) -> usize {
		if self.is_table {
			self.table.offset
		} else {
			self.list.offset
		}
	}

	fn selected(&self) -> Option<usize> {
		if self.is_table {
			self.table.selected()
		} else {
			self.list.selected()
		}
	}

	fn press(&mut self, row: i32) -> Routed<Msg> {
		let (area, col) = (self.area(), self.bar_col());
		self.ui().route_mouse(&left_down(col, row), area)
	}

	fn release(&mut self, row: i32) {
		let (area, col) = (self.area(), self.bar_col());
		self.ui().route_mouse(&left_up(col, row), area);
	}

	fn wheel(&mut self, lines: f32) -> Routed<Msg> {
		let area = self.area();
		self.ui()
			.route_mouse(&mouse(MouseKind::Wheel { lines }, None, 1, 2), area)
	}

	/// Такт времени и перерисовка, как делает приложение в `on_frame`; ответ — просит ли контрол
	/// следующий такт.
	fn tick(&mut self, now: Instant) -> bool {
		let routed = self.ui().tick(now);
		assert!(routed.messages.is_empty());
		self.draw();
		routed.handled == Handled::Yes
	}
}

fn scrolled(routed: &Routed<Msg>) {
	assert_eq!(routed.handled, Handled::Yes);
	assert!(routed.messages.is_empty(), "{:?}", routed.messages);
}

#[test]
fn the_wheel_scrolls_the_content_when_there_is_a_bar_and_moves_the_selection_without_one() {
	for mut app in [App::list(), App::table()] {
		app.draw();
		// Вниз (отрицательные строки по соглашению `winit`) — содержимое на пункт дальше.
		scrolled(&app.wheel(-1.0));
		scrolled(&app.wheel(-2.0));
		assert_eq!(
			(app.offset(), app.selected()),
			(3, None),
			"таблица: {}",
			app.is_table
		);
		app.draw();
		assert_eq!(app.offset(), 3, "отрисовка прокрутку не откатывает");
		scrolled(&app.wheel(5.0));
		assert_eq!(app.offset(), 0, "раньше начала не уходит");
		scrolled(&app.wheel(-500.0));
		let last = if app.is_table { 15 } else { 22 };
		assert_eq!(app.offset(), last, "дальше последней страницы не уходит");

		// Без полосы колесо, как раньше, двигает выделение.
		app.has_bar = false;
		app.draw();
		let routed = app.wheel(-1.0);
		assert_eq!(routed.messages, vec![Msg::Picked(0)]);
	}
}

#[test]
fn a_button_held_on_an_arrow_repeats_after_a_delay_until_it_is_released() {
	for mut app in [App::list(), App::table()] {
		let last_row = i32::from(app.area().height) - 1;
		app.draw();
		let start = Instant::now();
		assert!(!app.tick(start), "без нажатия время контролу не нужно");

		// Нажатие листает сразу; первый такт только заводит отсчёт.
		scrolled(&app.press(last_row));
		assert_eq!(app.offset(), 1);
		assert!(app.tick(start));
		assert_eq!(app.offset(), 1);
		assert!(app.tick(start + DELAY - MS));
		assert_eq!(app.offset(), 1, "до задержки повтора нет");

		assert!(app.tick(start + DELAY));
		assert_eq!(app.offset(), 2);
		assert!(app.tick(start + DELAY + INTERVAL - MS));
		assert_eq!(app.offset(), 2, "между повторами — промежуток");
		assert!(app.tick(start + DELAY + INTERVAL));
		assert!(app.tick(start + DELAY + INTERVAL * 2));
		assert_eq!(app.offset(), 4);
		assert_eq!(app.selected(), None, "выделение не меняется");

		// Отпускание останавливает повтор; новое нажатие отсчитывает задержку заново.
		app.release(last_row);
		assert!(!app.tick(start + DELAY * 10));
		assert_eq!(app.offset(), 4);
		let first_row = if app.is_table { 1 } else { 0 };
		app.press(first_row);
		assert_eq!(app.offset(), 3);
		assert!(app.tick(start + DELAY * 20));
		assert!(app.tick(start + DELAY * 20 + INTERVAL));
		assert_eq!(
			app.offset(),
			3,
			"задержка считается от первого такта после нажатия"
		);
		assert!(app.tick(start + DELAY * 21));
		assert_eq!(app.offset(), 2);
		app.release(first_row);
	}
}

#[test]
fn holding_the_track_pages_until_the_thumb_reaches_the_pointer() {
	// Список: видно 8 из 30, дорожка — строки 1–6, ползунок 2 ячейки, страница 8 пунктов.
	let mut app = App::list();
	app.draw();
	let start = Instant::now();
	scrolled(&app.press(6));
	assert_eq!(app.offset(), 8);
	assert!(app.tick(start));
	assert!(app.tick(start + DELAY));
	assert_eq!(app.offset(), 16);
	assert!(app.tick(start + DELAY + INTERVAL));
	assert_eq!(app.offset(), 22, "последняя страница");
	// Ползунок дошёл до указателя — листание остановилось, хотя кнопка всё ещё удержана.
	assert!(app.tick(start + DELAY + INTERVAL * 2));
	assert!(app.tick(start + DELAY + INTERVAL * 3));
	assert_eq!(app.offset(), 22);
	app.release(6);

	// Указатель посередине дорожки (строка 4): после второй страницы ползунок оказывается под
	// ним, и листание останавливается на смещении 16, не доходя до последней страницы.
	app.list.scroll_to(0);
	app.draw();
	let again = start + DELAY * 10;
	scrolled(&app.press(4));
	assert_eq!(app.offset(), 8);
	assert!(app.tick(again));
	assert!(app.tick(again + DELAY));
	assert_eq!(app.offset(), 16);
	assert!(app.tick(again + DELAY + INTERVAL));
	assert!(app.tick(again + DELAY + INTERVAL * 2));
	assert_eq!(
		app.offset(),
		16,
		"ползунок под указателем — дальше не листает"
	);
	app.release(4);

	// Нажатие на ползунок — протяжка, а не удержание: время ей не нужно.
	app.press(5);
	assert!(!app.tick(start + DELAY * 5));
	let (area, col) = (app.area(), app.bar_col());
	app.ui().route_mouse(&moved(col, 1), area);
	assert_eq!(app.offset(), 0);
	app.release(1);
}

#[test]
fn a_table_draws_its_bar_beside_the_rows_and_handles_keys_and_mouse_like_a_list() {
	let mut app = App::table();
	// Заголовок — без полосы; видно 5 из 20: стрелки, дорожка 3 ячейки, ползунок 1.
	assert_screen(
		&app.draw(),
		&[
			"имя",
			"р0     ▲",
			"р1     █",
			"р2     │",
			"р3     │",
			"р4     ▼",
		],
	);

	// `End`: ползунок внизу в том же кадре.
	let end = Key::Named(NamedKey::End);
	let routed = app.ui().route_key(&key_msg(&end, ModifiersState::empty()));
	assert_eq!(routed.messages, vec![Msg::Picked(19)]);
	assert_screen(
		&app.draw(),
		&[
			"имя",
			"р15    ▲",
			"р16    │",
			"р17    │",
			"р18    █",
			"р19    ▼",
		],
	);

	// Стрелка вверх и дорожка перед ползунком: строка и страница назад, выделение на месте.
	scrolled(&app.press(1));
	app.release(1);
	assert_eq!((app.offset(), app.selected()), (14, Some(19)));
	app.draw();
	scrolled(&app.press(2));
	app.release(2);
	assert_eq!(app.offset(), 9);
	app.draw();
	app.draw();
	assert_eq!(
		app.offset(),
		9,
		"отрисовка не возвращает к выделенной строке"
	);

	// Протяжка ползунка: смещение 9 из 15 — ползунок в средней ячейке дорожки (строка 3).
	scrolled(&app.press(3));
	let (area, col) = (app.area(), app.bar_col());
	app.ui().route_mouse(&moved(col, 2), area);
	assert_eq!(app.offset(), 0);
	app.ui().route_mouse(&moved(col, 9), area);
	assert_eq!(app.offset(), 15);
	app.release(9);

	// Клик по строке рядом с полосой по-прежнему выделяет её.
	app.draw();
	let routed = app.ui().route_mouse(&left_down(1, 2), area);
	assert_eq!(routed.messages, vec![Msg::Picked(16)]);
	app.ui().route_mouse(&left_up(1, 2), area);

	// Смена выделения подгоняет смещение под него один раз; дальше прокрутка снова свободна.
	app.draw();
	assert_eq!(app.offset(), 15);

	// Программная прокрутка с пределами.
	app.table.scroll_to(4);
	app.draw();
	assert_eq!(app.offset(), 4);
	app.table.scroll_by(-9);
	assert_eq!(app.offset(), 0);
	app.table.offset = 99;
	app.draw();
	assert_eq!(app.offset(), 15);
}
