//! Тема в дереве `Ui` (ADR-0037, ТЗ К5): базовый стиль области, рамки и рамка под фокусом,
//! модальный слой, все контролы по состояниям, запасные цепочки для темы без своих областей,
//! приоритет явного стиля, свои виджеты и контролы приложения.

mod common;

use common::{BG, CallLog, FG, Msg, Stub, call_log, screen};
use gterm::buffer::ScreenBuffer;
use gterm::cell::{Cell, CellFlags, Color};
use gterm_widgets::input::{Handled, KeyMsg, Messages, MouseMsg};
use gterm_widgets::widgets::{
	Block, Button, ButtonState, Checkbox, CheckboxState, ComboBox, ComboBoxState, Gauge, LineGauge,
	List, ListState, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table,
	TableState, Tabs, TabsState, TextInput, TextInputState,
};
use gterm_widgets::{
	BufferExt, Constraint, Control, Drawable, Layout, Modal, NodeState, Rect, StatefulWidget,
	Style, Theme, Ui, Widget,
};

const fn rgb(value: u32) -> Color {
	Color(0xFF00_0000 | value)
}

const GROUND: Color = rgb(0x000001);
const INK: Color = rgb(0x000002);
const INK_FOCUS: Color = rgb(0x000003);
const INACTIVE: Color = rgb(0x000004);
const BORDER: Color = rgb(0x000005);
const BORDER_FOCUS: Color = rgb(0x000006);
const POPUP: Color = rgb(0x000007);
const MENU_FG: Color = rgb(0x000008);
const MENU_BG: Color = rgb(0x000009);
const MENU_SELECTED: Color = rgb(0x00000A);
const SCROLL_THUMB: Color = rgb(0x00000B);
const SCROLL_TRACK: Color = rgb(0x00000C);
const SELECTION: Color = rgb(0x00000D);

const BUTTON_FG: Color = rgb(0x000101);
const BUTTON_BG: Color = rgb(0x000102);
const BUTTON_FOCUS: Color = rgb(0x000103);
const BUTTON_PRESSED: Color = rgb(0x000104);
const BUTTON_DISABLED: Color = rgb(0x000105);
const CHECK_FOCUS: Color = rgb(0x000201);
const INPUT_BG: Color = rgb(0x000301);
const INPUT_FOCUS: Color = rgb(0x000302);
const INPUT_SELECTION: Color = rgb(0x000303);
const INPUT_HINT: Color = rgb(0x000304);
const LIST_SELECTED: Color = rgb(0x000401);
const TABLE_HEADER: Color = rgb(0x000501);
const TABLE_SELECTED: Color = rgb(0x000502);
const TABS_SELECTED: Color = rgb(0x000601);
const BAR_TRACK: Color = rgb(0x000701);
const BAR_THUMB: Color = rgb(0x000702);
const GAUGE_FILLED: Color = rgb(0x000801);
const GAUGE_TRACK: Color = rgb(0x000802);
const GAUGE_UNFILLED: Color = rgb(0x000803);
const BADGE: Color = rgb(0x000901);

const EXPLICIT: Color = rgb(0x0F0F0F);

/// Тема только из областей helix: своих областей виджетов в ней нет.
fn helix_theme() -> Theme {
	let style = Style::new();
	let mut theme = Theme::new("helix-like");
	theme
		.set("ui.background", style.bg(GROUND))
		.set("ui.text", style.fg(INK))
		.set("ui.text.focus", style.fg(INK_FOCUS).bold())
		.set("ui.text.inactive", style.fg(INACTIVE))
		.set("ui.window", style.fg(BORDER).bg(MENU_BG))
		.set("ui.popup", style.bg(POPUP))
		.set("ui.menu", style.fg(MENU_FG).bg(MENU_BG))
		.set("ui.menu.selected", style.bg(MENU_SELECTED))
		.set("ui.menu.scroll", style.fg(SCROLL_THUMB).bg(SCROLL_TRACK))
		.set("ui.selection", style.bg(SELECTION));
	theme
}

/// Та же тема со своими областями виджетов и областью приложения.
fn full_theme() -> Theme {
	let style = Style::new();
	let mut theme = helix_theme();
	theme
		.set("ui.window.focus", style.fg(BORDER_FOCUS))
		.set("ui.button", style.fg(BUTTON_FG).bg(BUTTON_BG))
		.set("ui.button.focus", style.bg(BUTTON_FOCUS))
		.set("ui.button.pressed", style.bg(BUTTON_PRESSED))
		.set("ui.button.disabled", style.fg(BUTTON_DISABLED))
		.set("ui.checkbox.focus", style.fg(CHECK_FOCUS))
		.set("ui.input", style.bg(INPUT_BG))
		.set("ui.input.focus", style.bg(INPUT_FOCUS))
		.set("ui.input.selection", style.bg(INPUT_SELECTION))
		.set("ui.input.placeholder", style.fg(INPUT_HINT))
		.set("ui.list.selected", style.bg(LIST_SELECTED))
		.set("ui.table.header", style.fg(TABLE_HEADER))
		.set("ui.table.selected", style.bg(TABLE_SELECTED))
		.set("ui.tabs.selected", style.fg(TABS_SELECTED))
		.set("ui.scrollbar", style.fg(BAR_TRACK))
		.set("ui.scrollbar.thumb", style.fg(BAR_THUMB))
		.set("ui.gauge.filled", style.fg(GAUGE_FILLED).bg(GAUGE_TRACK))
		.set("ui.gauge.unfilled", style.fg(GAUGE_UNFILLED))
		.set("myapp.badge", style.fg(BADGE));
	theme
}

fn cell(buf: &ScreenBuffer, x: usize, y: usize) -> Cell {
	buf.cell(x, y)
}

fn colors(buf: &ScreenBuffer, x: usize, y: usize) -> (Color, Color) {
	let cell = cell(buf, x, y);
	(cell.fg, cell.bg)
}

/// Две кнопки в ряд по 4 ячейки: первая под фокусом, вторая — нет.
fn two_buttons<'a>(
	theme: &'a Theme,
	first: &'a mut ButtonState,
	second: &'a mut ButtonState,
	second_widget: Button<'a>,
) -> Ui<'a, Msg> {
	Ui::split(
		Layout::horizontal([Constraint::Length(4), Constraint::Length(4)]),
		vec![
			Ui::stateful(Button::new("A"), first),
			Ui::stateful(second_widget, second),
		],
	)
	.theme(theme)
}

#[test]
fn themed_ui_paints_its_area_with_the_base_style_and_text_takes_it() {
	let theme = full_theme();
	let mut buf = screen(6, 2);
	Ui::<Msg>::widget(Paragraph::new("ab"))
		.theme(&theme)
		.render(Rect::new(0, 0, 4, 1), &mut buf);
	assert_eq!(cell(&buf, 0, 0).ch, 'a');
	assert_eq!(colors(&buf, 0, 0), (INK, GROUND));
	// Пустая ячейка области тоже получает базовый стиль, ячейки вне области — нет.
	assert_eq!(colors(&buf, 3, 0), (INK, GROUND));
	assert_eq!(colors(&buf, 4, 0), (FG, BG));
	assert_eq!(colors(&buf, 0, 1), (FG, BG));
}

#[test]
fn without_a_theme_nothing_is_painted() {
	let mut buf = screen(8, 1);
	let (mut first, mut second) = (ButtonState::new(), ButtonState::new());
	Ui::<Msg>::split(
		Layout::horizontal([Constraint::Length(4), Constraint::Length(4)]),
		vec![
			Ui::stateful(Button::new("A"), &mut first),
			Ui::stateful(Button::new("B"), &mut second),
		],
	)
	.render(Rect::new(0, 0, 8, 1), &mut buf);
	for x in 0..8 {
		assert_eq!(colors(&buf, x, 0), (FG, BG), "ячейка {x}");
	}
}

#[test]
fn button_takes_its_scopes_in_every_state() {
	let theme = full_theme();
	let area = Rect::new(0, 0, 8, 1);

	let mut buf = screen(8, 1);
	let (mut first, mut second) = (ButtonState::new(), ButtonState::new());
	two_buttons(&theme, &mut first, &mut second, Button::new("B")).render(area, &mut buf);
	assert_eq!(colors(&buf, 0, 0), (BUTTON_FG, BUTTON_FOCUS), "под фокусом");
	assert_eq!(colors(&buf, 4, 0), (BUTTON_FG, BUTTON_BG), "обычная");

	let mut buf = screen(8, 1);
	second.pressed = true;
	two_buttons(&theme, &mut first, &mut second, Button::new("B")).render(area, &mut buf);
	assert_eq!(colors(&buf, 4, 0), (BUTTON_FG, BUTTON_PRESSED), "нажата");

	let mut buf = screen(8, 1);
	let mut disabled = ButtonState::disabled();
	two_buttons(&theme, &mut first, &mut disabled, Button::new("B")).render(area, &mut buf);
	assert_eq!(
		colors(&buf, 4, 0),
		(BUTTON_DISABLED, BUTTON_BG),
		"неактивная"
	);
}

#[test]
fn theme_without_widget_scopes_falls_back_to_helix_scopes() {
	let theme = helix_theme();
	let area = Rect::new(0, 0, 8, 1);

	let mut buf = screen(8, 1);
	let (mut first, mut second) = (ButtonState::new(), ButtonState::new());
	two_buttons(&theme, &mut first, &mut second, Button::new("B")).render(area, &mut buf);
	assert_eq!(colors(&buf, 0, 0), (MENU_FG, MENU_SELECTED), "под фокусом");
	assert_eq!(colors(&buf, 4, 0), (MENU_FG, MENU_BG), "обычная");

	let mut buf = screen(8, 1);
	second.pressed = true;
	two_buttons(&theme, &mut first, &mut second, Button::new("B")).render(area, &mut buf);
	assert_eq!(colors(&buf, 4, 0), (MENU_FG, SELECTION), "нажата");

	let mut buf = screen(8, 1);
	let mut disabled = ButtonState::disabled();
	two_buttons(&theme, &mut first, &mut disabled, Button::new("B")).render(area, &mut buf);
	assert_eq!(colors(&buf, 4, 0), (INACTIVE, MENU_BG), "неактивная");
}

#[test]
fn explicit_style_is_stronger_than_the_theme() {
	let theme = full_theme();
	let mut buf = screen(8, 1);
	let (mut first, mut second) = (ButtonState::new(), ButtonState::new());
	let explicit = Button::new("B").style(Style::new().bg(EXPLICIT));
	two_buttons(&theme, &mut first, &mut second, explicit).render(Rect::new(0, 0, 8, 1), &mut buf);
	// Заданный фон остаётся, незаданный цвет символа приходит из темы.
	assert_eq!(colors(&buf, 4, 0), (BUTTON_FG, EXPLICIT));
}

#[test]
fn themed_widget_outside_the_tree_matches_the_tree() {
	let theme = full_theme();
	let area = Rect::new(0, 0, 4, 1);

	let mut in_tree = screen(4, 1);
	let mut state = ButtonState::new();
	Ui::<Msg>::stateful(Button::new("A"), &mut state)
		.theme(&theme)
		.render(area, &mut in_tree);

	let mut outside = screen(4, 1);
	outside.set_style(area, theme.base());
	StatefulWidget::render(
		Button::new("A").themed(&theme),
		area,
		&mut outside,
		&mut state,
	);
	for x in 0..4 {
		assert_eq!(cell(&in_tree, x, 0), cell(&outside, x, 0), "ячейка {x}");
	}
}

#[test]
fn group_border_is_window_and_focus_border_inside_focus() {
	let theme = full_theme();
	let layout = || Layout::vertical([Constraint::Length(1)]);
	let area = Rect::new(0, 0, 6, 3);

	let mut buf = screen(6, 3);
	Ui::<Msg>::group(
		Block::bordered(),
		layout(),
		vec![Ui::widget(Paragraph::new("x"))],
	)
	.theme(&theme)
	.render(area, &mut buf);
	// Из `ui.window` рамка берёт только цвет символа: фон этой области — не её.
	assert_eq!(colors(&buf, 0, 0), (BORDER, GROUND), "без фокуса");

	let mut buf = screen(6, 3);
	let mut state = ButtonState::new();
	Ui::<Msg>::group(
		Block::bordered(),
		layout(),
		vec![Ui::stateful(Button::new("A"), &mut state)],
	)
	.theme(&theme)
	.render(area, &mut buf);
	assert_eq!(colors(&buf, 0, 0), (BORDER_FOCUS, GROUND), "фокус внутри");

	// Явный стиль фокуса группы сильнее темы.
	let mut buf = screen(6, 3);
	Ui::<Msg>::group(
		Block::bordered(),
		layout(),
		vec![Ui::stateful(Button::new("A"), &mut state)],
	)
	.focus_style(Style::new().fg(EXPLICIT))
	.theme(&theme)
	.render(area, &mut buf);
	assert_eq!(cell(&buf, 0, 0).fg, EXPLICIT);

	// В теме без `ui.window.focus` рамка под фокусом — цвет символа `ui.text.focus`.
	let helix = helix_theme();
	let mut buf = screen(6, 3);
	Ui::<Msg>::group(
		Block::bordered(),
		layout(),
		vec![Ui::stateful(Button::new("A"), &mut state)],
	)
	.theme(&helix)
	.render(area, &mut buf);
	let corner = cell(&buf, 0, 0);
	assert_eq!(corner.fg, INK_FOCUS);
	assert_eq!(corner.flags, CellFlags::empty(), "только цвет символа");
}

#[test]
fn modal_layer_is_popup_colored_and_passes_the_theme_to_its_content() {
	let theme = full_theme();
	let screen_area = Rect::new(0, 0, 12, 6);
	let layer = |style: Option<Style>| {
		let mut state = ButtonState::new();
		let mut modal = Modal::centered(6, 2).content(Ui::split(
			Layout::vertical([Constraint::Length(1), Constraint::Length(1)]),
			vec![
				Ui::widget(Paragraph::new("m")),
				Ui::stateful(Button::new("OK"), &mut state),
			],
		));
		if let Some(style) = style {
			modal = modal.style(style);
		}
		Ui::<Msg>::widget(Paragraph::new("screen"))
			.modal(modal)
			.theme(&theme)
			.modal_buffer(screen_area)
			.expect("слой")
			.1
	};

	let buffer = layer(None);
	assert_eq!(cell(&buffer, 0, 0).ch, 'm');
	assert_eq!(colors(&buffer, 0, 0), (INK, POPUP), "текст на фоне слоя");
	assert_eq!(
		colors(&buffer, 0, 1),
		(BUTTON_FG, BUTTON_FOCUS),
		"кнопка слоя"
	);

	let buffer = layer(Some(Style::new().bg(EXPLICIT)));
	assert_eq!(colors(&buffer, 0, 0), (INK, EXPLICIT), "явный фон слоя");

	// Темы без `ui.popup`: слой на фоне экрана.
	let mut plain = Theme::new("plain");
	plain
		.set("ui.background", Style::new().bg(GROUND))
		.set("ui.text", Style::new().fg(INK));
	let buffer = Ui::<Msg>::widget(Paragraph::new("screen"))
		.modal(Modal::centered(6, 2).content(Ui::widget(Paragraph::new("m"))))
		.theme(&plain)
		.modal_buffer(screen_area)
		.expect("слой")
		.1;
	assert_eq!(colors(&buffer, 0, 0), (INK, GROUND));
}

#[test]
fn checkbox_takes_focus_and_disabled_styles() {
	let theme = full_theme();
	let mut buf = screen(8, 2);
	let (mut focused, mut disabled) = (CheckboxState::new(), CheckboxState::disabled());
	Ui::<Msg>::split(
		Layout::vertical([Constraint::Length(1), Constraint::Length(1)]),
		vec![
			Ui::stateful(Checkbox::new("a"), &mut focused),
			Ui::stateful(Checkbox::new("b"), &mut disabled),
		],
	)
	.theme(&theme)
	.render(Rect::new(0, 0, 8, 2), &mut buf);
	assert_eq!(colors(&buf, 0, 0), (CHECK_FOCUS, GROUND), "под фокусом");
	assert_eq!(colors(&buf, 0, 1), (INACTIVE, GROUND), "неактивный");
}

#[test]
fn text_input_takes_field_focus_placeholder_and_selection_styles() {
	let theme = full_theme();
	let mut buf = screen(8, 3);
	let mut selected = TextInputState::with_value("abc");
	selected.select_all();
	let mut plain = TextInputState::with_value("xyz");
	let mut empty = TextInputState::new();
	Ui::<Msg>::split(
		Layout::vertical([
			Constraint::Length(1),
			Constraint::Length(1),
			Constraint::Length(1),
		]),
		vec![
			Ui::stateful(TextInput::new(), &mut selected),
			Ui::stateful(TextInput::new(), &mut plain),
			Ui::stateful(TextInput::new().placeholder("hint"), &mut empty),
		],
	)
	.theme(&theme)
	.render(Rect::new(0, 0, 8, 3), &mut buf);
	assert_eq!(cell(&buf, 0, 0).bg, INPUT_SELECTION, "выделение");
	assert_eq!(cell(&buf, 5, 0).bg, INPUT_FOCUS, "поле под фокусом");
	assert_eq!(colors(&buf, 0, 1), (INK, INPUT_BG), "поле без фокуса");
	assert_eq!(colors(&buf, 0, 2), (INPUT_HINT, INPUT_BG), "подсказка");
}

/// Выпадающий список в первой строке экрана.
fn combo_ui<'a>(
	theme: &'a Theme,
	items: &'a [&'a str],
	state: &'a mut ComboBoxState,
) -> Ui<'a, Msg> {
	Ui::split(
		Layout::vertical([Constraint::Length(1)]),
		vec![Ui::combo_box(ComboBox::new(items), state, Msg::Picked)],
	)
	.theme(theme)
}

#[test]
fn combo_box_field_and_dropdown_take_the_theme() {
	let theme = full_theme();
	let items = ["one", "two"];
	let screen_area = Rect::new(0, 0, 10, 8);
	let mut state = ComboBoxState::with_selected(Some(0));

	let mut buf = screen(10, 8);
	combo_ui(&theme, &items, &mut state).render(screen_area, &mut buf);
	// Своих областей `ui.combo.*` в теме нет: поле — как поле ввода под фокусом.
	assert_eq!(colors(&buf, 0, 0), (INK, INPUT_FOCUS));

	state.open_list();
	let (area, buffer) = combo_ui(&theme, &items, &mut state)
		.modal_buffer(screen_area)
		.expect("слой списка");
	assert_eq!(area, Rect::new(0, 1, 10, 4));
	assert_eq!(
		cell(&buffer, 0, 0).fg,
		BORDER_FOCUS,
		"рамка слоя под фокусом"
	);
	assert_eq!(cell(&buffer, 1, 1).ch, 'o');
	assert_eq!(
		colors(&buffer, 1, 1),
		(INK, LIST_SELECTED),
		"выделенный пункт"
	);
	assert_eq!(
		colors(&buffer, 1, 2),
		(INK, POPUP),
		"остальные — на фоне слоя"
	);
}

#[test]
fn list_takes_selection_scrollbar_and_focus_border() {
	let theme = full_theme();
	let mut buf = screen(6, 6);
	let mut state = ListState::with_selected(Some(1));
	let list = List::new(["a", "b", "c", "d", "e", "f", "g", "h"])
		.block(Block::bordered())
		.scrollbar(Scrollbar::new(ScrollbarOrientation::Vertical));
	Ui::<Msg>::stateful(list, &mut state)
		.theme(&theme)
		.render(Rect::new(0, 0, 6, 6), &mut buf);
	assert_eq!(cell(&buf, 0, 0).fg, BORDER_FOCUS, "рамка под фокусом");
	assert_eq!(colors(&buf, 1, 1), (INK, GROUND), "обычный пункт");
	assert_eq!(colors(&buf, 1, 2), (INK, LIST_SELECTED), "выделенный пункт");
	// Полоса — колонка 4 внутри рамки: стрелка сверху, ниже дорожка с ползунком.
	assert_eq!(cell(&buf, 4, 1).ch, '▲');
	assert_eq!(cell(&buf, 4, 1).fg, BAR_TRACK);
	let track: Vec<Color> = (2..4).map(|y| cell(&buf, 4, y).fg).collect();
	assert!(track.contains(&BAR_THUMB), "ползунок: {track:?}");
	assert!(track.contains(&BAR_TRACK), "дорожка: {track:?}");
}

#[test]
fn table_takes_header_and_selected_row_styles() {
	let theme = full_theme();
	let mut buf = screen(4, 3);
	let mut state = TableState::with_selected(Some(0));
	let table = Table::new([Row::new(["a"]), Row::new(["b"])], [Constraint::Fill(1)])
		.header(Row::new(["H"]));
	Ui::<Msg>::stateful(table, &mut state)
		.theme(&theme)
		.render(Rect::new(0, 0, 4, 3), &mut buf);
	assert_eq!(colors(&buf, 0, 0), (TABLE_HEADER, GROUND), "заголовок");
	assert_eq!(
		colors(&buf, 0, 1),
		(INK, TABLE_SELECTED),
		"выделенная строка"
	);
	assert_eq!(colors(&buf, 0, 2), (INK, GROUND), "обычная строка");
}

#[test]
fn tabs_take_selected_and_focus_styles_over_the_default_highlight() {
	let theme = full_theme();
	let area = Rect::new(0, 0, 8, 1);

	let mut buf = screen(8, 1);
	let mut state = TabsState::new();
	state.enabled = false;
	Ui::<Msg>::stateful(Tabs::new(["x", "y"]), &mut state)
		.theme(&theme)
		.render(area, &mut buf);
	let selected = cell(&buf, 1, 0);
	assert_eq!(selected.ch, 'x');
	assert_eq!((selected.fg, selected.bg), (TABS_SELECTED, GROUND));
	// Жирный с подчёркиванием — стиль выбранной вкладки по умолчанию — остаётся поверх темы.
	assert_eq!(selected.flags, CellFlags::BOLD | CellFlags::UNDERLINE);
	assert_eq!(colors(&buf, 5, 0), (INK, GROUND), "невыбранная вкладка");

	// Под фокусом: своей области `ui.tabs.focus` нет — берётся `ui.text.focus`.
	let mut buf = screen(8, 1);
	let mut state = TabsState::new();
	Ui::<Msg>::stateful(Tabs::new(["x", "y"]), &mut state)
		.theme(&theme)
		.render(area, &mut buf);
	assert_eq!(cell(&buf, 1, 0).fg, INK_FOCUS);
}

#[test]
fn gauges_and_scrollbar_take_their_scopes() {
	let theme = full_theme();
	let mut buf = screen(4, 2);
	Ui::<Msg>::split(
		Layout::vertical([Constraint::Length(1), Constraint::Length(1)]),
		vec![
			Ui::widget(Gauge::new().ratio(0.5).label("")),
			Ui::widget(LineGauge::new().ratio(0.5).label("")),
		],
	)
	.theme(&theme)
	.render(Rect::new(0, 0, 4, 2), &mut buf);
	assert_eq!(colors(&buf, 0, 0), (GAUGE_FILLED, GAUGE_TRACK), "заливка");
	assert_eq!(cell(&buf, 3, 0).bg, GAUGE_TRACK, "незалитая часть");
	// У линии — только цвет символа: фон остаётся фоном экрана.
	assert_eq!(colors(&buf, 1, 1), (GAUGE_FILLED, GROUND), "линия");
	assert_eq!(
		colors(&buf, 3, 1),
		(GAUGE_UNFILLED, GROUND),
		"остаток линии"
	);

	// Тема без своих областей: ползунок — цвет символа `ui.menu.scroll`, дорожка — как рамка.
	let helix = helix_theme();
	let mut buf = screen(1, 6);
	let bar = Scrollbar::new(ScrollbarOrientation::Vertical)
		.state(ScrollbarState::new(40).viewport_length(4))
		.themed(&helix);
	bar.render(Rect::new(0, 0, 1, 6), &mut buf);
	assert_eq!(cell(&buf, 0, 0).fg, BORDER, "стрелка");
	assert_eq!(cell(&buf, 0, 1).fg, SCROLL_THUMB, "ползунок");
	assert_eq!(cell(&buf, 0, 4).fg, BORDER, "дорожка");
}

/// Свой контрол приложения, берущий из темы свою область.
struct Badge {
	node: NodeState,
	style: Style,
}

impl Control<Msg> for Badge {
	fn node(&self) -> &NodeState {
		&self.node
	}

	fn node_mut(&mut self) -> &mut NodeState {
		&mut self.node
	}

	fn key(&mut self, _msg: &KeyMsg<'_>, _out: &mut Messages<Msg>) -> Handled {
		Handled::No
	}

	fn mouse(&mut self, _msg: &MouseMsg, _out: &mut Messages<Msg>) -> Handled {
		Handled::No
	}
}

impl Drawable for Badge {
	fn draw(&mut self, area: Rect, buf: &mut ScreenBuffer) {
		buf.fill(area, '*', self.style);
	}

	fn apply_theme(&mut self, theme: &Theme) {
		self.style = theme.style("myapp.badge.active");
	}
}

#[test]
fn application_controls_compile_unchanged_and_may_read_their_own_scopes() {
	let theme = full_theme();
	let log: CallLog = call_log();
	// Заглушка тестов `apply_theme` не реализует и рисует своими цветами.
	let mut stub = Stub::new("stub", false, &log);
	let mut badge = Badge {
		node: NodeState::default(),
		style: Style::new(),
	};
	let mut buf = screen(4, 2);
	Ui::split(
		Layout::vertical([Constraint::Length(1), Constraint::Length(1)]),
		vec![Ui::control(&mut stub), Ui::control(&mut badge)],
	)
	.theme(&theme)
	.render(Rect::new(0, 0, 4, 2), &mut buf);
	assert_eq!(colors(&buf, 0, 0), (FG, BG), "контрол без темы");
	assert_eq!(cell(&buf, 0, 1).ch, '*');
	// Область приложения с наследованием по точке; фон — базовый стиль экрана.
	assert_eq!(colors(&buf, 0, 1), (BADGE, GROUND));
}
