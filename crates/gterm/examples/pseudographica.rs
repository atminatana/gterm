//! Пример режимов псевдографики (ADR-0018): три таблицы — одиночные линии, двойные и смесь
//! (жирная рамка с одиночными перегородками), ниже — стыки одиночных с двойными, штриховые
//! линии, дуги, тени и блоки, образцы мозаик и фигур (ADR-0022); нижняя половина окна —
//! случайная псевдографика всех процедурных наборов (seed печатается в stderr). Шрифт — системный моноширинный 11 px, без `assets/`.
//!
//! Клавиши: `1`/`2`/`3` — OpenGL 3.3 / GLES 3.0 / Software; `q`/`w`/`e` — процедурная /
//! из шрифта / комбинированная псевдографика (в русской раскладке — те же клавиши `й`/`ц`/`у`);
//! `Esc` — выход. Запуск: `cargo run -p gterm --example pseudographica [-- --backend=…]`.

use std::error::Error;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use gterm::app::{App, AppHandler, AppOptions, Ctx, GridMode};
use gterm::buffer::ScreenBuffer;
use gterm::cell::{CellFlags, Color, Style};
use gterm::config::{BACKEND_ENV, Config};
use gterm::font::{FONT_SCALE, FontFamily, glyph_px_for};
use gterm::procedural;
use gterm::rasterizer::{BoxDrawingMode, Rasterizer};
use gterm::renderer::RendererKind;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, NamedKey};

/// Логический размер шрифта примера (решение владельца).
const FONT_SIZE_PX: u16 = 14;
const COLS: usize = 88;
const ROWS: usize = 26;
/// Первая строка случайного заполнения — после образцов наборов, до строки подсказки.
const FILL_FIRST_ROW: usize = 16;
const STATUS_ROW: usize = 0;
const TITLE_ROW: usize = 2;
const TABLE_ROW: usize = 3;
const EXTRAS_ROW: usize = 9;
const HINT_ROW: usize = ROWS - 1;
/// Левые столбцы трёх таблиц.
const TABLE_COLS: [usize; 3] = [3, 33, 63];
/// Ширина таблицы в ячейках и столбцы её вертикальных линий.
const TABLE_W: usize = 22;
const TABLE_LINES: [usize; 4] = [0, 7, 14, 21];

const FG: Color = Color::from_rgb(0xD8, 0xD8, 0xD8);
const BG: Color = Color::from_rgb(0x10, 0x14, 0x1C);
const LINE: Color = Color::from_rgb(0x80, 0xC0, 0xFF);
const TITLE: Color = Color::from_rgb(0xFF, 0xD0, 0x60);
const STATUS_FG: Color = Color::WHITE;
const STATUS_BG: Color = Color::from_rgb(0x00, 0x30, 0x60);
const HINT_FG: Color = Color::from_rgb(0x90, 0x90, 0x90);

/// Набор линий таблицы: внешняя рамка, внутренние перегородки и их стыки.
struct Lines {
	outer_h: char,
	inner_h: char,
	outer_v: char,
	inner_v: char,
	top_left: char,
	top_right: char,
	bottom_left: char,
	bottom_right: char,
	/// Стык внутренней вертикали с верхней, нижней, левой и правой сторонами рамки.
	top: char,
	bottom: char,
	left: char,
	right: char,
	cross: char,
}

const SINGLE: Lines = Lines {
	outer_h: '─',
	inner_h: '─',
	outer_v: '│',
	inner_v: '│',
	top_left: '┌',
	top_right: '┐',
	bottom_left: '└',
	bottom_right: '┘',
	top: '┬',
	bottom: '┴',
	left: '├',
	right: '┤',
	cross: '┼',
};

const DOUBLE: Lines = Lines {
	outer_h: '═',
	inner_h: '═',
	outer_v: '║',
	inner_v: '║',
	top_left: '╔',
	top_right: '╗',
	bottom_left: '╚',
	bottom_right: '╝',
	top: '╦',
	bottom: '╩',
	left: '╠',
	right: '╣',
	cross: '╬',
};

/// Жирная рамка с одиночными перегородками: стыки жирного с тонким (┯ ┷ ┠ ┨).
const MIXED: Lines = Lines {
	outer_h: '━',
	inner_h: '─',
	outer_v: '┃',
	inner_v: '│',
	top_left: '┏',
	top_right: '┓',
	bottom_left: '┗',
	bottom_right: '┛',
	top: '┯',
	bottom: '┷',
	left: '┠',
	right: '┨',
	cross: '┼',
};

const TABLES: [(&str, &Lines); 3] = [
	("одиночные линии", &SINGLE),
	("двойные линии", &DOUBLE),
	("смесь: жирная рамка, тонкие перегородки", &MIXED),
];

/// Содержимое ячеек таблицы: две строки по три поля.
const CELLS: [[&str; 3]; 2] = [["abc", "Яz 1", "x=12"], ["└┘ ok", "Ф Ю", "0.5"]];

fn main() -> ExitCode {
	let env_backend = std::env::var(BACKEND_ENV).ok();
	let config = match Config::parse(std::env::args().skip(1), env_backend.as_deref()) {
		Ok(config) if config.help => {
			println!("{}", Config::usage("pseudographica"));
			return ExitCode::SUCCESS;
		}
		Ok(config) => config,
		Err(e) => {
			eprintln!("pseudographica: {e}");
			return ExitCode::FAILURE;
		}
	};
	match run(config) {
		Ok(()) => ExitCode::SUCCESS,
		Err(e) => {
			eprintln!("pseudographica: {e}");
			ExitCode::FAILURE
		}
	}
}

fn run(config: Config) -> Result<(), Box<dyn Error>> {
	println!("pseudographica: searching for system monospace font (this may take a moment)...");
	let family = FontFamily::system_monospace()?;
	eprintln!(
		"pseudographica: font '{}' — {}",
		family.family(),
		family.face(Style::Regular).path.display()
	);
	// Сразу под размер примера при DPI 1.0, чтобы окно не перестраивало хинтеры.
	let glyph_px = glyph_px_for(FONT_SIZE_PX, FONT_SCALE, 1.0);
	let rasterizer = Rasterizer::new_at(family, glyph_px)?;
	let seed = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map_or(0x9E37_79B9_7F4A_7C15, |d| d.as_nanos() as u64)
		| 1;
	eprintln!("pseudographica: random fill seed {seed:#x}");
	let mut screen = ScreenBuffer::with_default_colors(COLS, ROWS, FG, BG);
	draw_scene(&mut screen);
	fill_random_procedural(&mut screen, seed);
	let options = AppOptions {
		grid_mode: GridMode::FixedGrid,
		font_size_px: FONT_SIZE_PX,
		..AppOptions::default()
	};
	App::new("gterm pseudographica", screen, rasterizer, config, options).run(Pseudographica {
		backend: "",
		mode: BoxDrawingMode::Procedural,
	})?;
	Ok(())
}

/// Заголовки, три таблицы, строка дополнений и подсказка; строку статуса пишет обработчик.
fn draw_scene(screen: &mut ScreenBuffer) {
	for (&col, (title, lines)) in TABLE_COLS.iter().zip(TABLES) {
		write_at(screen, col, TITLE_ROW, title, TITLE, CellFlags::BOLD);
		draw_table(screen, col, TABLE_ROW, lines);
	}
	write_at(
		screen,
		TABLE_COLS[0],
		EXTRAS_ROW,
		"╒═╤╕ ╞═╪╡ ╘═╧╛   ┄┄┄ ┅┅┅ ┆ ┇   ╭──╮ ╰──╯   ░░▒▒▓▓██ ▀▀▄▄ ▌▐ ▚▞",
		LINE,
		CellFlags::empty(),
	);
	write_at(
		screen,
		TABLE_COLS[0],
		EXTRAS_ROW + 1,
		"стыки одиночных с двойными · штриховые · дуги · тени и блоки",
		HINT_FG,
		CellFlags::empty(),
	);
	draw_set_samples(screen);
	screen.fill_row(HINT_ROW, HINT_FG, BG);
	screen.write_row(
		HINT_ROW,
		"1 GL  2 GLES  3 Software  ·  q процедурная  w шрифт  e комбинированная  ·  Esc выход",
		HINT_FG,
		BG,
	);
}

/// Нижняя часть окна (до строки подсказки) — случайные знаки всех процедурных наборов
/// (`procedural::ranges()`): стыки всех видов рядом друг с другом, на каждом запуске другие.
fn fill_random_procedural(screen: &mut ScreenBuffer, seed: u64) {
	let alphabet: Vec<char> = procedural::ranges()
		.flat_map(|(first, last)| first..=last)
		.collect();
	let mut state = seed;
	for row in FILL_FIRST_ROW..HINT_ROW {
		for col in 0..COLS {
			// xorshift64: достаточно для перемешивания, без зависимости `rand`.
			state ^= state << 13;
			state ^= state >> 7;
			state ^= state << 17;
			let ch = alphabet[(state % alphabet.len() as u64) as usize];
			screen.set_cell(col, row, ch, LINE, BG, CellFlags::empty());
		}
	}
}

/// Образцы наборов ADR-0022: секстанты, октанты, «Centered» и восьмушки, круги с композициями
/// 2×1, 1×2 и 2×2, треугольники, диагонали.
fn draw_set_samples(screen: &mut ScreenBuffer) {
	let run = |first: u32, count: u32| -> String {
		(first..first + count).filter_map(char::from_u32).collect()
	};
	let lines = [
		format!(
			"секстанты {}   октанты {}",
			run(0x1FB00, 20),
			run(0x1CD00, 20)
		),
		format!(
			"centered \u{25A0}{}   восьмушки {}\u{1CEA8}\u{1CEAB}\u{1CEA3}\u{1CEA0}",
			run(0x1FBE4, 4),
			run(0x1FB82, 10)
		),
		format!(
			"круги \u{25CF}\u{25D6}\u{25D7} \u{1FBE9}\u{1FBEB} \u{1FBEA} \u{1FBEE}\u{1FBED}   \
             треугольники \u{25B2}\u{25B6}\u{25BC}\u{25C0} \u{25C6} \u{25E2}\u{25E3}\u{25E4}\u{25E5} {}{}\u{1FB9A}\u{1FB9B}",
			run(0x1FB6C, 4),
			run(0x1FB68, 4)
		),
		format!(
			"{:13}\u{1FBE8} \u{1FBEC}\u{1FBEF}   диагонали {}",
			"",
			run(0x1FB3C, 44)
		),
	];
	for (i, line) in lines.iter().enumerate() {
		write_at(
			screen,
			TABLE_COLS[0],
			EXTRAS_ROW + 2 + i,
			line,
			LINE,
			CellFlags::empty(),
		);
	}
}

/// Таблица `TABLE_W × 5`: рамка, перегородка и две строки по три поля.
fn draw_table(screen: &mut ScreenBuffer, col: usize, row: usize, lines: &Lines) {
	for (dy, edge) in [(0, Edge::Top), (2, Edge::Middle), (4, Edge::Bottom)] {
		for dx in 0..TABLE_W {
			let ch = match (TABLE_LINES.contains(&dx), dx == 0, dx == TABLE_W - 1) {
				(true, true, _) => edge.left(lines),
				(true, _, true) => edge.right(lines),
				(true, _, _) => edge.inner(lines),
				(false, _, _) => edge.horizontal(lines),
			};
			screen.set_cell(col + dx, row + dy, ch, LINE, BG, CellFlags::empty());
		}
	}
	for (dy, cells) in [(1, CELLS[0]), (3, CELLS[1])] {
		for (i, &dx) in TABLE_LINES.iter().enumerate() {
			let is_outer = dx == 0 || dx == TABLE_W - 1;
			let ch = if is_outer {
				lines.outer_v
			} else {
				lines.inner_v
			};
			screen.set_cell(col + dx, row + dy, ch, LINE, BG, CellFlags::empty());
			if let Some(text) = cells.get(i) {
				write_at(screen, col + dx + 2, row + dy, text, FG, CellFlags::empty());
			}
		}
	}
}

/// Горизонтальная линия таблицы и её стыки с вертикальными.
#[derive(Clone, Copy)]
enum Edge {
	Top,
	Middle,
	Bottom,
}

impl Edge {
	fn horizontal(self, l: &Lines) -> char {
		match self {
			Edge::Middle => l.inner_h,
			Edge::Top | Edge::Bottom => l.outer_h,
		}
	}
	fn left(self, l: &Lines) -> char {
		match self {
			Edge::Top => l.top_left,
			Edge::Middle => l.left,
			Edge::Bottom => l.bottom_left,
		}
	}
	fn right(self, l: &Lines) -> char {
		match self {
			Edge::Top => l.top_right,
			Edge::Middle => l.right,
			Edge::Bottom => l.bottom_right,
		}
	}
	fn inner(self, l: &Lines) -> char {
		match self {
			Edge::Top => l.top,
			Edge::Middle => l.cross,
			Edge::Bottom => l.bottom,
		}
	}
}

fn write_at(
	screen: &mut ScreenBuffer,
	col: usize,
	row: usize,
	text: &str,
	fg: Color,
	flags: CellFlags,
) {
	for (i, ch) in text.chars().enumerate() {
		if col + i >= COLS {
			break;
		}
		screen.set_cell(col + i, row, ch, fg, BG, flags);
	}
}

fn mode_name(mode: BoxDrawingMode) -> &'static str {
	match mode {
		BoxDrawingMode::Procedural => "процедурная (q)",
		BoxDrawingMode::Font => "из шрифта (w)",
		BoxDrawingMode::Combined => "комбинированная (e)",
	}
}

/// Приложение: статус в строке 0, клавиши бэкенда и режима.
struct Pseudographica {
	backend: &'static str,
	mode: BoxDrawingMode,
}

impl Pseudographica {
	fn draw_status(&self, screen: &mut ScreenBuffer) {
		screen.fill_row(STATUS_ROW, STATUS_FG, STATUS_BG);
		screen.write_row(
			STATUS_ROW,
			&format!(
				" {}  ·  псевдографика: {}",
				self.backend,
				mode_name(self.mode)
			),
			STATUS_FG,
			STATUS_BG,
		);
	}

	fn set_mode(&mut self, mode: BoxDrawingMode, ctx: &mut Ctx<'_>) {
		self.mode = mode;
		ctx.request.box_drawing = Some(mode);
		self.draw_status(ctx.screen);
		ctx.redraw();
	}
}

impl AppHandler for Pseudographica {
	fn on_backend_changed(&mut self, name: &'static str, ctx: &mut Ctx<'_>) {
		self.backend = name;
		self.draw_status(ctx.screen);
		ctx.redraw();
	}

	fn on_key(&mut self, event: &KeyEvent, ctx: &mut Ctx<'_>) {
		if event.state != ElementState::Pressed {
			return;
		}
		match &event.logical_key {
			Key::Named(NamedKey::Escape) => ctx.exit(),
			// `Key::Character` в winit 0.30 несёт строку по текущей раскладке.
			Key::Character(c) => match &**c {
				"1" => ctx.request.backend = Some(RendererKind::OpenGl33),
				"2" => ctx.request.backend = Some(RendererKind::Gles30),
				"3" => ctx.request.backend = Some(RendererKind::Software),
				"q" | "й" => self.set_mode(BoxDrawingMode::Procedural, ctx),
				"w" | "ц" => self.set_mode(BoxDrawingMode::Font, ctx),
				"e" | "у" => self.set_mode(BoxDrawingMode::Combined, ctx),
				_ => {}
			},
			_ => {}
		}
	}
}
