//! Образец: элементы интерфейса, на которых видно действие темы, — поле ввода, выпадающий
//! список, флажки, вкладки, таблица с полосой прокрутки, индикаторы, абзац и кнопки. Своих
//! стилей у виджетов нет: всё красит тема, подключённая к описанию экрана.

use gterm_widgets::widgets::{
	Block, BorderType, Button, ButtonState, Checkbox, CheckboxState, ComboBox, ComboBoxState,
	Gauge, LineGauge, Paragraph, Row, Scrollbar, ScrollbarOrientation, Table, TableState, Tabs,
	TabsState, TextInput, TextInputState, Wrap,
};
use gterm_widgets::{Constraint, Layout, Line, Span, Text, Theme, Ui};

use crate::{Msg, gap, labeled};

const LABEL_WIDTH: u16 = 10;

const COMBO_ITEMS: [&str; 4] = ["первый", "второй", "третий", "четвёртый"];

const TABLE_ROWS: [[&str; 3]; 10] = [
	["01", "буфер", "готово"],
	["02", "шрифт", "готово"],
	["03", "атлас", "готово"],
	["04", "рендерер", "идёт"],
	["05", "наложение", "идёт"],
	["06", "виджеты", "ждёт"],
	["07", "контролы", "ждёт"],
	["08", "сочетания", "ждёт"],
	["09", "буфер обмена", "ждёт"],
	["10", "темы", "идёт"],
];

pub struct Sample {
	input: TextInputState,
	combo: ComboBoxState,
	check: CheckboxState,
	disabled_check: CheckboxState,
	tabs: TabsState,
	table: TableState,
	dialog: ButtonState,
	warnings: ButtonState,
	disabled_button: ButtonState,
}

impl Sample {
	pub fn new() -> Self {
		Self {
			input: TextInputState::new(),
			combo: ComboBoxState::new(),
			check: CheckboxState::with_checked(true),
			disabled_check: CheckboxState::disabled(),
			tabs: TabsState::new(),
			table: TableState::with_selected(Some(0)),
			dialog: ButtonState::new(),
			warnings: ButtonState::new(),
			disabled_button: ButtonState::disabled(),
		}
	}

	/// Описание образца; `ratio` — доля для индикаторов.
	pub fn ui<'a>(&'a mut self, theme: &'a Theme, ratio: f64) -> Ui<'a, Msg> {
		let dim = theme.style("ui.text.inactive");
		let is_table_page = self.tabs.selected() == 0;
		let page = if is_table_page {
			Ui::stateful(table(), &mut self.table)
		} else {
			indicators(theme, ratio)
		};
		Ui::group(
			Block::bordered()
				.title(" Образец ")
				.border_type(BorderType::Rounded),
			Layout::vertical([
				Constraint::Length(1),
				Constraint::Length(1),
				Constraint::Length(1),
				Constraint::Length(1),
				Constraint::Length(1),
				Constraint::Fill(1),
				Constraint::Length(1),
			])
			.margin(1),
			vec![
				labeled(
					"Поле",
					LABEL_WIDTH,
					dim,
					Ui::stateful(
						TextInput::new().placeholder("введите текст"),
						&mut self.input,
					),
				),
				labeled(
					"Список",
					LABEL_WIDTH,
					dim,
					Ui::combo_box(
						ComboBox::new(&COMBO_ITEMS).placeholder("выберите"),
						&mut self.combo,
						|_| Msg::Sample,
					),
				),
				Ui::split(
					Layout::horizontal([
						Constraint::Length(LABEL_WIDTH),
						Constraint::Length(12),
						Constraint::Length(16),
						Constraint::Fill(1),
					]),
					vec![
						gap(),
						Ui::stateful(Checkbox::new("флажок"), &mut self.check),
						Ui::stateful(Checkbox::new("неактивный"), &mut self.disabled_check),
					],
				),
				gap(),
				Ui::stateful(Tabs::new(["Таблица", "Индикаторы"]), &mut self.tabs),
				page,
				Ui::split(
					Layout::horizontal([
						Constraint::Length(8),
						Constraint::Length(1),
						Constraint::Length(16),
						Constraint::Length(1),
						Constraint::Length(11),
						Constraint::Fill(1),
					]),
					vec![
						Ui::stateful(Button::new("Диалог"), &mut self.dialog)
							.on_activate(Msg::OpenDialog),
						gap(),
						Ui::stateful(Button::new("Предупреждения"), &mut self.warnings)
							.on_activate(Msg::OpenWarnings),
						gap(),
						Ui::stateful(Button::new("Неактивна"), &mut self.disabled_button),
					],
				),
			],
		)
	}
}

fn table<'a>() -> Table<'a> {
	Table::new(
		TABLE_ROWS.map(Row::new),
		[
			Constraint::Length(3),
			Constraint::Fill(1),
			Constraint::Length(8),
		],
	)
	.header(Row::new(["№", "задача", "статус"]))
	.block(Block::bordered().title(" Таблица "))
	.highlight_symbol("» ")
	.scrollbar(Scrollbar::new(ScrollbarOrientation::Vertical))
}

fn indicators<'a>(theme: &Theme, ratio: f64) -> Ui<'a, Msg> {
	let text = Text::from(vec![
		Line::from(vec![
			Span::raw("Обычный текст, "),
			Span::styled("текст под фокусом", theme.style("ui.text.focus")),
			Span::raw(" и "),
			Span::styled("неактивный текст", theme.style("ui.text.inactive")),
			Span::raw("."),
		]),
		Line::raw("Индикаторы показывают место темы в перечне."),
	]);
	Ui::split(
		Layout::vertical([
			Constraint::Length(3),
			Constraint::Length(1),
			Constraint::Fill(1),
		]),
		vec![
			Ui::widget(
				Gauge::new()
					.ratio(ratio)
					.block(Block::bordered().title(" Gauge ")),
			),
			Ui::widget(LineGauge::new().ratio(ratio).label(" LineGauge:")),
			Ui::widget(
				Paragraph::new(text)
					.block(
						Block::bordered()
							.title(" Абзац ")
							.border_type(BorderType::Thick),
					)
					.wrap(Wrap { trim: true }),
			),
		],
	)
}
