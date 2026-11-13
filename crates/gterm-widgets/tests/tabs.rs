//! Вкладки (ADR-0032): отрисовка заголовков, разделителя и стилей, обрезка, смена вкладки
//! клавишами и мышью через дерево с привязкой `on_select`, поведение до первой отрисовки,
//! неактивные вкладки, рамка под фокусом.

mod common;

use common::{BG, FG, Msg, assert_screen, key_msg, left_down, left_up, screen};
use gterm::cell::{CellFlags, Color};
use gterm_widgets::input::{Handled, Messages, Routed};
use gterm_widgets::widgets::{Block, Tabs, TabsState};
use gterm_widgets::{
	Constraint, Control, Layout, Rect, StatefulWidget, Style, Ui, WidgetEvent, WidgetEvents,
};
use winit::keyboard::{Key, ModifiersState, NamedKey};

const ACCENT: Color = Color::from_rgb(0xFF, 0x80, 0x00);
const DIM: Color = Color::from_rgb(0x80, 0x80, 0x80);
const AREA: Rect = Rect::new(0, 0, 24, 2);
const TITLES: [&str; 3] = ["Один", "Два", "Три"];

fn tabs<'a>() -> Tabs<'a> {
	Tabs::new(TITLES)
}

fn draw(state: &mut TabsState, widget: Tabs<'_>, width: usize) -> gterm::buffer::ScreenBuffer {
	let mut s = screen(width, 1);
	widget.render(Rect::new(0, 0, width as u16, 1), &mut s, state);
	s
}

fn press(state: &mut TabsState, key: NamedKey) -> Handled {
	let key = Key::Named(key);
	state.key(
		&key_msg(&key, ModifiersState::empty()),
		&mut Messages::<Msg>::new(),
	)
}

/// Вкладки в первой строке области 24×2 с привязкой `on_select`.
fn ui(state: &mut TabsState) -> Ui<'_, Msg> {
	Ui::split(
		Layout::vertical([Constraint::Length(1), Constraint::Length(1)]),
		vec![Ui::stateful(tabs(), state).on_select(Msg::Picked)],
	)
}

fn click(state: &mut TabsState, col: i32) -> Routed<Msg> {
	let routed = ui(state).route_mouse(&left_down(col, 0), AREA);
	ui(state).route_mouse(&left_up(col, 0), AREA);
	routed
}

#[test]
fn titles_are_drawn_with_padding_and_dividers_and_the_selected_one_is_highlighted() {
	let mut state = TabsState::with_selected(1);
	let s = draw(&mut state, tabs(), 24);
	// Колонки: ` Один ` 0–5, `│` 6, ` Два ` 7–11, `│` 12, ` Три ` 13–17.
	assert_screen(&s, &[" Один │ Два │ Три"]);
	let highlighted = CellFlags::BOLD | CellFlags::UNDERLINE;
	for col in 7..12 {
		assert_eq!(s.cell(col, 0).flags, highlighted, "колонка {col}");
	}
	for col in [0, 1, 5, 6, 12, 13, 17] {
		assert_eq!(s.cell(col, 0).flags, CellFlags::empty(), "колонка {col}");
	}

	// Свой стиль выбранной вкладки; под фокусом поверх него — стиль фокуса.
	let styled = || {
		tabs()
			.style(Style::new().fg(DIM))
			.highlight_style(Style::new().fg(ACCENT))
			.focus_style(Style::new().bg(ACCENT).fg(BG))
	};
	let s = draw(&mut state, styled(), 24);
	assert_eq!((s.cell(8, 0).fg, s.cell(8, 0).bg), (ACCENT, BG));
	assert_eq!(s.cell(1, 0).fg, DIM, "прочие вкладки — стилем области");
	state.node.focused = true;
	let s = draw(&mut state, styled(), 24);
	assert_eq!((s.cell(8, 0).fg, s.cell(8, 0).bg), (BG, ACCENT));
	assert_eq!((s.cell(1, 0).fg, s.cell(1, 0).bg), (DIM, BG));
}

#[test]
fn divider_padding_clipping_and_selection_clamping() {
	let mut state = TabsState::new();
	let s = draw(&mut state, tabs().divider("/").padding("[", "]"), 24);
	assert_screen(&s, &["[Один]/[Два]/[Три]"]);

	// Не поместившееся обрезается шириной области.
	let s = draw(&mut state, tabs(), 10);
	assert_screen(&s, &[" Один │ Дв"]);

	// Индекс за последней вкладкой подрезает отрисовка; без вкладок — ничего не рисуется.
	state.select(9);
	draw(&mut state, tabs(), 24);
	assert_eq!(state.selected(), 2);
	let s = draw(&mut state, Tabs::new(Vec::<&str>::new()), 6);
	assert_screen(&s, &[""]);
	assert_eq!(state.selected(), 0);
}

#[test]
fn arrow_keys_change_the_tab_and_report_it_once_per_change() {
	let mut state = TabsState::new();
	assert!(Control::<Msg>::focusable(&state));

	// До первой отрисовки число вкладок неизвестно: клавиша принята, выбор не меняется.
	assert_eq!(press(&mut state, NamedKey::ArrowRight), Handled::Yes);
	assert_eq!((state.selected(), state.take_event()), (0, None));

	draw(&mut state, tabs(), 24);
	assert_eq!(press(&mut state, NamedKey::ArrowRight), Handled::Yes);
	assert_eq!(state.take_event(), Some(WidgetEvent::Selected(1)));
	press(&mut state, NamedKey::ArrowRight);
	assert_eq!(state.take_event(), Some(WidgetEvent::Selected(2)));
	// У края выбор не меняется и события нет; клавиша всё равно принята.
	assert_eq!(press(&mut state, NamedKey::ArrowRight), Handled::Yes);
	assert_eq!((state.selected(), state.take_event()), (2, None));

	press(&mut state, NamedKey::Home);
	assert_eq!(state.take_event(), Some(WidgetEvent::Selected(0)));
	assert_eq!(press(&mut state, NamedKey::ArrowLeft), Handled::Yes);
	assert_eq!(state.take_event(), None);
	press(&mut state, NamedKey::End);
	assert_eq!(state.take_event(), Some(WidgetEvent::Selected(2)));
	press(&mut state, NamedKey::ArrowLeft);
	assert_eq!(state.take_event(), Some(WidgetEvent::Selected(1)));

	// Прочие клавиши — приложению; программный выбор события не даёт.
	for key in [NamedKey::Enter, NamedKey::ArrowDown, NamedKey::Space] {
		assert_eq!(press(&mut state, key), Handled::No, "{key:?}");
	}
	state.select(0);
	assert_eq!(state.take_event(), None);
}

#[test]
fn a_click_on_a_title_selects_it_through_the_tree_binding() {
	let mut state = TabsState::new();
	ui(&mut state).render(AREA, &mut screen(24, 2));

	// Заголовок вместе с отступами — область клика.
	let routed = click(&mut state, 8);
	assert_eq!(routed.handled, Handled::Yes);
	assert_eq!(routed.messages, vec![Msg::Picked(1)]);
	assert!(!state.node.captured, "отпускание сняло захват");
	let routed = click(&mut state, 17);
	assert_eq!(routed.messages, vec![Msg::Picked(2)]);

	// Уже выбранная вкладка: нажатие принято, сообщения нет.
	let routed = click(&mut state, 14);
	assert_eq!(routed.handled, Handled::Yes);
	assert!(routed.messages.is_empty());

	// Разделитель и пустое место справа — не вкладка.
	for col in [6, 12, 18, 23] {
		let routed = click(&mut state, col);
		assert_eq!(routed.handled, Handled::No, "колонка {col}");
		assert!(routed.messages.is_empty());
	}
	assert_eq!(state.selected(), 2);

	// Клавиша через дерево даёт то же сообщение привязки.
	let left = Key::Named(NamedKey::ArrowLeft);
	let routed = ui(&mut state).route_key(&key_msg(&left, ModifiersState::empty()));
	assert_eq!(routed.messages, vec![Msg::Picked(1)]);
}

#[test]
fn disabled_tabs_take_no_focus_and_no_input() {
	let mut state = TabsState::with_selected(1);
	state.enabled = false;
	assert!(!Control::<Msg>::focusable(&state));
	ui(&mut state).render(AREA, &mut screen(24, 2));
	assert_eq!(press(&mut state, NamedKey::ArrowRight), Handled::No);
	let routed = click(&mut state, 1);
	assert_eq!(routed.handled, Handled::No);
	assert_eq!(state.selected(), 1);
}

#[test]
fn tabs_inside_a_block_light_its_border_under_focus() {
	let widget = || {
		tabs()
			.block(Block::bordered())
			.focus_style(Style::new().fg(ACCENT))
	};
	let render = |state: &mut TabsState| {
		let mut s = screen(22, 3);
		widget().render(Rect::new(0, 0, 22, 3), &mut s, state);
		s
	};
	let mut state = TabsState::new();
	let s = render(&mut state);
	assert_screen(
		&s,
		&[
			"┌────────────────────┐",
			"│ Один │ Два │ Три   │",
			"└────────────────────┘",
		],
	);
	assert_eq!(s.cell(0, 0).fg, FG);

	state.node.focused = true;
	let s = render(&mut state);
	assert_eq!(s.cell(0, 0).fg, ACCENT, "рамка под фокусом");
	assert_eq!(s.cell(2, 1).fg, ACCENT, "выбранная вкладка под фокусом");

	// Клик считается по строке заголовков внутри рамки, а не по рамке.
	let mut out = Messages::<Msg>::new();
	assert_eq!(state.mouse(&left_down(9, 0), &mut out), Handled::No);
	assert_eq!(state.mouse(&left_down(9, 1), &mut out), Handled::Yes);
	assert_eq!(state.selected(), 1);
}
