//! Замер NFR-4 / NFR-5 флагом `--bench` (ТЗ §4.3, FR-27, ADR-0006): непрерывные кадры с полной
//! перезаписью сетки — все строки dirty каждый кадр, то есть худший случай для обоих бэкендов.
//!
//! Сетка — по активному бэкенду: 200×60 для GL/GLES (NFR-4: CPU-часть кадра ≤ 1 мс) и 120×40
//! для Software (NFR-5: ≥ 60 FPS при полной перерисовке). Статистика берётся из `Ctx.stats`
//! в `on_frame`; следующий кадр запрашивается оттуда же — это и есть непрерывная перерисовка.
//! Отчёт — в stderr, выход с кодом 0.
//!
//! С флагом `--overlay` поверх сетки каждый кадр ложится один полупрозрачный клиентский буфер на
//! весь экран (ADR-0014): цена наложения в худшем случае.

use std::error::Error;
use std::time::{Duration, Instant};

use gterm::app::{App, AppHandler, AppOptions, Ctx, DrawCtx, Grid, GridMode};
use gterm::buffer::ScreenBuffer;
use gterm::cell::{CellFlags, Color};
use gterm::config::{BackendKind, Config};
use gterm::font::FONT_SIZE_PX;
use gterm::rasterizer::Rasterizer;
use gterm::renderer::RendererKind;

use gterm_demo::common::{self, FontSource};

/// Флаг демо.
pub const BENCH: &str = "--bench";
/// Флаг замера: наложение на весь экран.
pub const OVERLAY: &str = "--overlay";
/// Сетка замера NFR-4 (12 000 ячеек).
const GL_GRID: Grid = Grid::new(200, 60);
/// Сетка замера NFR-5.
const SOFT_GRID: Grid = Grid::new(120, 40);
/// Кадры прогрева: растеризация набора глифов и первая загрузка атласа в статистику не входят.
const WARMUP_FRAMES: u64 = 30;
/// Измеряемых кадров.
const MEASURED_FRAMES: usize = 300;
/// Набор символов сцены — печатная ASCII `!`…`~`: растеризуется на прогреве, дальше горячий путь.
const GLYPH_FIRST: u32 = 0x21;
const GLYPH_COUNT: usize = 94;

/// Запуск замера: стартовая сетка по запрошенному бэкенду, кадры гонит `on_frame`, размер окна —
/// по сетке.
pub fn run(config: Config, overlay: bool, font_source: &FontSource) -> Result<(), Box<dyn Error>> {
	let family = common::load_family(font_source, FONT_SIZE_PX)?;
	let grid = if config.backend == BackendKind::Soft {
		SOFT_GRID
	} else {
		GL_GRID
	};
	let m = family.metrics();
	eprintln!(
		"gterm bench: стартовая сетка {}×{}, логическое окно {}×{} px \
         (если экран меньше — сетка обрезана, замер Software неполный)",
		grid.cols,
		grid.rows,
		grid.cols * m.glyph_w,
		grid.rows * m.glyph_h
	);
	// Без VSync: иначе ожидание монитора ложится в первый GL-вызов следующего кадра, и CPU-часть
	// кадра неотличима от периода монитора (ADR-0006). `total` без VSync — честный максимум FPS.
	let options = AppOptions {
		grid_mode: GridMode::FixedGrid,
		vsync: false,
		..AppOptions::default()
	};
	App::new(
		"gterm bench",
		ScreenBuffer::new(grid.cols, grid.rows),
		Rasterizer::new(family)?,
		config,
		options,
	)
	.run(Bench::new(overlay))?;
	Ok(())
}

/// Приложение-замер: перезаписывает сетку после каждого кадра и копит статистику.
struct Bench {
	backend: &'static str,
	/// Отрисованных кадров с начала (прогрев + замер).
	frame_no: u64,
	cpu: Vec<Duration>,
	total: Vec<Duration>,
	/// Момент первого и последнего измеренного `on_frame` — для FPS по стенным часам.
	first_measured_at: Option<Instant>,
	last_measured_at: Option<Instant>,
	/// Буфер наложения на весь экран (`--overlay`); пересоздаётся под сетку.
	overlay: Option<ScreenBuffer>,
}

impl Bench {
	fn new(overlay: bool) -> Self {
		Self {
			backend: "",
			frame_no: 0,
			cpu: Vec::with_capacity(MEASURED_FRAMES),
			total: Vec::with_capacity(MEASURED_FRAMES),
			first_measured_at: None,
			last_measured_at: None,
			overlay: overlay.then(|| ScreenBuffer::transparent(0, 0)),
		}
	}

	/// Сетка под активный бэкенд: Software меряется на 120×40, GL — на 200×60.
	fn grid_for(backend: &str) -> Grid {
		if backend == RendererKind::Software.name() {
			SOFT_GRID
		} else {
			GL_GRID
		}
	}

	/// Перезаписать все ячейки: содержимое сдвигается на кадр, чтобы ни одна строка не совпала
	/// с прошлым кадром. Стоимость этого цикла — на стороне приложения, в статистику не входит.
	fn fill(&self, screen: &mut ScreenBuffer) {
		let shift = self.frame_no as usize;
		for row in 0..screen.rows() {
			for col in 0..screen.cols() {
				let codepoint = GLYPH_FIRST + ((shift + col + row * 7) % GLYPH_COUNT) as u32;
				let glyph = char::from_u32(codepoint).unwrap_or('\0');
				let fg = if (shift + col + row) % 2 == 0 {
					Color::WHITE
				} else {
					Color::GREEN
				};
				screen.set_cell(col, row, glyph, fg, Color::BLACK, CellFlags::empty());
			}
		}
	}

	fn report(&self, grid: Grid) {
		let frames = self.cpu.len();
		let wall = match (self.first_measured_at, self.last_measured_at) {
			(Some(first), Some(last)) if frames > 1 => last.duration_since(first),
			_ => Duration::ZERO,
		};
		let fps = if wall.is_zero() {
			0.0
		} else {
			(frames - 1) as f64 / wall.as_secs_f64()
		};
		let cpu = Percentiles::of(&self.cpu);
		let total = Percentiles::of(&self.total);
		eprintln!(
			"gterm bench: {}{}, сетка {}×{} ({} ячеек), прогрев {WARMUP_FRAMES} кадров, замер {frames} кадров",
			self.backend,
			if self.overlay.is_some() {
				" + наложение на весь экран"
			} else {
				""
			},
			grid.cols,
			grid.rows,
			grid.cols * grid.rows
		);
		eprintln!(
			"  cpu   mean {:.3} ms · p50 {:.3} · p95 {:.3} · max {:.3}   (NFR-4: GL 200×60 ≤ 1 ms)",
			common::millis(cpu.mean),
			common::millis(cpu.p50),
			common::millis(cpu.p95),
			common::millis(cpu.max)
		);
		eprintln!(
			"  total mean {:.3} ms · p50 {:.3} · p95 {:.3} · max {:.3}",
			common::millis(total.mean),
			common::millis(total.p50),
			common::millis(total.p95),
			common::millis(total.max)
		);
		eprintln!("  fps   {fps:.1}   (NFR-5: Software 120×40 ≥ 60 fps; VSync выключен)");
	}
}

impl AppHandler for Bench {
	fn on_redraw(&mut self, ctx: &mut DrawCtx<'_>) {
		let Some(layer) = &mut self.overlay else {
			return;
		};
		let grid = ctx.grid();
		if (layer.cols(), layer.rows()) != (grid.cols, grid.rows) {
			*layer = ScreenBuffer::transparent(grid.cols, grid.rows);
			for row in 0..grid.rows {
				for col in 0..grid.cols {
					let ch = if (col + row) % 3 == 0 { '█' } else { '░' };
					let fg = Color::from_rgba(0xFF, 0x80, 0x20, 140);
					let bg = Color::from_rgba(0x20, 0x40, 0xC0, 90);
					layer.set_cell(col, row, ch, fg, bg, CellFlags::empty());
				}
			}
		}
		ctx.draw_buffer(layer, 0, 0);
	}

	fn on_backend_changed(&mut self, name: &'static str, ctx: &mut Ctx<'_>) {
		self.backend = name;
		let wanted = Self::grid_for(name);
		if (ctx.screen.cols(), ctx.screen.rows()) != (wanted.cols, wanted.rows) {
			// Стартовая сетка строилась по запрошенному бэкенду; после отката она другая.
			ctx.request.grid = Some(wanted);
		}
		self.fill(ctx.screen);
		ctx.redraw();
	}

	fn on_grid_resized(&mut self, _old: Grid, ctx: &mut Ctx<'_>) {
		self.fill(ctx.screen);
		ctx.redraw();
	}

	fn on_frame(&mut self, ctx: &mut Ctx<'_>) {
		self.frame_no += 1;
		if self.frame_no > WARMUP_FRAMES {
			let now = Instant::now();
			self.first_measured_at.get_or_insert(now);
			self.last_measured_at = Some(now);
			self.cpu.push(ctx.stats.cpu);
			self.total.push(ctx.stats.total);
		}
		if self.cpu.len() >= MEASURED_FRAMES {
			self.report(Grid::new(ctx.screen.cols(), ctx.screen.rows()));
			ctx.exit();
			return;
		}
		self.fill(ctx.screen);
		ctx.redraw();
	}
}

/// Сводка по выборке длительностей.
struct Percentiles {
	mean: Duration,
	p50: Duration,
	p95: Duration,
	max: Duration,
}

impl Percentiles {
	fn of(samples: &[Duration]) -> Self {
		if samples.is_empty() {
			return Self {
				mean: Duration::ZERO,
				p50: Duration::ZERO,
				p95: Duration::ZERO,
				max: Duration::ZERO,
			};
		}
		let mut sorted = samples.to_vec();
		sorted.sort_unstable();
		let sum: Duration = sorted.iter().sum();
		let at = |q: f64| sorted[((sorted.len() - 1) as f64 * q).round() as usize];
		Self {
			mean: sum / sorted.len() as u32,
			p50: at(0.50),
			p95: at(0.95),
			max: sorted[sorted.len() - 1],
		}
	}
}
