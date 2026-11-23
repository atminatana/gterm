//! Форма из контролов `gterm-widgets` (ADR-0026, ADR-0027, ADR-0029) на системном моноширинном шрифте:
//! три поля ввода, выпадающий список, ползунок «Уровень» со значением справа (ADR-0036; `←`/`→`,
//! `Home`/`End`, клик, протяжка, колесо), группа «Параметры» с двумя флажками (ADR-0031; пробел,
//! Enter или клик — переключить), кнопки «Применить» и «Сброс», строка статуса. Экран
//! описан один раз — `Form::ui()`; текст приходит через `on_text` и идёт полю под фокусом,
//! `Tab`/`Shift+Tab` ходят по кольцу поля → список → кнопки, `Enter` в поле — «Применить»,
//! список раскрывается `Enter`/пробелом/`↓`/кликом слоем под полем и закрывается выбором, `Esc`
//! или кликом мимо; `Esc` на форме — выход. Сочетания из карты экрана (ADR-0029) не зависят от
//! раскладки и работают при фокусе в поле: `Ctrl+S` — «Применить», `Ctrl+R` — «Сброс».
//! В полях — выделение (`Shift` со стрелками, мышь, `Ctrl+A`) и системный буфер обмена
//! (`Ctrl+C`/`Ctrl+X`/`Ctrl+V`, ADR-0030).
//!
//! Цвета — из темы (ADR-0033): она подключена к описанию одной строкой `Ui::theme`, и каждый
//! виджет берёт из неё свои области; свои стили пример задаёт только тексту подписей и значку
//! бэкенда — тоже из темы, по имени области. Без ключей — встроенная тема; `--theme=<имя>` —
//! тема из `examples/themes/` (`gterm_dark`, `gterm_light`) или из каталога `--theme-dir=<путь>`
//! (например, `runtime/themes` редактора helix); `F2` — следующая тема по кругу.
//! Запуск: `cargo run -p gterm-widgets --example widget_edit -- --theme=gterm_light`.

use std::error::Error;
use std::path::PathBuf;
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
	Paragraph, Slider, SliderState, TextInput, TextInputState,
};
use gterm_widgets::{
	Constraint, HotkeyMap, Layout, Line, Rect, Span, Style, SystemClipboard, Theme, ThemeError, Ui,
};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, NamedKey};

const FONT_SIZE_PX: u16 = 14;
const COLS: usize = 80;
const ROWS: usize = 24;

const THEME_DIR_ARG: &str = "--theme-dir=";
const THEME_ARG: &str = "--theme=";

/// Область приложения в теме: значок бэкенда в строке статуса. Тема без неё (любая тема
/// helix) даёт значку вид строки состояния или выделенного пункта меню.
const BADGE_SCOPES: [&str; 3] = [
	"widget_edit.badge",
	"ui.statusline.normal",
	"ui.menu.selected",
];

const KINDS: [&str; 4] = ["файл", "каталог", "ссылка", "устройство"];
/// Уровень по умолчанию и после «Сброс»; шаг клавиш и колеса.
const LEVEL_DEFAULT: i64 = 50;
const LEVEL_STEP: i64 = 5;

fn main() -> ExitCode {
	let env_backend = std::env::var(BACKEND_ENV).ok();
	let (theme_args, args) = ThemeArgs::split(std::env::args().skip(1));
	let config = match Config::parse(args, env_backend.as_deref()) {
		Ok(config) if config.help => {
			println!("{}", Config::usage("widget_edit"));
			println!(
				"       [{THEME_DIR_ARG}DIR] [{THEME_ARG}NAME]\n\
				 \x20 {THEME_DIR_ARG}DIR  directory with helix-format themes (*.toml)\n\
				 \x20 {THEME_ARG}NAME     theme to start with (default: built-in); F2 — next theme"
			);
			return ExitCode::SUCCESS;
		}
		Ok(config) => config,
		Err(e) => {
			eprintln!("widget_edit: {e}");
			return ExitCode::FAILURE;
		}
	};
	match run(config, theme_args) {
		Ok(()) => ExitCode::SUCCESS,
		Err(e) => {
			eprintln!("widget_edit: {e}");
			ExitCode::FAILURE
		}
	}
}

/// Ключи темы; остальные аргументы разбирает `Config`.
#[derive(Default)]
struct ThemeArgs {
	dir: Option<PathBuf>,
	name: Option<String>,
}

impl ThemeArgs {
	fn split(args: impl Iterator<Item = String>) -> (Self, Vec<String>) {
		let mut theme_args = Self::default();
		let mut rest = Vec::new();
		for arg in args {
			if let Some(dir) = arg.strip_prefix(THEME_DIR_ARG) {
				theme_args.dir = Some(PathBuf::from(dir));
			} else if let Some(name) = arg.strip_prefix(THEME_ARG) {
				theme_args.name = Some(name.to_owned());
			} else {
				rest.push(arg);
			}
		}
		(theme_args, rest)
	}
}

/// Темы примера: встроенная и файлы каталогов — заданного ключом и `examples/themes/`.
#[derive(Default)]
struct Themes {
	dirs: Vec<PathBuf>,
	/// Имена тем из каталогов, по алфавиту.
	names: Vec<String>,
	/// Номер текущей темы в `names`; `None` — встроенная.
	current: Option<usize>,
	theme: Theme,
}

impl Themes {
	fn new(args: ThemeArgs) -> Result<Self, ThemeError> {
		let own_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/themes");
		let dirs: Vec<PathBuf> = args.dir.into_iter().chain([own_dir]).collect();
		let mut names: Vec<String> = dirs
			.iter()
			.filter_map(|dir| std::fs::read_dir(dir).ok())
			.flatten()
			.filter_map(|entry| entry.ok().map(|entry| entry.path()))
			.filter(|path| {
				path.extension()
					.is_some_and(|extension| extension == "toml")
			})
			.filter_map(|path| path.file_stem()?.to_str().map(str::to_owned))
			.collect();
		names.sort();
		names.dedup();
		let mut themes = Self {
			dirs,
			names,
			..Self::default()
		};
		if let Some(name) = args.name {
			themes.theme = Theme::load(&themes.dirs, &name)?;
			themes.current = themes.names.iter().position(|known| *known == name);
		}
		Ok(themes)
	}

	/// Следующая тема по кругу: встроенная, затем файлы. Возвращает строку для статуса; тема,
	/// которая не загрузилась, пропускается с причиной, прежняя остаётся.
	fn next(&mut self) -> String {
		self.current = match self.current {
			None if self.names.is_empty() => None,
			None => Some(0),
			Some(index) if index + 1 < self.names.len() => Some(index + 1),
			Some(_) => None,
		};
		let Some(index) = self.current else {
			self.theme = Theme::default();
			return "тема: встроенная".to_owned();
		};
		match Theme::load(&self.dirs, &self.names[index]) {
			Ok(theme) => {
				self.theme = theme;
				format!("тема: {}", self.names[index])
			}
			Err(error) => format!("тема не загружена: {error}"),
		}
	}
}

fn run(config: Config, theme_args: ThemeArgs) -> Result<(), Box<dyn Error>> {
	let themes = Themes::new(theme_args)?;
	for warning in themes.theme.warnings() {
		eprintln!("widget_edit: theme '{}': {warning}", themes.theme.name());
	}
	println!("widget_edit: searching for system monospace font (this may take a moment)...");
	let family = FontFamily::system_monospace()?;
	eprintln!(
		"widget_edit: font '{}' — {}",
		family.family(),
		family.face(FontStyle::Regular).path.display()
	);
	let glyph_px = glyph_px_for(FONT_SIZE_PX, FONT_SCALE, 1.0);
	let rasterizer = Rasterizer::new_at(family, glyph_px)?;
	let base = themes.theme.base();
	let screen = ScreenBuffer::with_default_colors(
		COLS,
		ROWS,
		base.fg.unwrap_or(Color::WHITE),
		base.bg.unwrap_or(Color::BLACK),
	);
	let options = AppOptions {
		grid_mode: GridMode::AutoGrid,
		font_size_px: FONT_SIZE_PX,
		..AppOptions::default()
	};
	let form = Form {
		keys: HotkeyMap::new()
			.bind_str("Ctrl+S", Msg::Apply)?
			.bind_str("Ctrl+R", Msg::Reset)?
			.bind_str("F2", Msg::NextTheme)?,
		themes,
		level: SliderState::new(0, 100)
			.with_value(LEVEL_DEFAULT)
			.with_step(LEVEL_STEP),
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
	Level(i64),
	Hidden(bool),
	ReadOnly(bool),
	Apply,
	Reset,
	NextTheme,
}

#[derive(Default)]
struct Form {
	backend: &'static str,
	name: TextInputState,
	path: TextInputState,
	size: TextInputState,
	kind: ComboBoxState,
	level: SliderState,
	hidden: CheckboxState,
	read_only: CheckboxState,
	apply: ButtonState,
	reset: ButtonState,
	/// Комбинация, которую сейчас набирает пользователь; кормится из `on_key` через описание.
	tracker: HotkeyTracker,
	/// Карта «сочетание → сообщение» экрана (ADR-0029).
	keys: HotkeyMap<Msg>,
	/// Текущая тема и перечень тем для смены на лету (ADR-0033).
	themes: Themes,
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

/// Только цвет символа стиля.
fn foreground(style: Style) -> Style {
	Style {
		fg: style.fg,
		..Style::new()
	}
}

/// Строка формы: подпись слева (стилем `label_style`), контрол справа.
fn row<'a>(label: &'a str, label_style: Style, control: Ui<'a, Msg>) -> Ui<'a, Msg> {
	Ui::split(
		Layout::horizontal([Constraint::Length(10), Constraint::Fill(1)]),
		vec![
			Ui::widget(Paragraph::new(Line::styled(label, label_style))),
			control,
		],
	)
}

fn field<'a>(placeholder: &'a str) -> TextInput<'a> {
	TextInput::new().placeholder(placeholder)
}

impl Form {
	fn ui(&mut self) -> Ui<'_, Msg> {
		let theme = &self.themes.theme;
		// Стили самого приложения — тоже из темы, по имени области: приглушённый текст
		// подписей и своя область значка бэкенда.
		let dim = theme.style("ui.text.inactive");
		let badge = theme.first(&BADGE_SCOPES);
		// Значение — до того, как описание возьмёт состояние ползунка по `&mut`.
		let level = format!("{:>4} %", self.level.value());
		// Своих областей темы у ползунка нет (ADR-0036 старше тем): приложение красит его тонами
		// темы само — заполненная часть цветом акцента, остальная цветом рамки, фокус как у поля.
		let slider = Slider::new()
			.filled_style(foreground(theme.style("ui.text.focus")))
			.unfilled_style(foreground(theme.style("ui.window")))
			.focus_style(theme.style("ui.input.focus"));
		let form = Ui::group(
			Block::bordered()
				.title(" Форма ")
				.border_type(BorderType::Rounded),
			Layout::vertical([
				Constraint::Length(1),
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
					dim,
					Ui::stateful(field("как назвать"), &mut self.name)
						.on_change(Msg::Name)
						.on_submit(|_| Msg::Apply),
				),
				row(
					"Путь",
					dim,
					Ui::stateful(field("каталог назначения"), &mut self.path)
						.on_change(Msg::Path)
						.on_submit(|_| Msg::Apply),
				),
				row(
					"Размер",
					dim,
					Ui::stateful(field("байт"), &mut self.size)
						.on_change(Msg::Size)
						.on_submit(|_| Msg::Apply),
				),
				row(
					"Тип",
					dim,
					Ui::combo_box(
						ComboBox::new(&KINDS).placeholder("выберите"),
						&mut self.kind,
						Msg::Kind,
					),
				),
				row(
					"Уровень",
					dim,
					Ui::split(
						Layout::horizontal([Constraint::Fill(1), Constraint::Length(7)]),
						vec![
							Ui::stateful(slider, &mut self.level).on_value(Msg::Level),
							Ui::widget(Paragraph::new(level)),
						],
					),
				),
				Ui::group(
					Block::bordered().title(" Параметры "),
					Layout::horizontal([
						Constraint::Length(1),
						Constraint::Length(14),
						Constraint::Length(20),
						Constraint::Fill(1),
					]),
					vec![
						Ui::widget(Paragraph::new("")),
						Ui::stateful(Checkbox::new("скрытый"), &mut self.hidden)
							.on_toggle(Msg::Hidden),
						Ui::stateful(Checkbox::new("только чтение"), &mut self.read_only)
							.on_toggle(Msg::ReadOnly),
					],
				),
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
						Ui::stateful(Button::new("Применить"), &mut self.apply)
							.on_activate(Msg::Apply),
						Ui::widget(Paragraph::new("")),
						Ui::stateful(Button::new("Сброс"), &mut self.reset).on_activate(Msg::Reset),
					],
				),
			],
		);

		let status = Paragraph::new(Line::from(vec![
			Span::styled(format!(" {} ", self.backend), badge),
			Span::styled(format!(" {}", self.last), dim),
			Span::raw(if self.applied.is_empty() {
				String::new()
			} else {
				format!("  │ применено: {}", self.applied)
			}),
		]));
		let hint = Paragraph::new(Line::styled(
			"Tab фокус  Enter применить  Ctrl+R сброс  Ctrl+C/X/V буфер  F2 тема  Esc выход",
			dim,
		));

		Ui::split(
			Layout::vertical([
				Constraint::Fill(1),
				Constraint::Length(15),
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
		.theme(theme)
	}

	fn update(&mut self, message: Msg) {
		match message {
			Msg::Name(value) => self.last = format!("имя: «{value}»"),
			Msg::Path(value) => self.last = format!("путь: «{value}»"),
			Msg::Size(value) => self.last = format!("размер: «{value}»"),
			Msg::Kind(index) => self.last = format!("тип: {}", KINDS[index]),
			Msg::Level(value) => self.last = format!("уровень: {value} %"),
			Msg::Hidden(checked) => self.last = format!("скрытый: {}", yes_no(checked)),
			Msg::ReadOnly(checked) => self.last = format!("только чтение: {}", yes_no(checked)),
			Msg::Apply => {
				self.applied = format!(
					"имя={} путь={} размер={} тип={} уровень={} скрытый={} чтение={}",
					self.name.value(),
					self.path.value(),
					self.size.value(),
					self.kind.selected().map_or("—", |i| KINDS[i]),
					self.level.value(),
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
				// Программная установка сообщения `Level` не даёт (ADR-0036).
				self.level.set_value(LEVEL_DEFAULT);
				self.hidden.set_checked(false);
				self.read_only.set_checked(false);
				self.applied.clear();
				self.last = "сброс".to_owned();
			}
			Msg::NextTheme => self.last = self.themes.next(),
		}
	}

	fn draw(&mut self, screen: &mut ScreenBuffer) {
		// Поле вокруг сетки — тем же фоном, что и экран.
		if let Some(background) = self.themes.theme.base().bg {
			screen.set_background(background);
		}
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
