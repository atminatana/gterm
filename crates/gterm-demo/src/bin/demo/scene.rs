//! Статическая сцена демо (FR-27): текст с флагами и начертаниями, псевдографика, фрагмент
//! карты из SVG-тайлов, палитра, фрагмент таблицы глифов, подсказка по клавишам. Строки 0 (статус) и 1 (мышь) и приглашение
//! с курсором рисует само приложение в `main.rs`.

use gterm::buffer::ScreenBuffer;
use gterm::cell::{CellFlags, Color};

use crate::common::HELP_FG;

/// Стартовая сетка: по ней создаётся окно, дальше сетку ведёт окно (FR-23 `AutoGrid`).
pub const COLS: usize = 80;
pub const ROWS: usize = 25;
/// Строка и колонка курсора приложения: единственный курсор в окне.
pub const PROMPT_ROW: usize = 13;
pub const PROMPT_COL: usize = 2;
/// Нижняя строка подсказок по клавишам.
pub const HELP_ROW: usize = ROWS - 1;

/// Нарисовать сцену целиком, кроме строк 0–1 и приглашения.
pub fn draw(screen: &mut ScreenBuffer) {
	let white = Color::WHITE;
	let black = Color::BLACK;
	let cols = screen.cols();
	if screen.rows() <= HELP_ROW {
		return; // сетка ниже сцены (FR-23 `AutoGrid`): остаётся уцелевшее при смене размера
	}
	screen.write_row(2, "Обычный текст / plain text 0123456789", white, black);
	write_flagged(screen, 3, "BOLD: жирный глиф + осветление", CellFlags::BOLD);
	write_flagged(screen, 4, "ITALIC: курсивный глиф", CellFlags::ITALIC);
	write_flagged(
		screen,
		5,
		"BOLD ITALIC: жирный курсив",
		CellFlags::BOLD | CellFlags::ITALIC,
	);
	// Альфа (ADR-0013): символ поверх фона ячейки и фон ячейки поверх фона буфера (чёрного).
	write_alpha_row(screen, 6, "альфа символа", |a| {
		(
			Color::from_rgba(0xFF, 0xFF, 0xFF, a),
			Color::from_rgb(0x20, 0x30, 0x90),
		)
	});
	write_flagged(screen, 7, "UNDERLINE", CellFlags::UNDERLINE);
	write_alpha_row(screen, 8, "альфа фона", |a| {
		(Color::WHITE, Color::from_rgba(0xE0, 0x30, 0x30, a))
	});
	screen.write_row(
		9,
		"┌─────┬─────┐ ╔═══╦═══╗ ░▒▓█ ▲▼◄► ☺☻♥♦ ▚▞",
		Color::CYAN,
		black,
	);
	screen.write_row(10, "└─────┴─────┘ ╚═══╩═══╝ ╭──╮ ╱╲╳", Color::CYAN, black);
	draw_map(screen);
	write_flagged(
		screen,
		11,
		"начертания: наклонный жирный",
		CellFlags::BOLD | CellFlags::ITALIC,
	);

	for (i, (name, color)) in PALETTE.iter().enumerate() {
		let col = i * 10;
		if col + 8 <= cols {
			for (k, ch) in name.chars().enumerate() {
				screen.set_cell(col + k, 12, ch, *color, black, CellFlags::empty());
			}
		}
	}

	// Таблица символов: латиница, греческий, кириллица. Проверяет ёмкость атласа (FR-27);
	// все глифы шрифта постранично показывает бинарник `full_table` (FR-27.1).
	let mut row = 15;
	let mut cp = 0x20u32;
	let mut col = 0;
	while row < HELP_ROW {
		match cp {
			0x7F => cp = 0x391, // Α — греческий, затем кириллица с 0x410
			0x3CA => cp = 0x410,
			_ => {}
		}
		let ch = char::from_u32(cp).unwrap_or('\0');
		screen.set_cell(col, row, ch, white, black, CellFlags::empty());
		cp += 1;
		col += 1;
		if col == cols {
			col = 0;
			row += 1;
		}
	}
	screen.write_row(
		HELP_ROW,
		"Esc выход · F11 полный экран · 1/2/3 бэкенд GL/GLES/Software",
		HELP_FG,
		black,
	);
}

/// Фрагмент карты из SVG-тайлов `assets/svg/1000xx.svg` (ADR-0008): тайл — глиф, цвета —
/// ячейки. Справа от текста строк 2–8; на узкой сетке не рисуется.
const MAP: [&str; 7] = [
	"########################",
	"#....T.......~~~~~.....#",
	"#..T....T...~~~~~~~..T.#",
	"#.........T..~~~~~.....#",
	"#..TT..............T...#",
	"#......T.....T.........#",
	"########################",
];
const MAP_COL: usize = 52;
const MAP_ROW: usize = 2;

/// Тайл карты: кодовая точка SVG-глифа, цвет символа и фона.
fn map_tile(symbol: char) -> (char, Color, Color) {
	let grass_bg = Color::from_rgb(0x10, 0x30, 0x10);
	match symbol {
		'#' => (
			'\u{100002}',
			Color::from_rgb(0xA0, 0x60, 0x40),
			Color::from_rgb(0x30, 0x20, 0x18),
		),
		'~' => (
			'\u{100001}',
			Color::from_rgb(0x80, 0xC0, 0xFF),
			Color::from_rgb(0x10, 0x20, 0x60),
		),
		'T' => ('\u{100003}', Color::from_rgb(0x30, 0xD0, 0x30), grass_bg),
		_ => ('\u{100000}', Color::from_rgb(0x40, 0x90, 0x40), grass_bg),
	}
}

fn draw_map(screen: &mut ScreenBuffer) {
	if screen.cols() < MAP_COL + MAP[0].len() {
		return;
	}
	for (dy, line) in MAP.iter().enumerate() {
		for (dx, symbol) in line.chars().enumerate() {
			let (ch, fg, bg) = map_tile(symbol);
			screen.set_cell(MAP_COL + dx, MAP_ROW + dy, ch, fg, bg, CellFlags::empty());
		}
	}
}

/// Текст с флагами белым по чёрному.
/// Значения альфы в строке демо: от прозрачного до непрозрачного.
const ALPHAS: [u8; 5] = [0, 64, 128, 192, 255];

/// Строка с подписью и пятью образцами `Aж█` — по одному на значение из [`ALPHAS`]; цвета образца
/// даёт `colors(a)`, подписи значений непрозрачны. Ширина — 50 колонок, левее карты.
fn write_alpha_row(
	screen: &mut ScreenBuffer,
	row: usize,
	title: &str,
	colors: impl Fn(u8) -> (Color, Color),
) {
	screen.write_row(row, title, Color::WHITE, Color::BLACK);
	let mut col = 15;
	for a in ALPHAS {
		let label = format!("{a:>3} ");
		let (fg, bg) = colors(a);
		let cells = label
			.chars()
			.map(|ch| (ch, Color::WHITE, Color::BLACK))
			.chain("Aж█".chars().map(|ch| (ch, fg, bg)));
		for (ch, fg, bg) in cells {
			if col >= screen.cols() {
				return;
			}
			screen.set_cell(col, row, ch, fg, bg, CellFlags::empty());
			col += 1;
		}
	}
}

pub fn write_flagged(screen: &mut ScreenBuffer, row: usize, text: &str, flags: CellFlags) {
	for (i, ch) in text.chars().enumerate() {
		if i >= screen.cols() {
			break;
		}
		screen.set_cell(i, row, ch, Color::WHITE, Color::BLACK, flags);
	}
}

const PALETTE: [(&str, Color); 8] = [
	("white", Color::WHITE),
	("green", Color::GREEN),
	("magenta", Color::MAGENTA),
	("cyan", Color::CYAN),
	("red", Color::from_rgb(0xFF, 0x40, 0x40)),
	("yellow", Color::from_rgb(0xFF, 0xFF, 0x40)),
	("blue", Color::from_rgb(0x60, 0x80, 0xFF)),
	("grey", Color::from_rgb(0x80, 0x80, 0x80)),
];
