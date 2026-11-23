//! Просмотр тем оформления (ADR-0037) на системном моноширинном шрифте: слева перечень тем
//! каталогов, посередине — элементы выбранной темы (области «имя → стиль»), каждая строка
//! оформлена стилем, который описывает; знаком `≈` помечены области, которых в файле темы нет —
//! они выведены из её основы (ADR-0038), справа — образец элементов интерфейса, раскрашенный
//! этой темой; кнопки образца открывают диалоговые окна.
//!
//! Каталоги тем — в поле сверху: пути через `;`, `Enter` перечитывает перечень
//! (`Theme::list`); путь можно вставить из системного буфера обмена — в полях работают
//! выделение, `Ctrl+A` и `Ctrl+C`/`Ctrl+X`/`Ctrl+V` (ADR-0030). Начальные каталоги — ключи `--theme-dir=<путь>` (ключ можно повторять;
//! например, `runtime/themes` редактора helix), без ключа — `examples/themes/` крейта. Первой в
//! перечне стоит встроенная тема; `--theme=<имя>` — тема, с которой начать.
//!
//! Тема меняется выделением в перечне (стрелки, клик) и сочетаниями `F2`/`F3` — следующая и
//! предыдущая по кругу; сочетания действуют и при открытом окне. `F4` — диалог, `F5` — окно с
//! тем, что тема при загрузке отобразить не смогла. `F6` — сохранить тему на экране полной
//! записью (`Theme::to_toml_string`): все области, включая выведенные, в файл, который можно
//! править или взять за основу; путь предлагается в окне (`<имя>_full.toml` в первом каталоге),
//! существующий файл не заменяется. `Tab`/`Shift+Tab` — фокус, `Esc` — закрыть
//! окно либо выйти.
//! Запуск: `cargo run -p gterm-widgets --example theme -- --theme-dir=<каталог>`.

mod catalog;
mod dialogs;
mod sample;
mod scopes;

use std::error::Error;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use gterm::app::{App, AppHandler, AppOptions, Ctx, DrawCtx, Grid, GridMode, PointerEvent};
use gterm::buffer::ScreenBuffer;
use gterm::cell::{Color, Style as FontStyle};
use gterm::config::{BACKEND_ENV, Config};
use gterm::font::{FONT_SCALE, FontFamily, glyph_px_for};
use gterm::hotkey::HotkeyTracker;
use gterm::rasterizer::Rasterizer;
use gterm_widgets::widgets::{
	Block, List, ListState, Paragraph, Scrollbar, ScrollbarOrientation, TextInput, TextInputState,
};
use gterm_widgets::{
	Constraint, HotkeyMap, Layout, Line, Rect, Span, Style, SystemClipboard, ThemeError, Ui,
};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, NamedKey};

use catalog::Catalog;
use dialogs::{Dialogs, Kind};
use sample::Sample;

const FONT_SIZE_PX: u16 = 14;
const COLS: usize = 132;
const ROWS: usize = 34;

const THEME_DIR_ARG: &str = "--theme-dir=";
const THEME_ARG: &str = "--theme=";

/// Цвета ячеек и поля вокруг сетки для темы, которая не задаёт `ui.text` или `ui.background`.
const FALLBACK_FG: Color = Color::from_rgb(0xD8, 0xD8, 0xD8);
const FALLBACK_BG: Color = Color::BLACK;

/// Значок бэкенда в строке статуса: вид строки состояния helix, иначе выделенного пункта меню.
const BADGE_SCOPES: [&str; 2] = ["ui.statusline.normal", "ui.menu.selected"];

const DIRS_LABEL_WIDTH: u16 = 15;
const THEMES_WIDTH: u16 = 24;
const SAMPLE_WIDTH: u16 = 44;

const HINT: &str = " Tab фокус  Enter — перечитать каталоги  Ctrl+C/X/V буфер  F2/F3 тема  \
F4 диалог  F5 предупреждения  F6 сохранить  Esc выход";

fn main() -> ExitCode {
	let env_backend = std::env::var(BACKEND_ENV).ok();
	let (theme_args, args) = ThemeArgs::split(std::env::args().skip(1));
	let config = match Config::parse(args, env_backend.as_deref()) {
		Ok(config) if config.help => {
			println!("{}", Config::usage("theme"));
			println!(
				"       [{THEME_DIR_ARG}DIR]... [{THEME_ARG}NAME]\n\
				 \x20 {THEME_DIR_ARG}DIR  directory with helix-format themes (*.toml); repeatable\n\
				 \x20 {THEME_ARG}NAME     theme to start with (default: built-in)"
			);
			return ExitCode::SUCCESS;
		}
		Ok(config) => config,
		Err(e) => {
			eprintln!("theme: {e}");
			return ExitCode::FAILURE;
		}
	};
	match run(config, theme_args) {
		Ok(()) => ExitCode::SUCCESS,
		Err(e) => {
			eprintln!("theme: {e}");
			ExitCode::FAILURE
		}
	}
}

/// Ключи темы; остальные аргументы разбирает `Config`.
#[derive(Default)]
struct ThemeArgs {
	dirs: Vec<PathBuf>,
	name: Option<String>,
}

impl ThemeArgs {
	fn split(args: impl Iterator<Item = String>) -> (Self, Vec<String>) {
		let mut theme_args = Self::default();
		let mut rest = Vec::new();
		for arg in args {
			if let Some(dirs) = arg.strip_prefix(THEME_DIR_ARG) {
				theme_args.dirs.extend(catalog::parse_dirs(dirs));
			} else if let Some(name) = arg.strip_prefix(THEME_ARG) {
				theme_args.name = Some(name.to_owned());
			} else {
				rest.push(arg);
			}
		}
		(theme_args, rest)
	}
}

fn run(config: Config, theme_args: ThemeArgs) -> Result<(), Box<dyn Error>> {
	let dirs = if theme_args.dirs.is_empty() {
		vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/themes")]
	} else {
		theme_args.dirs
	};
	let dirs_text = catalog::join_dirs(&dirs);
	let mut catalog = Catalog::new();
	catalog.scan(dirs)?;
	if let Some(name) = theme_args.name {
		let index = catalog
			.position(&name)
			.ok_or(ThemeError::NotFound { name })?;
		catalog.select(index)?;
	}

	println!("theme: searching for system monospace font (this may take a moment)...");
	let family = FontFamily::system_monospace()?;
	eprintln!(
		"theme: font '{}' — {}",
		family.family(),
		family.face(FontStyle::Regular).path.display()
	);
	let glyph_px = glyph_px_for(FONT_SIZE_PX, FONT_SCALE, 1.0);
	let rasterizer = Rasterizer::new_at(family, glyph_px)?;
	let screen = ScreenBuffer::with_default_colors(COLS, ROWS, FALLBACK_FG, FALLBACK_BG);
	let options = AppOptions {
		grid_mode: GridMode::AutoGrid,
		font_size_px: FONT_SIZE_PX,
		..AppOptions::default()
	};
	let theme_keys = HotkeyMap::new()
		.bind_str("F2", Msg::NextTheme)?
		.bind_str("F3", Msg::PreviousTheme)?;
	let mut viewer = Viewer {
		backend: "",
		dirs: TextInputState::with_value(dirs_text),
		themes: ListState::with_selected(Some(catalog.current())),
		scopes: ListState::with_selected(Some(0)),
		catalog,
		sample: Sample::new(),
		dialogs: Dialogs::default(),
		tracker: HotkeyTracker::default(),
		keys: theme_keys
			.clone()
			.bind_str("F4", Msg::OpenDialog)?
			.bind_str("F5", Msg::OpenWarnings)?
			.bind_str("F6", Msg::OpenSave)?,
		dialog_keys: theme_keys,
		clipboard: SystemClipboard::new(),
		status: String::new(),
	};
	// Первый фокус — перечню тем, а не полю каталогов: стрелки сразу меняют тему.
	viewer.themes.node.focused = true;
	viewer.status = format!("{} · {}", viewer.catalog_summary(), viewer.theme_summary());
	App::new("gterm theme", screen, rasterizer, config, options).run(viewer)?;
	Ok(())
}

/// Сообщения приложения: публикуют привязки описания, разбирает `Viewer::update`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
	/// `Enter` в поле каталогов: новая строка каталогов.
	Dirs(String),
	/// Выделение в перечне тем.
	Theme(usize),
	NextTheme,
	PreviousTheme,
	OpenDialog,
	OpenWarnings,
	/// Открыть окно сохранения темы.
	OpenSave,
	/// Записать тему в файл, названный в окне сохранения.
	SaveTheme,
	CloseDialog,
	/// Событие контрола образца: состояние уже в контроле, приложению делать нечего.
	Sample,
}

struct Viewer {
	backend: &'static str,
	catalog: Catalog,
	/// Поле каталогов тем.
	dirs: TextInputState,
	/// Перечень тем; выделение — тема на экране (или та, что не загрузилась).
	themes: ListState,
	/// Элементы темы на экране.
	scopes: ListState,
	sample: Sample,
	dialogs: Dialogs,
	/// Комбинация, которую сейчас набирает пользователь; кормится из `on_key` через описание.
	tracker: HotkeyTracker,
	/// Карта сочетаний экрана (ADR-0029).
	keys: HotkeyMap<Msg>,
	/// Карта сочетаний диалогового окна: пока оно открыто, действует только она.
	dialog_keys: HotkeyMap<Msg>,
	/// Системный буфер обмена для команд правки в полях (ADR-0030): путь в поле каталогов
	/// можно вставить.
	clipboard: SystemClipboard,
	status: String,
}

/// Строка формы: подпись шириной `width` слева (стилем `style`), контрол справа.
pub fn labeled<'a>(label: &'a str, width: u16, style: Style, control: Ui<'a, Msg>) -> Ui<'a, Msg> {
	Ui::split(
		Layout::horizontal([Constraint::Length(width), Constraint::Fill(1)]),
		vec![
			Ui::widget(Paragraph::new(Line::styled(label, style))),
			control,
		],
	)
}

/// Пустое место в раскладке.
pub fn gap<'a>() -> Ui<'a, Msg> {
	Ui::widget(Paragraph::new(""))
}

/// Записать новый файл; существующий не заменяется — ошибка `AlreadyExists`.
fn write_new(path: &Path, text: &str) -> std::io::Result<()> {
	let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
	file.write_all(text.as_bytes())
}

fn scrollbar() -> Scrollbar {
	Scrollbar::new(ScrollbarOrientation::Vertical)
}

impl Viewer {
	fn ui(&mut self) -> Ui<'_, Msg> {
		let theme = self.catalog.theme();
		let shown_title = self.catalog.title(self.catalog.current());
		// Стили самого приложения — из темы, по имени области.
		let dim = theme.style("ui.text.inactive");
		let badge = theme.first(&BADGE_SCOPES);
		let ratio = (self.catalog.current() + 1) as f64 / self.catalog.len() as f64;

		let dirs = labeled(
			" Каталоги тем",
			DIRS_LABEL_WIDTH,
			dim,
			Ui::stateful(
				TextInput::new().placeholder("пути через «;», Enter — перечитать перечень"),
				&mut self.dirs,
			)
			.on_submit(Msg::Dirs),
		);
		let themes = List::new(self.catalog.titles())
			.block(Block::bordered().title(format!(" Темы · {} ", self.catalog.len())))
			.highlight_symbol("▶ ")
			.scrollbar(scrollbar());
		let scope_items = scopes::items(theme);
		let scopes = List::new(scope_items)
			.block(Block::bordered().title(format!(
				" Элементы темы {shown_title} · {} ",
				theme.scopes().count()
			)))
			.highlight_symbol("▶ ")
			.scrollbar(scrollbar());
		let status = Paragraph::new(Line::from(vec![
			Span::styled(format!(" {} ", self.backend), badge),
			Span::raw(format!(" {}", self.status)),
		]));

		let screen = Ui::split(
			Layout::vertical([
				Constraint::Length(1),
				Constraint::Fill(1),
				Constraint::Length(1),
				Constraint::Length(1),
			]),
			vec![
				dirs,
				Ui::split(
					Layout::horizontal([
						Constraint::Length(THEMES_WIDTH),
						Constraint::Fill(1),
						Constraint::Length(SAMPLE_WIDTH),
					]),
					vec![
						Ui::stateful(themes, &mut self.themes).on_select(Msg::Theme),
						Ui::stateful(scopes, &mut self.scopes),
						self.sample.ui(theme, ratio),
					],
				),
				Ui::widget(Paragraph::new(Line::styled(HINT, dim))),
				Ui::widget(status),
			],
		)
		.hotkeys(&mut self.tracker, &self.keys)
		.clipboard(&mut self.clipboard)
		.theme(theme);
		match self.dialogs.modal(theme, shown_title, &self.dialog_keys) {
			Some(modal) => screen.modal(modal),
			None => screen,
		}
	}

	fn update(&mut self, message: Msg) {
		match message {
			Msg::Dirs(text) => self.rescan(&text),
			Msg::Theme(index) => self.status = self.show_theme(index),
			Msg::NextTheme => self.step_theme(true),
			Msg::PreviousTheme => self.step_theme(false),
			Msg::OpenDialog => self.dialogs.open(Kind::Form),
			Msg::OpenWarnings => self.dialogs.open(Kind::Warnings),
			Msg::OpenSave => {
				let path = self.catalog.save_path();
				self.dialogs.open_save(path.display().to_string());
			}
			Msg::SaveTheme => self.save_theme(),
			Msg::CloseDialog => self.dialogs.close(),
			Msg::Sample => {}
		}
	}

	/// Перечитать перечень тем по строке каталогов. Тема на экране перезагружается из новых
	/// каталогов; если её в них нет — показывается встроенная.
	fn rescan(&mut self, text: &str) {
		let scanned = self.catalog.scan(catalog::parse_dirs(text));
		let index = self.catalog.current();
		self.themes.select(Some(index));
		let theme_status = self.show_theme(index);
		self.status = match scanned {
			Ok(()) => format!("{} · {theme_status}", self.catalog_summary()),
			Err(error) => format!("перечень тем не получен: {error}"),
		};
	}

	/// Записать тему на экране в файл из окна сохранения. Удача закрывает окно и перечитывает
	/// перечень — файл, записанный в каталог тем, появляется в нём; при ошибке окно остаётся
	/// открытым, причина — в статусе.
	fn save_theme(&mut self) {
		let path = PathBuf::from(self.dialogs.save_path().trim());
		let text = self.catalog.theme().to_toml_string();
		match write_new(&path, &text) {
			Ok(()) => {
				self.dialogs.close();
				let dirs = catalog::join_dirs(self.catalog.dirs());
				self.rescan(&dirs);
				self.status = format!("тема сохранена: {}", path.display());
			}
			Err(error) => self.status = format!("тема не сохранена: {}: {error}", path.display()),
		}
	}

	/// Загрузить и показать тему перечня; возвращает строку для статуса. Тема, которая не
	/// загрузилась, оставляет на экране прежнюю.
	fn show_theme(&mut self, index: usize) -> String {
		if let Err(error) = self.catalog.select(index) {
			return format!("тема не загружена: {error}");
		}
		// Элементы у новой темы свои: список — к началу.
		self.scopes.select(Some(0));
		self.scopes.scroll_to(0);
		self.theme_summary()
	}

	/// Соседняя тема перечня по кругу — от выделенной, а не от показанной: тема, которая не
	/// загрузилась, выделена, и шаг идёт дальше неё.
	fn step_theme(&mut self, is_forward: bool) {
		let count = self.catalog.len();
		let selected = self.themes.selected().unwrap_or(0).min(count - 1);
		let step = if is_forward { 1 } else { count - 1 };
		let next = (selected + step) % count;
		self.themes.select(Some(next));
		self.status = self.show_theme(next);
	}

	fn catalog_summary(&self) -> String {
		format!(
			"каталогов {}, тем в них {}",
			self.catalog.dir_count(),
			self.catalog.len() - 1
		)
	}

	fn theme_summary(&self) -> String {
		let theme = self.catalog.theme();
		format!(
			"тема {}: элементов {} (≈ выведено {}), предупреждений {}",
			self.catalog.title(self.catalog.current()),
			theme.scopes().count(),
			theme
				.scopes()
				.filter(|(name, _)| theme.is_derived(name))
				.count(),
			theme.warnings().len()
		)
	}

	fn draw(&mut self, screen: &mut ScreenBuffer) {
		// Поле вокруг сетки — тем же фоном, что и экран.
		let background = self.catalog.theme().base().bg.unwrap_or(FALLBACK_BG);
		screen.set_background(background);
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

impl AppHandler for Viewer {
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

	fn on_frame(&mut self, ctx: &mut Ctx<'_>) {
		// Кнопка удержана на стрелке или дорожке полосы прокрутки: список листает по тактам
		// времени (ADR-0032) и просит следующий кадр; без удержания такт ничего не делает.
		let routed = self.ui().tick(Instant::now());
		if routed.handled.is_yes() {
			self.apply_messages(routed);
			self.draw(ctx.screen);
			ctx.redraw();
		}
	}

	fn on_redraw(&mut self, ctx: &mut DrawCtx<'_>) {
		self.ui().draw_modal(ctx);
	}
}
