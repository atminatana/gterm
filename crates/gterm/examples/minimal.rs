//! Минимальный потребитель библиотеки (ADR-0017, критерий К13): без каталога `assets/` — шрифт
//! системы по умолчанию (`FontFamily::system_monospace`), несколько строк текста и рамка
//! псевдографики. Запуск: `cargo run -p gterm --example minimal [-- --backend=gl|gles|soft]`;
//! выход — закрыть окно.

use std::error::Error;
use std::process::ExitCode;

use gterm::app::{App, AppHandler, AppOptions};
use gterm::buffer::ScreenBuffer;
use gterm::cell::{CellFlags, Color, Style};
use gterm::config::{BACKEND_ENV, Config};
use gterm::font::FontFamily;
use gterm::rasterizer::Rasterizer;

const COLS: usize = 44;
const ROWS: usize = 7;
const FG: Color = Color::from_rgb(0xE0, 0xE0, 0xE0);
const BG: Color = Color::from_rgb(0x10, 0x18, 0x28);
const FRAME: Color = Color::from_rgb(0x60, 0xA0, 0xFF);

fn main() -> ExitCode {
	let env_backend = std::env::var(BACKEND_ENV).ok();
	let config = match Config::parse(std::env::args().skip(1), env_backend.as_deref()) {
		Ok(config) if config.help => {
			println!("{}", Config::usage("minimal"));
			return ExitCode::SUCCESS;
		}
		Ok(config) => config,
		Err(e) => {
			eprintln!("minimal: {e}");
			return ExitCode::FAILURE;
		}
	};
	match run(config) {
		Ok(()) => ExitCode::SUCCESS,
		Err(e) => {
			eprintln!("minimal: {e}");
			ExitCode::FAILURE
		}
	}
}

fn run(config: Config) -> Result<(), Box<dyn Error>> {
	println!("minimal: searching for system monospace font (this may take a moment)...");
	let family = FontFamily::system_monospace()?;
	eprintln!(
		"minimal: font '{}' — {}",
		family.family(),
		family.face(Style::Regular).path.display()
	);
	let mut screen = ScreenBuffer::with_default_colors(COLS, ROWS, FG, BG);
	draw_scene(&mut screen);
	App::new(
		"gterm minimal",
		screen,
		Rasterizer::new(family)?,
		config,
		AppOptions::default(),
	)
	.run(Minimal)?;
	Ok(())
}

/// Рамка из псевдографики по краю сетки и три строки текста внутри.
fn draw_scene(screen: &mut ScreenBuffer) {
	let (last_col, last_row) = (COLS - 1, ROWS - 1);
	for col in 1..last_col {
		screen.set_cell(col, 0, '─', FRAME, BG, CellFlags::empty());
		screen.set_cell(col, last_row, '─', FRAME, BG, CellFlags::empty());
	}
	for row in 1..last_row {
		screen.set_cell(0, row, '│', FRAME, BG, CellFlags::empty());
		screen.set_cell(last_col, row, '│', FRAME, BG, CellFlags::empty());
	}
	for (col, row, ch) in [
		(0, 0, '┌'),
		(last_col, 0, '┐'),
		(0, last_row, '└'),
		(last_col, last_row, '┘'),
	] {
		screen.set_cell(col, row, ch, FRAME, BG, CellFlags::empty());
	}
	let lines = [
		("gterm без assets/: системный шрифт", CellFlags::BOLD),
		(
			"The quick brown fox jumps over the lazy dog",
			CellFlags::empty(),
		),
		(
			"Съешь же ещё этих мягких французских булок",
			CellFlags::ITALIC,
		),
		(
			"0123456789 !\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~",
			CellFlags::UNDERLINE,
		),
	];
	for (i, (text, flags)) in lines.iter().enumerate() {
		for (k, ch) in text.chars().enumerate().take(COLS - 2) {
			screen.set_cell(1 + k, 1 + i, ch, FG, BG, *flags);
		}
	}
}

/// Приложение без собственных событий: сцена статична, выход — закрытие окна.
struct Minimal;

impl AppHandler for Minimal {}
