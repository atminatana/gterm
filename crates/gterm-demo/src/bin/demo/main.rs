//! Демо-бинарник (ТЗ §5.1, FR-27, FR-28): конфигурация из аргументов и окружения (FR-19),
//! окно, цепочка бэкендов, сцена со всеми ветками алгоритма, клавиши, счётчик FPS и курсор
//! приложения. Флаг `--bench` — замер NFR-4/NFR-5 (`bench.rs`).
//!
//! Приложение реализует [`AppHandler`] (ADR-0004): ввод приходит как есть, экран принадлежит
//! `App` и выдаётся в `Ctx`, курсор рисует приложение — библиотека о нём не знает. Статистику
//! кадров даёт `Ctx` (ADR-0006). Зума нет (FR-28), размер шрифта задаёт DPI окна
//! (FR-24, ADR-0005).

// В релизной сборке на Windows скрываем консольное окно (только окно приложения). stderr при
// этом не виден — для `--bench` перенаправляйте его в файл: `demo --bench 2> bench.txt`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bench;
mod scene;

use std::error::Error;
use std::process::ExitCode;

use gterm::app::{App, AppHandler, Ctx, Grid, PointerEvent};
use gterm::buffer::ScreenBuffer;
use gterm::cell::{CellFlags, Color};
use gterm::config::Config;
use winit::event::{ElementState, KeyEvent, MouseButton};
use winit::keyboard::{Key, NamedKey};

use gterm_demo::common::{self, FontSource, FpsMeter};
use scene::{COLS, PROMPT_COL, PROMPT_ROW, ROWS};

fn main() -> ExitCode {
	let mut args: Vec<String> = std::env::args().skip(1).collect();
	let bench = common::take_flag(&mut args, bench::BENCH);
	let bench_overlay = common::take_flag(&mut args, bench::OVERLAY);
	let font_source = common::font_source(common::take_flag(&mut args, common::SYSTEM_FONT));
	let config = match common::parse_config(args, &usage()) {
		Ok(Some(config)) => config,
		Ok(None) => return ExitCode::SUCCESS,
		Err(e) => {
			eprintln!("gterm: {e}");
			return ExitCode::FAILURE;
		}
	};
	let result = if bench {
		bench::run(config, bench_overlay, &font_source)
	} else {
		run(config, &font_source)
	};
	match result {
		Ok(()) => ExitCode::SUCCESS,
		Err(e) => {
			eprintln!("gterm: {e}");
			ExitCode::FAILURE
		}
	}
}

/// Справка: ключи библиотеки плюс ключи самого демо.
fn usage() -> String {
	format!(
		"{}\n\
         \x20 --bench          замер NFR-4/NFR-5: непрерывные кадры с полной перезаписью сетки, \
         статистика в stderr
           --overlay        вместе с --bench: полупрозрачный буфер наложения на весь экран",
		common::usage("demo")
	)
}

fn run(config: Config, font_source: &FontSource) -> Result<(), Box<dyn Error>> {
	App::new(
		"gterm",
		ScreenBuffer::new(COLS, ROWS),
		common::load_rasterizer(font_source)?,
		config,
		common::demo_options(),
	)
	.run(Demo::new())?;
	Ok(())
}

/// Приложение: сцена, курсор, строка статуса, реакция на клавиши (FR-25, FR-27, FR-28).
///
/// Экрана у него нет — буфер остаётся у `App` и выдаётся в [`Ctx`] (ADR-0004).
struct Demo {
	/// Имя активного бэкенда для строки статуса.
	backend: &'static str,
	/// Позиция курсора приложения.
	col: usize,
	row: usize,
	/// Полный экран включён: библиотека состояние не хранит, помнит приложение.
	fullscreen: bool,
	/// Последняя позиция указателя в физических пикселях окна.
	pointer: (f64, f64),
	/// Строка ввода, которую пользователь набирает.
	input: String,
	/// FPS между событиями, обновившими строку статуса (FR-27).
	fps: FpsMeter,
}

impl Demo {
	fn new() -> Self {
		Self {
			backend: "",
			col: PROMPT_COL,
			row: PROMPT_ROW,
			fullscreen: false,
			pointer: (0.0, 0.0),
			input: String::new(),
			fps: FpsMeter::default(),
		}
	}

	/// Строка 0 — бэкенд и статистика кадров (FR-20, FR-27). Таймера в библиотеке нет, поэтому
	/// статус обновляется по событиям; по `on_frame` нельзя — запись в экран породила бы петлю.
	fn write_status(&mut self, ctx: &mut Ctx<'_>) {
		let fps = self.fps.update(ctx.stats.frames);
		common::write_status(ctx.screen, self.backend, ctx.stats, fps);
	}

	/// Строка 1 — позиция указателя: события мыши приходят без интерпретации (FR-25).
	fn write_pointer(&mut self, screen: &mut ScreenBuffer) {
		if screen.rows() < 2 {
			return;
		}
		screen.write_row(
			1,
			&format!("мышь: {:.0},{:.0}", self.pointer.0, self.pointer.1),
			Color::from_rgb(0x60, 0x80, 0xFF),
			Color::BLACK,
		);
	}

	/// Перерисовать строку ввода и поставить на неё курсор.
	fn redraw_prompt(&mut self, screen: &mut ScreenBuffer) {
		if self.row >= screen.rows() {
			return;
		}
		let cols = screen.cols();
		screen.fill_row(self.row, Color::GREEN, Color::BLACK);
		screen.set_cell(
			0,
			self.row,
			'>',
			Color::GREEN,
			Color::BLACK,
			CellFlags::empty(),
		);
		for (i, ch) in self.input.chars().enumerate() {
			let col = PROMPT_COL + i;
			if col >= cols {
				break;
			}
			screen.set_cell(
				col,
				self.row,
				ch,
				Color::WHITE,
				Color::BLACK,
				CellFlags::empty(),
			);
		}
		self.col = (PROMPT_COL + self.input.chars().count()).min(cols.saturating_sub(1));
		self.draw_cursor(screen);
	}

	/// Курсор рисует приложение: цвета своей ячейки меняет местами (ADR-0004, FR-27). Снимать его
	/// не нужно — `redraw_prompt` переписывает строку целиком перед каждым вызовом.
	fn draw_cursor(&mut self, screen: &mut ScreenBuffer) {
		if self.col >= screen.cols() || self.row >= screen.rows() {
			return;
		}
		let cell = screen.cell(self.col, self.row);
		screen.set_cell(self.col, self.row, cell.ch, cell.bg, cell.fg, cell.flags);
	}

	/// Вся сцена заново: при выборе бэкенда и смене сетки.
	fn redraw_all(&mut self, ctx: &mut Ctx<'_>) {
		scene::draw(ctx.screen);
		self.write_pointer(ctx.screen);
		self.write_status(ctx);
		self.redraw_prompt(ctx.screen);
		ctx.redraw();
	}
}

impl AppHandler for Demo {
	fn on_backend_changed(&mut self, name: &'static str, ctx: &mut Ctx<'_>) {
		// Событие приходит до первого кадра: самое время нарисовать сцену целиком.
		self.backend = name;
		self.redraw_all(ctx);
	}

	fn on_grid_resized(&mut self, _old: Grid, ctx: &mut Ctx<'_>) {
		// FR-23: содержимое сохранилось в пересекающейся области, достраиваем под новый размер.
		self.redraw_all(ctx);
	}

	fn on_key(&mut self, event: &KeyEvent, ctx: &mut Ctx<'_>) {
		let handled = common::handle_common_key(event, &mut self.fullscreen, ctx);
		if !handled
			&& event.state == ElementState::Pressed
			&& event.logical_key == Key::Named(NamedKey::Backspace)
		{
			self.input.pop();
		}
		self.write_status(ctx);
		self.redraw_prompt(ctx.screen);
		ctx.redraw();
	}

	fn on_text(&mut self, text: &str, ctx: &mut Ctx<'_>) {
		self.input.push_str(text);
		self.write_status(ctx);
		self.redraw_prompt(ctx.screen);
		ctx.redraw();
	}

	fn on_pointer(&mut self, event: &PointerEvent<'_>, ctx: &mut Ctx<'_>) {
		match event {
			PointerEvent::Moved { position, .. } => {
				self.pointer = (position.x, position.y);
				self.write_pointer(ctx.screen);
			}
			PointerEvent::Input {
				state: ElementState::Pressed,
				button: MouseButton::Left,
				..
			} => {
				self.input.push_str(" [клик] ");
				self.write_status(ctx);
				self.redraw_prompt(ctx.screen);
			}
			_ => return,
		}
		ctx.redraw();
	}
}
