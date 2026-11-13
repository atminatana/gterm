//! Форма из контролов `gterm-widgets` (ADR-0026, ADR-0027, ADR-0029) на системном моноширинном шрифте:
//! три поля ввода, выпадающий список, группа «Параметры» с двумя флажками (ADR-0031; пробел, Enter или
//! клик — переключить), кнопки «Применить» и «Сброс», строка статуса. Экран
//! описан один раз — `Form::ui()`; текст приходит через `on_text` и идёт полю под фокусом,
//! `Tab`/`Shift+Tab` ходят по кольцу поля → список → кнопки, `Enter` в поле — «Применить»,
//! список раскрывается `Enter`/пробелом/`↓`/кликом слоем под полем и закрывается выбором, `Esc`
//! или кликом мимо; `Esc` на форме — выход. Сочетания из карты экрана (ADR-0029) не зависят от
//! раскладки и работают при фокусе в поле: `Ctrl+S` — «Применить», `Ctrl+R` — «Сброс».
//! В полях — выделение (`Shift` со стрелками, мышь, `Ctrl+A`) и системный буфер обмена
//! (`Ctrl+C`/`Ctrl+X`/`Ctrl+V`, ADR-0030).
//! Запуск: `cargo run -p gterm-widgets --example widget_edit`.

use std::error::Error;
use std::process::ExitCode;

use gterm::app::{App, AppHandler, AppOptions, Ctx, DrawCtx, Grid, GridMode, PointerEvent};
use gterm::buffer::ScreenBuffer;
use gterm::cell::{Color, Style as FontStyle};
use gterm::config::{BACKEND_ENV, Config};
use gterm::font::{FONT_SCALE, FontFamily, glyph_px_for};
use gterm::hotkey::HotkeyTracker;
use gterm::rasterizer::Rasterizer;
use gterm_widgets::widgets::{
	Block, BorderType, Button, ButtonState, Checkbox, CheckboxState, ComboBox, ComboBoxState,
	Paragraph, TextInput, TextInputState,
};
use gterm_widgets::{Constraint, HotkeyMap, Layout, Line, Rect, Span, Style, SystemClipboard, Ui};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, NamedKey};

const FONT_SIZE_PX: u16 = 14;
const COLS: usize = 80;
const ROWS: usize = 24;

const FG: Color = Color::from_rgb(0xD8, 0xD8, 0xD8);
const BG: Color = Color::from_rgb(0x10, 0x14, 0x1C);
const ACCENT: Color = Color::from_rgb(0x80, 0xC0, 0xFF);
const DIM: Color = Color::from_rgb(0x80, 0x80, 0x80);
const FIELD_BG: Color = Color::from_rgb(0x20, 0x28, 0x38);
const FOCUS_BG: Color = Color::from_rgb(0x2A, 0x4A, 0x7A);

const KINDS: [&str; 4] = ["файл", "каталог", "ссылка", "устройство"];

fn main() -> ExitCode {
	let env_backend = std::env::var(BACKEND_ENV).ok();
	let config = match Config::parse(std::env::args().skip(1), env_backend.as_deref()) {
		Ok(config) if config.help => {
			println!("{}", Config::usage("widget_edit"));
			return ExitCode::SUCCESS;
		}
		Ok(config) => config,
		Err(e) => {
			eprintln!("widget_edit: {e}");
			return ExitCode::FAILURE;
		}
	};
	match run(config) {
		Ok(()) => ExitCode::SUCCESS,
		Err(e) => {
			eprintln!("widget_edit: {e}");
			ExitCode::FAILURE
		}
	}
}

fn run(config: Config) -> Result<(), Box<dyn Error>> {
	println!("widget_edit: searching for system monospace font (this may take a moment)...");
	let family = FontFamily::system_monospace()?;
	eprintln!(
		"widget_edit: font '{}' — {}",
		family.family(),
		family.face(FontStyle::Regular).path.display()
	);
	let glyph_px = glyph_px_for(FONT_SIZE_PX, FONT_SCALE, 1.0);
	let rasterizer = Rasterizer::new_at(family, glyph_px)?;
	let screen = ScreenBuffer::with_default_colors(COLS, ROWS, FG, BG);
	let options = AppOptions {
		grid_mode: GridMode::AutoGrid,
		font_size_px: FONT_SIZE_PX,
		..AppOptions::default()
	};
	let form = Form {
		keys: HotkeyMap::new()
			.bind_str("Ctrl+S", Msg::Apply)?
			.bind_str("Ctrl+R", Msg::Reset)?,
		..Form::default()
	};
	App::new("gterm widget_edit", screen, rasterizer, config, options).run(form)?;
	Ok(())
}

/// Сообщения формы: публикуют привязки описания, разбирает `Form::update`.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Msg {
	Name(String),
	Path(String),
	Size(String),
	Kind(usize),
	Hidden(bool),
	ReadOnly(bool),
	Apply,
	Reset,
}

#[derive(Default)]
struct Form {
	backend: &'static str,
	name: TextInputState,
	path: TextInputState,
	size: TextInputState,
	kind: ComboBoxState,
	hidden: CheckboxState,
	read_only: CheckboxState,
	apply: ButtonState,
	reset: ButtonState,
	/// Комбинация, которую сейчас набирает пользователь; кормится из `on_key` через описание.
	tracker: HotkeyTracker,
	/// Карта «сочетание → сообщение» экрана (ADR-0029).
	keys: HotkeyMap<Msg>,
	/// Системный буфер обмена для команд правки в полях (ADR-0030).
	clipboard: SystemClipboard,
	/// Последнее сообщение — для строки статуса.
	last: String,
	/// Итог последнего «Применить».
	applied: String,
}

fn yes_no(value: bool) -> &'static str {
	if value { "да" } else { "нет" }
}

/// Строка формы: подпись слева, контрол справа.
fn row<'a>(label: &'a str, control: Ui<'a, Msg>) -> Ui<'a, Msg> {
	Ui::split(
		Layout::horizontal([Constraint::Length(10), Constraint::Fill(1)]),
		vec![
			Ui::widget(Paragraph::new(Line::styled(label, Style::new().fg(DIM)))),
			control,
		],
	)
}

fn field<'a>(placeholder: &'a str) -> TextInput<'a> {
	TextInput::new()
		.style(Style::new().bg(FIELD_BG))
		.focus_style(Style::new().bg(FOCUS_BG))
		.placeholder(placeholder)
		.placeholder_style(Style::new().fg(DIM))
}

fn checkbox<'a>(label: &'a str) -> Checkbox<'a> {
	Checkbox::new(label)
		.focus_style(Style::new().fg(ACCENT).bold())
		.pressed_style(Style::new().bg(FOCUS_BG))
}

fn button<'a>(label: &'a str) -> Button<'a> {
	Button::new(label)
		.style(Style::new().fg(BG).bg(DIM))
		.focus_style(Style::new().bg(ACCENT))
		.pressed_style(Style::new().bg(FOCUS_BG).fg(FG))
}

impl Form {
	fn ui(&mut self) -> Ui<'_, Msg> {
		let form = Ui::group(
			Block::bordered()
				.title(" Форма ")
				.border_type(BorderType::Rounded)
				.border_style(Style::new().fg(DIM)),
			Layout::vertical([
				Constraint::Length(1),
				Constraint::Length(1),
				Constraint::Length(1),
				Constraint::Length(1),
				Constraint::Length(3),
				Constraint::Length(1),
				Constraint::Fill(1),
			])
			.margin(1),
			vec![
				row(
					"Имя",
					Ui::stateful(field("как назвать"), &mut self.name)
						.on_change(Msg::Name)
						.on_submit(|_| Msg::Apply),
				),
				row(
					"Путь",
					Ui::stateful(field("каталог назначения"), &mut self.path)
						.on_change(Msg::Path)
						.on_submit(|_| Msg::Apply),
				),
				row(
					"Размер",
					Ui::stateful(field("байт"), &mut self.size)
						.on_change(Msg::Size)
						.on_submit(|_| Msg::Apply),
				),
				row(
					"Тип",
					Ui::combo_box(
						ComboBox::new(&KINDS)
							.placeholder("выберите")
							.style(Style::new().bg(FIELD_BG))
							.focus_style(Style::new().bg(FOCUS_BG))
							.highlight_style(Style::new().bg(FOCUS_BG).bold()),
						&mut self.kind,
						Msg::Kind,
					),
				),
				Ui::group(
					Block::bordered()
						.title(" Параметры ")
						.border_style(Style::new().fg(DIM)),
					Layout::horizontal([
						Constraint::Length(1),
						Constraint::Length(14),
						Constraint::Length(20),
						Constraint::Fill(1),
					]),
					vec![
						Ui::widget(Paragraph::new("")),
						Ui::stateful(checkbox("скрытый"), &mut self.hidden).on_toggle(Msg::Hidden),
						Ui::stateful(checkbox("только чтение"), &mut self.read_only)
							.on_toggle(Msg::ReadOnly),
					],
				)
				.focus_style(Style::new().fg(ACCENT)),
				Ui::split(
					Layout::horizontal([
						Constraint::Length(10),
						Constraint::Length(13),
						Constraint::Length(2),
						Constraint::Length(9),
						Constraint::Fill(1),
					]),
					vec![
						Ui::widget(Paragraph::new("")),
						Ui::stateful(button("Применить"), &mut self.apply).on_activate(Msg::Apply),
						Ui::widget(Paragraph::new("")),
						Ui::stateful(button("Сброс"), &mut self.reset).on_activate(Msg::Reset),
					],
				),
			],
		)
		.focus_style(Style::new().fg(ACCENT));

		let status = Paragraph::new(Line::from(vec![
			Span::styled(
				format!(" {} ", self.backend),
				Style::new().fg(BG).bg(ACCENT),
			),
			Span::styled(format!(" {}", self.last), Style::new().fg(DIM)),
			Span::styled(
				if self.applied.is_empty() {
					String::new()
				} else {
					format!("  │ применено: {}", self.applied)
				},
				Style::new().fg(FG),
			),
		]));
		let hint = Paragraph::new(Line::styled(
			"Tab фокус  Enter/Ctrl+S применить  Ctrl+R сброс  Ctrl+C/X/V буфер  Esc выход",
			Style::new().fg(DIM),
		));

		Ui::split(
			Layout::vertical([
				Constraint::Fill(1),
				Constraint::Length(14),
				Constraint::Fill(1),
				Constraint::Length(1),
				Constraint::Length(1),
			]),
			vec![
				Ui::widget(Paragraph::new("")),
				Ui::split(
					Layout::horizontal([
						Constraint::Fill(1),
						Constraint::Length(56),
						Constraint::Fill(1),
					]),
					vec![Ui::widget(Paragraph::new("")), form],
				),
				Ui::widget(Paragraph::new("")),
				Ui::widget(hint),
				Ui::widget(status),
			],
		)
		.hotkeys(&mut self.tracker, &self.keys)
		.clipboard(&mut self.clipboard)
	}

	fn update(&mut self, message: Msg) {
		match message {
			Msg::Name(value) => self.last = format!("имя: «{value}»"),
			Msg::Path(value) => self.last = format!("путь: «{value}»"),
			Msg::Size(value) => self.last = format!("размер: «{value}»"),
			Msg::Kind(index) => self.last = format!("тип: {}", KINDS[index]),
			Msg::Hidden(checked) => self.last = format!("скрытый: {}", yes_no(checked)),
			Msg::ReadOnly(checked) => self.last = format!("только чтение: {}", yes_no(checked)),
			Msg::Apply => {
				self.applied = format!(
					"имя={} путь={} размер={} тип={} скрытый={} чтение={}",
					self.name.value(),
					self.path.value(),
					self.size.value(),
					self.kind.selected().map_or("—", |i| KINDS[i]),
					yes_no(self.hidden.is_checked()),
					yes_no(self.read_only.is_checked()),
				);
				self.last = "применить".to_owned();
			}
			Msg::Reset => {
				self.name.clear();
				self.path.clear();
				self.size.clear();
				self.kind.select(None);
				self.hidden.set_checked(false);
				self.read_only.set_checked(false);
				self.applied.clear();
				self.last = "сброс".to_owned();
			}
		}
	}

	fn draw(&mut self, screen: &mut ScreenBuffer) {
		screen.clear_all();
		let area = Rect::of_screen(screen);
		self.ui().render(area, screen);
	}

	fn apply_messages(&mut self, messages: impl IntoIterator<Item = Msg>) {
		for message in messages {
			self.update(message);
		}
	}
}

impl AppHandler for Form {
	fn on_backend_changed(&mut self, name: &'static str, ctx: &mut Ctx<'_>) {
		self.backend = name;
		self.draw(ctx.screen);
		ctx.redraw();
	}

	fn on_grid_resized(&mut self, _old: Grid, ctx: &mut Ctx<'_>) {
		self.draw(ctx.screen);
		ctx.redraw();
	}

	fn on_key(&mut self, event: &KeyEvent, ctx: &mut Ctx<'_>) {
		// Описание получает и отпускания: ими кормится трекер карты сочетаний (ADR-0029).
		let routed = self.ui().key(event, ctx);
		if event.state != ElementState::Pressed {
			return;
		}
		let handled = routed.handled;
		self.apply_messages(routed);
		if !handled.is_yes() && matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
			ctx.exit();
		}
		self.draw(ctx.screen);
		ctx.redraw();
	}

	fn on_focus(&mut self, focused: bool, _ctx: &mut Ctx<'_>) {
		// Отпускания вне окна не придут — без сброса клавиша «зависла» бы в трекере.
		if !focused {
			self.tracker.reset();
		}
	}

	fn on_text(&mut self, text: &str, ctx: &mut Ctx<'_>) {
		let routed = self.ui().text(text);
		self.apply_messages(routed);
		self.draw(ctx.screen);
		ctx.redraw();
	}

	fn on_pointer(&mut self, event: &PointerEvent<'_>, ctx: &mut Ctx<'_>) {
		let routed = self.ui().pointer(event, ctx);
		let changed = routed.handled.is_yes() || !routed.messages.is_empty();
		self.apply_messages(routed);
		if changed {
			self.draw(ctx.screen);
			ctx.redraw();
		}
	}

	fn on_redraw(&mut self, ctx: &mut DrawCtx<'_>) {
		self.ui().draw_modal(ctx);
	}
}
