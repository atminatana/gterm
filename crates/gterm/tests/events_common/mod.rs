//! Общая часть оконных интеграционных тестов (этапы 6 и 8): приложение-испытатель и один запуск
//! окна.
//!
//! Живёт в отдельном модуле, потому что `winit` запрещает создавать второй цикл событий в
//! процессе: каждый режим сетки (FR-23) проверяется отдельным тест-бинарником со своим `main`.

mod launch;

use std::sync::{Arc, Mutex};

use gterm::app::{AppError, AppHandler, AppOptions, Ctx, DrawCtx, Grid, GridMode};
use gterm::buffer::ScreenBuffer;
use gterm::cell::{CellFlags, Color};
use gterm::config::Config;
use gterm::rasterizer::BoxDrawingMode;
use gterm::renderer::RendererKind;

/// Метка в левом верхнем углу: обязана пережить смену сетки (FR-6, FR-23).
pub const MARKER: char = 'X';
/// Сетка, которую приложение просит при первом уведомлении.
pub const WANTED: Grid = Grid { cols: 40, rows: 12 };
/// Сколько кадров рисуем до выхода: тик приложение-испытатель не заводит, цикл ведёт оно само,
/// запрашивая следующий кадр из `on_frame` (ADR-0006) — как замер `--bench`.
pub const FRAMES_BEFORE_EXIT: u64 = 5;

/// Что приложение узнало за время работы окна.
#[derive(Default, Debug)]
pub struct Log {
	/// Имена бэкендов, о которых сообщила библиотека (FR-20, FR-21).
	pub backends: Vec<&'static str>,
	/// Старые сетки в уведомлениях `on_grid_resized` (FR-23).
	pub resized_from: Vec<Grid>,
	/// Сетка на момент последнего уведомления.
	pub last_grid: Option<Grid>,
	/// `ctx.stats.frames` на момент выхода (ADR-0006).
	pub frames: u64,
	/// Сколько раз пришёл `on_frame` (ADR-0006): по одному на каждый отрисованный кадр.
	pub frame_events: u64,
	/// Сколько раз пришёл `on_redraw` (ADR-0014): не реже, чем отрисованные кадры.
	pub redraws: u64,
	/// Остался ли маркер в углу при переходе на новую сетку (FR-6, FR-23).
	pub marker_kept: Option<bool>,
	/// `ctx.cell_px` на последнем кадре (ADR-0026): метрики атласа активного бэкенда.
	pub cell_px: Option<(u32, u32)>,
	/// `ctx.modifiers.is_empty()` на последнем кадре (ADR-0026): клавиатуру тест не трогает,
	/// поэтому модификаторы обязаны быть пусты.
	pub modifiers_empty: Option<bool>,
}

/// Приложение-испытатель: задаёт сетку, проверяет содержимое и уходит по `ctx.exit()`.
pub struct Probe {
	log: Arc<Mutex<Log>>,
	frames_before_exit: u64,
	/// На каком кадре попросить смену бэкенда — проверка отката на лету (FR-21).
	switch_at_frame: Option<u64>,
}

impl Probe {
	/// Записать наблюдение и вернуть результат замыкания (счётчик, флаг).
	fn note<T>(&self, f: impl FnOnce(&mut Log) -> T) -> T {
		let mut log = self.log.lock().expect("log");
		f(&mut log)
	}
}

impl AppHandler for Probe {
	fn on_redraw(&mut self, ctx: &mut DrawCtx<'_>) {
		// Полупрозрачная метка поверх угла: путь наложения проходит каждый кадр на GL и Software.
		let mut mark = ScreenBuffer::transparent(2, 1);
		mark.write_row(
			0,
			"ok",
			Color::from_rgba(0xFF, 0xFF, 0, 160),
			Color::TRANSPARENT,
		);
		let grid = ctx.grid();
		ctx.draw_buffer(&mark, grid.cols as i32 - 1, -1);
		ctx.draw_buffer(&mark, 0, 0);
		self.note(|log| log.redraws += 1);
	}

	fn on_backend_changed(&mut self, name: &'static str, ctx: &mut Ctx<'_>) {
		self.note(|log| {
			log.backends.push(name);
			log.last_grid = Some(Grid::new(ctx.screen.cols(), ctx.screen.rows()));
		});
		// Уведомление приходит до первого кадра: просим сетку (FR-23) и перерисовку.
		ctx.request.grid = Some(WANTED);
		ctx.redraw();
	}

	fn on_grid_resized(&mut self, old: Grid, ctx: &mut Ctx<'_>) {
		let marker_kept = ctx.screen.cell(0, 0).ch == MARKER;
		self.note(|log| {
			log.resized_from.push(old);
			log.last_grid = Some(Grid::new(ctx.screen.cols(), ctx.screen.rows()));
			log.marker_kept = Some(marker_kept);
		});
		ctx.redraw();
	}

	fn on_frame(&mut self, ctx: &mut Ctx<'_>) {
		// `ctx.stats` уже учитывает этот кадр: счётчики обязаны идти вровень.
		let frames = ctx.stats.frames;
		let cell_px = ctx.cell_px;
		let modifiers_empty = ctx.modifiers.is_empty();
		let frame_events = self.note(|log| {
			log.frame_events += 1;
			log.frames = frames;
			log.cell_px = Some(cell_px);
			log.modifiers_empty = Some(modifiers_empty);
			log.frame_events
		});
		// ADR-0018: смена источника псевдографики на лету на любом бэкенде — кадры идут дальше.
		if frame_events == 2 {
			ctx.request.box_drawing = Some(BoxDrawingMode::Font);
		}
		if self.switch_at_frame == Some(frame_events) {
			ctx.request.backend = Some(RendererKind::Software);
			// Выходим после того, как смена подтверждена уведомлением и ещё двумя кадрами.
			self.switch_at_frame = None;
			self.frames_before_exit = self.frames_before_exit.max(frame_events + 2);
		}
		// Ошибочный кадр сюда не приходит: повтор и откат запрашивает сама библиотека, а
		// остановить цикл из-за ошибки кадра обязана она же (§5.3 п.5).
		if frame_events >= self.frames_before_exit {
			ctx.exit();
		} else {
			ctx.redraw();
		}
	}
}

/// Один запуск приложения, обязанный завершиться штатно; `None` — окно недоступно, это пропуск.
pub fn run_once(
	config: Config,
	grid_mode: GridMode,
	switch_at_frame: Option<u64>,
) -> Option<Arc<Mutex<Log>>> {
	let finished = run_to_end(config, grid_mode, switch_at_frame)?;
	if let Err(e) = finished.result {
		panic!("application loop failed: {e}");
	}
	Some(finished.log)
}

/// Итог запуска: что узнало приложение и чем закончился `App::run`.
pub struct Finished {
	pub log: Arc<Mutex<Log>>,
	pub result: Result<(), AppError>,
}

/// Один запуск приложения на настоящем окне: журнал и то, чем закончился `App::run`.
/// `None` — окно недоступно, это пропуск.
pub fn run_to_end(
	config: Config,
	grid_mode: GridMode,
	switch_at_frame: Option<u64>,
) -> Option<Finished> {
	let log = Arc::new(Mutex::new(Log::default()));
	let probe = Probe {
		log: Arc::clone(&log),
		frames_before_exit: FRAMES_BEFORE_EXIT,
		switch_at_frame,
	};
	let options = AppOptions {
		grid_mode,
		resizable: true,
		vsync: true,
		..AppOptions::default()
	};
	let result = launch::run_handler(probe, start_screen(), config, options)?;
	Some(Finished { log, result })
}

/// Цикл не завис: приложение получило все кадры, которые просило до выхода.
pub fn assert_kept_drawing(log: &Log) {
	assert!(
		log.frame_events >= FRAMES_BEFORE_EXIT,
		"on_frame пришёл {} раз, ожидалось не меньше {FRAMES_BEFORE_EXIT}",
		log.frame_events
	);
}

/// ADR-0006: счётчик кадров растёт и `on_frame` пришёл ровно по разу на каждый отрисованный кадр.
pub fn assert_frame_stats(log: &Log) {
	assert!(log.frames >= 1, "ни одного кадра в ctx.stats.frames");
	assert_eq!(
		log.frame_events, log.frames,
		"on_frame пришёл {} раз, а кадров отрисовано {}",
		log.frame_events, log.frames
	);
	assert!(
		log.redraws >= log.frame_events,
		"on_redraw пришёл {} раз, а кадров отрисовано {}",
		log.redraws,
		log.frame_events
	);
}

/// Имена всех бэкендов, о которых сообщила библиотека, должны быть из цепочки FR-17.
pub fn assert_known_backends(log: &Log) -> &[&'static str] {
	for backend in &log.backends {
		assert!(
			["OpenGL 3.3", "OpenGL ES 3.0", "Software"].contains(backend),
			"неизвестный бэкенд {backend}"
		);
	}
	&log.backends
}

/// Экран с меткой в углу, которая обязана пережить смену сетки.
fn start_screen() -> ScreenBuffer {
	let mut screen = ScreenBuffer::new(24, 6);
	screen.set_cell(0, 0, MARKER, Color::WHITE, Color::BLACK, CellFlags::empty());
	screen.write_row(1, "текст", Color::GREEN, Color::BLACK);
	screen
}
