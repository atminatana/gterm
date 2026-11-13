//! Бинарник-пример наложения клиентских буферов (ADR-0014, дополнение к ТЗ
//! `docs/spec/2026-10-09-overlay-buffers.md`). Буферы создаёт и хранит приложение; на каждом кадре
//! оно рисует их через `DrawCtx::draw_buffer` поверх основного экрана:
//!
//! - многоцветный символ из трёх слоёв — глифы, подчёркивание другого цвета и полупрозрачная
//!   подсветка, все в одних и тех же ячейках;
//! - полупрозрачная рамка поверх текста, двигается стрелками — проверка частичного кадра;
//! - буфер, выходящий за правый нижний край сетки, — проверка обрезки.
//!
//! Клавиши FR-28 (`1/2/3`, `F11`, `Esc`) общие с `demo`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::process::ExitCode;

use gterm::app::{App, AppHandler, Ctx, DrawCtx, Grid};
use gterm::buffer::ScreenBuffer;
use gterm::cell::{CellFlags, Color};
use gterm::config::Config;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, NamedKey};

use gterm_demo::common::{self, FontSource, FpsMeter};

/// Стартовая сетка; дальше сетку ведёт окно (FR-23 `AutoGrid`).
const COLS: usize = 80;
const ROWS: usize = 25;
/// Строка, в которой лежит многоцветное слово.
const WORD_ROW: i32 = 3;
const WORD_COL: i32 = 2;
const WORD: &str = "МНОГОЦВЕТНЫЙ символ";
/// Размер рамки в ячейках.
const FRAME_COLS: usize = 26;
const FRAME_ROWS: usize = 7;
/// Размер буфера у правого нижнего края.
const CLIP_COLS: usize = 12;
const CLIP_ROWS: usize = 4;
/// Сколько ячеек буфера у края остаётся видимыми по каждой оси.
const CLIP_VISIBLE: usize = 6;

const TEXT: [&str; 8] = [
	"Основной экран — обычный ScreenBuffer приложения, рисуется как всегда.",
	"Поверх него каждый кадр ложатся клиентские буферы: их создаёт и хранит",
	"приложение, а в on_redraw передаёт библиотеке через ctx.draw_buffer.",
	"Порядок вызовов — порядок слоёв: позже нарисованный лежит выше.",
	"Ячейки клиентского буфера прозрачны по умолчанию (ScreenBuffer::transparent),",
	"поэтому буфер закрывает экран только там, где в нём что-то нарисовано.",
	"Альфа символа и фона действует так же, как на основном экране (ADR-0013).",
	"Всё, что выходит за сетку, обрезается: буфер справа внизу виден частично.",
];

fn main() -> ExitCode {
	let mut args: Vec<String> = std::env::args().skip(1).collect();
	let font_source = common::font_source(common::take_flag(&mut args, common::SYSTEM_FONT));
	let config = match common::parse_config(args, &common::usage("overlay")) {
		Ok(Some(config)) => config,
		Ok(None) => return ExitCode::SUCCESS,
		Err(e) => {
			eprintln!("gterm: {e}");
			return ExitCode::FAILURE;
		}
	};
	match run(config, &font_source) {
		Ok(()) => ExitCode::SUCCESS,
		Err(e) => {
			eprintln!("gterm: {e}");
			ExitCode::FAILURE
		}
	}
}

fn run(config: Config, font_source: &FontSource) -> Result<(), Box<dyn Error>> {
	App::new(
		"gterm — overlay",
		ScreenBuffer::new(COLS, ROWS),
		common::load_rasterizer(font_source)?,
		config,
		common::demo_options(),
	)
	.run(OverlayDemo::new())?;
	Ok(())
}

/// Приложение: основной экран плюс собственные буферы наложения.
struct OverlayDemo {
	backend: &'static str,
	fullscreen: bool,
	fps: FpsMeter,
	/// Три слоя многоцветного слова: глифы, подчёркивание, подсветка.
	glyphs: ScreenBuffer,
	underline: ScreenBuffer,
	highlight: ScreenBuffer,
	/// Полупрозрачная рамка и её позиция в ячейках основной сетки.
	frame: ScreenBuffer,
	frame_pos: (i32, i32),
	/// Буфер у правого нижнего края.
	clip: ScreenBuffer,
}

impl OverlayDemo {
	fn new() -> Self {
		Self {
			backend: "",
			fullscreen: false,
			fps: FpsMeter::default(),
			glyphs: word_glyphs(),
			underline: word_underline(),
			highlight: word_highlight(),
			frame: frame_buffer(),
			frame_pos: (30, 8),
			clip: clip_buffer(),
		}
	}

	/// Основной экран: статус, текст, подсказка.
	fn draw_screen(&mut self, ctx: &mut Ctx<'_>) {
		let screen = &mut *ctx.screen;
		let rows = screen.rows();
		for row in 1..rows {
			screen.fill_row(row, Color::WHITE, Color::from_rgb(0x10, 0x14, 0x18));
		}
		for (i, line) in TEXT.iter().enumerate() {
			let row = 6 + i * 2;
			if row + 1 < rows {
				screen.write_row(
					row,
					line,
					Color::from_rgb(0xC8, 0xC8, 0xC8),
					Color::from_rgb(0x10, 0x14, 0x18),
				);
			}
		}
		if rows > 1 {
			screen.fill_row(rows - 1, common::HELP_FG, Color::BLACK);
			screen.write_row(
				rows - 1,
				"Стрелки — двигать рамку · Esc выход · F11 полный экран · 1/2/3 бэкенд",
				common::HELP_FG,
				Color::BLACK,
			);
		}
		self.write_status(ctx);
	}

	fn write_status(&mut self, ctx: &mut Ctx<'_>) {
		let fps = self.fps.update(ctx.stats.frames);
		common::write_status(ctx.screen, self.backend, ctx.stats, fps);
	}
}

impl AppHandler for OverlayDemo {
	fn on_backend_changed(&mut self, name: &'static str, ctx: &mut Ctx<'_>) {
		self.backend = name;
		self.draw_screen(ctx);
		ctx.redraw();
	}

	fn on_grid_resized(&mut self, _old: Grid, ctx: &mut Ctx<'_>) {
		self.draw_screen(ctx);
		ctx.redraw();
	}

	fn on_key(&mut self, event: &KeyEvent, ctx: &mut Ctx<'_>) {
		if common::handle_common_key(event, &mut self.fullscreen, ctx)
			|| event.state != ElementState::Pressed
		{
			return;
		}
		let (dx, dy) = match event.logical_key {
			Key::Named(NamedKey::ArrowLeft) => (-1, 0),
			Key::Named(NamedKey::ArrowRight) => (1, 0),
			Key::Named(NamedKey::ArrowUp) => (0, -1),
			Key::Named(NamedKey::ArrowDown) => (0, 1),
			_ => return,
		};
		self.frame_pos = (self.frame_pos.0 + dx, self.frame_pos.1 + dy);
		self.write_status(ctx);
		ctx.redraw();
	}

	fn on_redraw(&mut self, ctx: &mut DrawCtx<'_>) {
		// Три слоя в одних ячейках: снизу вверх — глифы, подчёркивание, подсветка.
		for layer in [&self.glyphs, &self.underline, &self.highlight] {
			ctx.draw_buffer(layer, WORD_COL, WORD_ROW);
		}
		ctx.draw_buffer(&self.frame, self.frame_pos.0, self.frame_pos.1);
		let grid = ctx.grid();
		let col = grid.cols as i32 - CLIP_VISIBLE as i32;
		let row = grid.rows as i32 - 1 - (CLIP_ROWS as i32 - 1);
		ctx.draw_buffer(&self.clip, col, row);
	}
}

/// Глифы слова: каждая буква своего цвета, фон прозрачный.
fn word_glyphs() -> ScreenBuffer {
	const PALETTE: [Color; 6] = [
		Color::from_rgb(0xFF, 0x50, 0x50),
		Color::from_rgb(0xFF, 0xB0, 0x30),
		Color::from_rgb(0xF0, 0xF0, 0x40),
		Color::from_rgb(0x50, 0xE0, 0x60),
		Color::from_rgb(0x50, 0xB0, 0xFF),
		Color::from_rgb(0xC0, 0x70, 0xFF),
	];
	let mut buffer = ScreenBuffer::transparent(WORD.chars().count(), 1);
	for (i, ch) in WORD.chars().enumerate() {
		let fg = PALETTE[i % PALETTE.len()];
		buffer.set_cell(i, 0, ch, fg, Color::TRANSPARENT, CellFlags::BOLD);
	}
	buffer
}

/// Подчёркивание другого цвета под тем же словом: пустой символ с флагом `UNDERLINE`.
fn word_underline() -> ScreenBuffer {
	let count = WORD.chars().count();
	let mut buffer = ScreenBuffer::transparent(count, 1);
	for i in 0..count {
		buffer.set_cell(
			i,
			0,
			' ',
			Color::from_rgb(0x30, 0xF0, 0xF0),
			Color::TRANSPARENT,
			CellFlags::UNDERLINE,
		);
	}
	buffer
}

/// Полупрозрачная подсветка поверх слова: фон с альфой, растущей к концу слова.
fn word_highlight() -> ScreenBuffer {
	let count = WORD.chars().count();
	let mut buffer = ScreenBuffer::transparent(count, 1);
	for i in 0..count {
		let a = (24 + i * 120 / count.max(1)) as u8;
		buffer.set_cell(
			i,
			0,
			' ',
			Color::TRANSPARENT,
			Color::from_rgba(0xFF, 0xFF, 0xFF, a),
			CellFlags::empty(),
		);
	}
	buffer
}

/// Рамка: полупрозрачный тёмный фон, непрозрачная граница, подпись внутри.
fn frame_buffer() -> ScreenBuffer {
	let back = Color::from_rgba(0x00, 0x30, 0x80, 150);
	let line = Color::from_rgb(0x60, 0xE0, 0xFF);
	let mut buffer = ScreenBuffer::transparent(FRAME_COLS, FRAME_ROWS);
	for row in 0..FRAME_ROWS {
		for col in 0..FRAME_COLS {
			let ch = match (row, col) {
				(0, 0) => '╭',
				(0, c) if c == FRAME_COLS - 1 => '╮',
				(r, 0) if r == FRAME_ROWS - 1 => '╰',
				(r, c) if r == FRAME_ROWS - 1 && c == FRAME_COLS - 1 => '╯',
				(0, _) | (FRAME_ROWS_LAST, _) => '─',
				(_, 0) | (_, FRAME_COLS_LAST) => '│',
				_ => ' ',
			};
			buffer.set_cell(col, row, ch, line, back, CellFlags::empty());
		}
	}
	let label = "полупрозрачная рамка";
	for (i, ch) in label.chars().enumerate() {
		buffer.set_cell(
			2 + i,
			FRAME_ROWS / 2,
			ch,
			Color::from_rgba(0xFF, 0xFF, 0xFF, 220),
			back,
			CellFlags::ITALIC,
		);
	}
	buffer
}

const FRAME_ROWS_LAST: usize = FRAME_ROWS - 1;
const FRAME_COLS_LAST: usize = FRAME_COLS - 1;

/// Буфер у края: шахматка из двух полупрозрачных цветов с номерами колонок.
fn clip_buffer() -> ScreenBuffer {
	let mut buffer = ScreenBuffer::transparent(CLIP_COLS, CLIP_ROWS);
	for row in 0..CLIP_ROWS {
		for col in 0..CLIP_COLS {
			let bg = if (row + col) % 2 == 0 {
				Color::from_rgba(0xE0, 0x40, 0x40, 170)
			} else {
				Color::from_rgba(0x40, 0xC0, 0x40, 170)
			};
			let digit = char::from_digit((col % 10) as u32, 10).unwrap_or('?');
			buffer.set_cell(col, row, digit, Color::WHITE, bg, CellFlags::empty());
		}
	}
	buffer
}
