//! Диалоговые окна образца — модальные слои, на которых видно действие темы (`ui.popup` и
//! области контролов внутри): форма с полем, флажком и кнопками и перечень того, что тема при
//! загрузке отобразить не смогла (`Theme::warnings`). Третье окно — рабочее: путь файла для
//! полной записи темы (`Theme::to_toml_string`).

use gterm_widgets::widgets::{
	Block, BorderType, Button, ButtonState, Checkbox, CheckboxState, List, ListState, Paragraph,
	Scrollbar, ScrollbarOrientation, TextInput, TextInputState,
};
use gterm_widgets::{Constraint, HotkeyMap, Layout, Line, Modal, Theme, Ui};

use crate::{Msg, gap, labeled};

const LABEL_WIDTH: u16 = 6;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
	Form,
	Warnings,
	Save,
}

#[derive(Default)]
pub struct Dialogs {
	open: Option<Kind>,
	name: TextInputState,
	remember: CheckboxState,
	ok: ButtonState,
	cancel: ButtonState,
	warnings: ListState,
	close: ButtonState,
	/// Путь файла в окне сохранения.
	save_path: TextInputState,
	save: ButtonState,
	save_cancel: ButtonState,
}

impl Dialogs {
	pub fn open(&mut self, kind: Kind) {
		self.open = Some(kind);
	}

	pub fn close(&mut self) {
		self.open = None;
	}

	/// Открыть окно сохранения с предложенным путём файла.
	pub fn open_save(&mut self, path: String) {
		self.save_path = TextInputState::with_value(path);
		self.open = Some(Kind::Save);
	}

	/// Путь файла, набранный в окне сохранения.
	pub fn save_path(&self) -> &str {
		self.save_path.value()
	}

	/// Слой открытого окна; `title` — подпись темы, `keys` — карта сочетаний слоя.
	pub fn modal<'a>(
		&'a mut self,
		theme: &'a Theme,
		title: &str,
		keys: &'a HotkeyMap<Msg>,
	) -> Option<Modal<'a, Msg>> {
		let modal = match self.open? {
			Kind::Form => Modal::centered(64, 10)
				.content(self.form(theme))
				.hotkeys(keys),
			Kind::Warnings => Modal::centered(84, 18)
				.content(self.warnings(theme, title))
				.hotkeys(keys),
			// Без карты сочетаний: смена темы под окном разошлась бы с именем файла в поле.
			Kind::Save => Modal::centered(100, 9).content(self.save(theme, title)),
		};
		Some(modal.on_escape(Msg::CloseDialog))
	}

	fn form<'a>(&'a mut self, theme: &Theme) -> Ui<'a, Msg> {
		let dim = theme.style("ui.text.inactive");
		Ui::group(
			Block::bordered()
				.title(" Диалог ")
				.border_type(BorderType::Rounded),
			Layout::vertical([
				Constraint::Length(1),
				Constraint::Length(1),
				Constraint::Length(1),
				Constraint::Length(1),
				Constraint::Fill(1),
				Constraint::Length(1),
			])
			.margin(1),
			vec![
				Ui::widget(Paragraph::new(Line::styled(
					"Фон окна — область ui.popup; F2/F3 меняют тему и здесь.",
					dim,
				))),
				gap(),
				labeled(
					"Имя",
					LABEL_WIDTH,
					dim,
					Ui::stateful(TextInput::new().placeholder("как назвать"), &mut self.name),
				),
				Ui::stateful(Checkbox::new("запомнить выбор"), &mut self.remember),
				gap(),
				Ui::split(
					Layout::horizontal([
						Constraint::Fill(1),
						Constraint::Length(8),
						Constraint::Length(2),
						Constraint::Length(10),
						Constraint::Fill(1),
					]),
					vec![
						gap(),
						Ui::stateful(Button::new("OK"), &mut self.ok).on_activate(Msg::CloseDialog),
						gap(),
						Ui::stateful(Button::new("Отмена"), &mut self.cancel)
							.on_activate(Msg::CloseDialog),
					],
				),
			],
		)
	}

	fn save<'a>(&'a mut self, theme: &Theme, title: &str) -> Ui<'a, Msg> {
		let dim = theme.style("ui.text.inactive");
		Ui::group(
			Block::bordered()
				.title(format!(" Сохранить тему {title} "))
				.border_type(BorderType::Rounded),
			Layout::vertical([
				Constraint::Length(1),
				Constraint::Length(1),
				Constraint::Length(1),
				Constraint::Fill(1),
				Constraint::Length(1),
			])
			.margin(1),
			vec![
				Ui::widget(Paragraph::new(Line::styled(
					"Полная запись: все области, включая выведенные. Существующий файл не заменяется.",
					dim,
				))),
				gap(),
				labeled(
					"Файл",
					LABEL_WIDTH,
					dim,
					Ui::stateful(TextInput::new(), &mut self.save_path)
						.on_submit(|_| Msg::SaveTheme),
				),
				gap(),
				Ui::split(
					Layout::horizontal([
						Constraint::Fill(1),
						Constraint::Length(13),
						Constraint::Length(2),
						Constraint::Length(10),
						Constraint::Fill(1),
					]),
					vec![
						gap(),
						Ui::stateful(Button::new("Сохранить"), &mut self.save)
							.on_activate(Msg::SaveTheme),
						gap(),
						Ui::stateful(Button::new("Отмена"), &mut self.save_cancel)
							.on_activate(Msg::CloseDialog),
					],
				),
			],
		)
	}

	fn warnings<'a>(&'a mut self, theme: &'a Theme, title: &str) -> Ui<'a, Msg> {
		let warnings = theme.warnings();
		let body = if warnings.is_empty() {
			Ui::widget(Paragraph::new(
				"Предупреждений нет: всё, что задано в теме, отображается.",
			))
		} else {
			Ui::stateful(
				List::new(warnings.iter().map(String::as_str))
					.scrollbar(Scrollbar::new(ScrollbarOrientation::Vertical)),
				&mut self.warnings,
			)
		};
		Ui::group(
			Block::bordered()
				.title(format!(
					" Предупреждения темы {title} · {} ",
					warnings.len()
				))
				.border_type(BorderType::Rounded),
			Layout::vertical([
				Constraint::Fill(1),
				Constraint::Length(1),
				Constraint::Length(1),
			])
			.margin(1),
			vec![
				body,
				gap(),
				Ui::split(
					Layout::horizontal([
						Constraint::Fill(1),
						Constraint::Length(8),
						Constraint::Fill(1),
					]),
					vec![
						gap(),
						Ui::stateful(Button::new("OK"), &mut self.close)
							.on_activate(Msg::CloseDialog),
					],
				),
			],
		)
	}
}
