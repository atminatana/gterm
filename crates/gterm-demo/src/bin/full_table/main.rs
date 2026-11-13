//! Второй демо-бинарник (ТЗ FR-27.1): таблица **всех** глифов шрифта постранично. Глиф занимает
//! блок 2×2 ячейки — сам глиф в левой верхней, три остальные — чёрный зазор; у соседних
//! блоков разные и фон, и цвет символа (пары консольной палитры). Клавиши FR-28 действуют;
//! страницы — `PgUp`/`PgDn`/`Home`/`End` и колесо мыши.
//!
//! Сетка следует за окном (FR-23 `AutoGrid`): таблица занимает всю ширину сетки и строки до
//! подсказки, число глифов на странице и число страниц пересчитываются при смене сетки
//! ([`TableLayout`]).
//!
//! Перечень кодовых точек Unicode, для которых у семейства есть глиф, строится один раз до
//! создания окна: перебор `cmap` начертания Regular, процедурной псевдографики и SVG-глифов, отбор по
//! `Rasterizer::has_glyph` (ADR-0009). На стартовой сетке 80×32 страница атласа ≥ 1024² вмещает
//! таблицу при любом размере глифа 8…48 px на Software (страница 2048²); на GL с
//! `GL_MAX_TEXTURE_SIZE = 1024` и крупном глифе сцена может не поместиться — тогда срабатывает
//! штатный откат на Software (NFR-7). На сетке крупнее стартовой глифов на странице больше.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::process::ExitCode;

use gterm::app::{App, AppHandler, Ctx, Grid, PointerEvent};
use gterm::box_drawing::{BOX_FIRST, BOX_LAST};
use gterm::buffer::ScreenBuffer;
use gterm::cell::{CellFlags, Color, Style};
use gterm::config::Config;
use gterm::rasterizer::Rasterizer;
use skrifa::{FontRef, MetadataProvider};
use winit::event::{ElementState, KeyEvent, MouseScrollDelta};
use winit::keyboard::{Key, NamedKey};

use gterm_demo::common::{self, FontSource, FpsMeter};

/// Стартовая сетка: по ней создаётся окно, дальше сетку ведёт окно (FR-23 `AutoGrid`).
const COLS: usize = 80;
const ROWS: usize = 32;
/// Заголовок таблицы (под строкой статуса) и первая строка самой таблицы.
const TABLE_HEADER_ROW: usize = 1;
const TABLE_ROW0: usize = 2;
/// Сторона блока глифа в ячейках: глиф и чёрный зазор в одну ячейку справа и снизу.
const BLOCK: usize = 2;
/// Первая кодовая точка таблицы: управляющие символы глифов не имеют.
const FIRST_CODEPOINT: char = ' ';

/// Раскладка таблицы глифов на текущей сетке в блоках `BLOCK × BLOCK`: вся ширина, строки от
/// `TABLE_ROW0` до последней строки сетки, занятой подсказкой. Неполный блок у края не
/// используется.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TableLayout {
	/// Блоков в строке и строк блоков.
	cols: usize,
	rows: usize,
}

impl TableLayout {
	fn of(grid_cols: usize, grid_rows: usize) -> Self {
		Self {
			cols: grid_cols / BLOCK,
			rows: grid_rows.saturating_sub(TABLE_ROW0 + 1) / BLOCK,
		}
	}

	/// Блоков (глифов) таблицы на сетке; ноль, если сетка ниже таблицы.
	fn cells(self) -> usize {
		self.cols * self.rows
	}

	/// Глифов на странице; не меньше одного — на это число делят.
	fn per_page(self) -> usize {
		self.cells().max(1)
	}

	/// Число страниц, не меньше одной.
	fn pages(self, glyphs: usize) -> usize {
		glyphs.div_ceil(self.per_page()).max(1)
	}

	/// Страница, на которой окажется глиф с номером `index`; за концом перечня — последняя.
	fn page_of(self, index: usize, glyphs: usize) -> usize {
		(index / self.per_page()).min(self.pages(glyphs) - 1)
	}
}

/// 16 цветов консоли: 8 обычных и 8 ярких (порядок ANSI).
const CONSOLE_16: [Color; 16] = [
	Color::from_rgb(0x00, 0x00, 0x00),
	Color::from_rgb(0x80, 0x00, 0x00),
	Color::from_rgb(0x00, 0x80, 0x00),
	Color::from_rgb(0x80, 0x80, 0x00),
	Color::from_rgb(0x00, 0x00, 0x80),
	Color::from_rgb(0x80, 0x00, 0x80),
	Color::from_rgb(0x00, 0x80, 0x80),
	Color::from_rgb(0xC0, 0xC0, 0xC0),
	Color::from_rgb(0x80, 0x80, 0x80),
	Color::from_rgb(0xFF, 0x00, 0x00),
	Color::from_rgb(0x00, 0xFF, 0x00),
	Color::from_rgb(0xFF, 0xFF, 0x00),
	Color::from_rgb(0x00, 0x00, 0xFF),
	Color::from_rgb(0xFF, 0x00, 0xFF),
	Color::from_rgb(0x00, 0xFF, 0xFF),
	Color::from_rgb(0xFF, 0xFF, 0xFF),
];

/// Пары (фон, символ) — индексы `CONSOLE_16`; фоны попарно различны, цвета символа тоже, каждая
/// пара читаема (контраст по яркости не ниже ~4 : 1).
const GLYPH_COLORS: [(usize, usize); 10] = [
	(0, 11),  // чёрный / жёлтый
	(4, 14),  // тёмно-синий / голубой
	(1, 15),  // тёмно-красный / белый
	(5, 10),  // фиолетовый / салатовый
	(6, 0),   // бирюзовый / чёрный
	(7, 4),   // серебристый / тёмно-синий
	(11, 1),  // жёлтый / тёмно-красный
	(14, 5),  // голубой / фиолетовый
	(15, 12), // белый / синий
	(12, 7),  // синий / серебристый
];

/// Цвета блока `(x, y)`: пара `(x + 2y) mod 10` — у соседей по горизонтали, вертикали и диагонали
/// номера отличаются на 1, 2 или 3, то есть разные и фон, и цвет символа.
fn glyph_colors(x: usize, y: usize) -> (Color, Color) {
	let (bg, fg) = GLYPH_COLORS[(x + 2 * y) % GLYPH_COLORS.len()];
	(CONSOLE_16[bg], CONSOLE_16[fg])
}

fn main() -> ExitCode {
	let mut args: Vec<String> = std::env::args().skip(1).collect();
	let font_source = common::font_source(common::take_flag(&mut args, common::SYSTEM_FONT));
	let config = match common::parse_config(args, &common::usage("full_table")) {
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
	let rasterizer = common::load_rasterizer(font_source)?;
	let codepoints = glyph_codepoints(&rasterizer)?;
	let layout = TableLayout::of(COLS, ROWS);
	eprintln!(
		"gterm: глифов: {}, на стартовой сетке {COLS}×{ROWS} страниц по {}: {}",
		codepoints.len(),
		layout.per_page(),
		layout.pages(codepoints.len())
	);
	App::new(
		"gterm — full table",
		ScreenBuffer::new(COLS, ROWS),
		rasterizer,
		config,
		common::demo_options(),
	)
	.run(FullTable::new(codepoints, layout))?;
	Ok(())
}

/// Кодовые точки с глифом в начертании Regular, по возрастанию. Перебирается `cmap` шрифта, а не
/// весь диапазон U+0000–U+10FFFF (1,1 млн вызовов `has_glyph`); псевдографика и SVG-глифы
/// (ADR-0008) не из `cmap` — добавляются отдельно.
fn glyph_codepoints(rasterizer: &Rasterizer) -> Result<Vec<char>, Box<dyn Error>> {
	let face = rasterizer.family().face(Style::Regular);
	let font = FontRef::new(&face.bytes)
		.map_err(|e| format!("{}: не разбирается cmap: {e}", face.path.display()))?;
	let mut codepoints: Vec<char> = font
		.charmap()
		.mappings()
		.filter_map(|(codepoint, _)| char::from_u32(codepoint))
		.chain(BOX_FIRST..=BOX_LAST)
		.chain(rasterizer.svg_glyphs().codepoints())
		.filter(|&ch| ch >= FIRST_CODEPOINT && rasterizer.has_glyph(ch, Style::Regular))
		.collect();
	codepoints.sort_unstable();
	codepoints.dedup();
	Ok(codepoints)
}

struct FullTable {
	backend: &'static str,
	codepoints: Vec<char>,
	/// Раскладка таблицы на сетке последней полной отрисовки.
	layout: TableLayout,
	page: usize,
	fullscreen: bool,
	fps: FpsMeter,
}

impl FullTable {
	fn new(codepoints: Vec<char>, layout: TableLayout) -> Self {
		Self {
			backend: "",
			codepoints,
			layout,
			page: 0,
			fullscreen: false,
			fps: FpsMeter::default(),
		}
	}

	fn last_page(&self) -> usize {
		self.layout.pages(self.codepoints.len()) - 1
	}

	/// Принять сетку экрана. Первый глиф текущей страницы остаётся на экране: страница
	/// пересчитывается по его номеру, а не сбрасывается в начало.
	fn fit_layout(&mut self, screen: &ScreenBuffer) {
		let first_glyph = self.page * self.layout.per_page();
		self.layout = TableLayout::of(screen.cols(), screen.rows());
		self.page = self.layout.page_of(first_glyph, self.codepoints.len());
	}

	/// Заголовок и текущая страница таблицы. Область таблицы сначала очищается чёрным: цветной —
	/// только ячейка глифа, зазоры блоков, неполные блоки у края и хвост последней страницы
	/// остаются чёрными.
	fn draw_table(&self, screen: &mut ScreenBuffer) {
		if screen.rows() <= TABLE_HEADER_ROW {
			return;
		}
		let start = self.page * self.layout.per_page();
		let end = (start + self.layout.per_page()).min(self.codepoints.len());
		let range = if start < end {
			format!(
				"U+{:04X}…U+{:04X}",
				u32::from(self.codepoints[start]),
				u32::from(self.codepoints[end - 1])
			)
		} else {
			String::from("—")
		};
		screen.fill_row(TABLE_HEADER_ROW, Color::WHITE, Color::BLACK);
		screen.write_row(
			TABLE_HEADER_ROW,
			&format!(
				"глифов: {} · страница {}/{} · {range}",
				self.codepoints.len(),
				self.page + 1,
				self.last_page() + 1
			),
			Color::CYAN,
			Color::BLACK,
		);
		let help_row = screen.rows() - 1;
		for row in TABLE_ROW0..help_row {
			screen.fill_row(row, Color::WHITE, Color::BLACK);
		}
		for (index, &ch) in self.codepoints[start..end].iter().enumerate() {
			let (x, y) = (index % self.layout.cols, index / self.layout.cols);
			let (bg, fg) = glyph_colors(x, y);
			let (col, row) = (x * BLOCK, TABLE_ROW0 + y * BLOCK);
			screen.set_cell(col, row, ch, fg, bg, CellFlags::empty());
		}
	}

	/// Всё заново под текущую сетку: при выборе бэкенда и смене сетки.
	fn draw_all(&mut self, ctx: &mut Ctx<'_>) {
		self.fit_layout(ctx.screen);
		self.write_status(ctx);
		self.draw_table(ctx.screen);
		draw_help(ctx.screen);
		ctx.redraw();
	}

	/// Строка статуса: таймера в библиотеке нет, поэтому она обновляется по событиям — полной
	/// отрисовке и смене страницы.
	fn write_status(&mut self, ctx: &mut Ctx<'_>) {
		let fps = self.fps.update(ctx.stats.frames);
		common::write_status(ctx.screen, self.backend, ctx.stats, fps);
	}

	/// Перейти на страницу с ограничением по краям; перерисовываются таблица и статус.
	fn go_to(&mut self, page: usize, ctx: &mut Ctx<'_>) {
		let page = page.min(self.last_page());
		if page == self.page {
			return;
		}
		self.page = page;
		self.write_status(ctx);
		self.draw_table(ctx.screen);
		ctx.redraw();
	}
}

/// Подсказка на последней строке сетки; на сетке ниже таблицы не рисуется — там заголовок.
fn draw_help(screen: &mut ScreenBuffer) {
	if screen.rows() <= TABLE_ROW0 {
		return;
	}
	let row = screen.rows() - 1;
	// После смены сетки в этой строке могли остаться глифы таблицы.
	screen.fill_row(row, common::HELP_FG, Color::BLACK);
	screen.write_row(
		row,
		"PgUp/PgDn/Home/End, колесо — страницы · Esc выход · F11 полный экран · 1/2/3 бэкенд",
		common::HELP_FG,
		Color::BLACK,
	);
}

impl AppHandler for FullTable {
	fn on_backend_changed(&mut self, name: &'static str, ctx: &mut Ctx<'_>) {
		self.backend = name;
		self.draw_all(ctx);
	}

	fn on_grid_resized(&mut self, _old: Grid, ctx: &mut Ctx<'_>) {
		self.draw_all(ctx);
	}

	fn on_key(&mut self, event: &KeyEvent, ctx: &mut Ctx<'_>) {
		if common::handle_common_key(event, &mut self.fullscreen, ctx)
			|| event.state != ElementState::Pressed
		{
			return;
		}
		match event.logical_key {
			Key::Named(NamedKey::PageDown) => self.go_to(self.page + 1, ctx),
			Key::Named(NamedKey::PageUp) => self.go_to(self.page.saturating_sub(1), ctx),
			Key::Named(NamedKey::Home) => self.go_to(0, ctx),
			Key::Named(NamedKey::End) => self.go_to(self.last_page(), ctx),
			_ => {}
		}
	}

	fn on_pointer(&mut self, event: &PointerEvent<'_>, ctx: &mut Ctx<'_>) {
		if let PointerEvent::Wheel {
			delta: MouseScrollDelta::LineDelta(_, lines),
			..
		} = event
		{
			if *lines < 0.0 {
				self.go_to(self.page + 1, ctx);
			} else if *lines > 0.0 {
				self.go_to(self.page.saturating_sub(1), ctx);
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Число глифов BMP в Roboto Mono Nerd Font Mono v3.5.1 (см. `PROJECT_MAP.md`).
	const GLYPHS: usize = 5019;

	#[test]
	fn start_grid_keeps_the_fixed_layout() {
		// 80×32: 40 блоков в строке, (32 − 3) / 2 = 14 строк блоков.
		let layout = TableLayout::of(COLS, ROWS);
		assert_eq!(layout, TableLayout { cols: 40, rows: 14 });
		assert_eq!(layout.per_page(), 560);
		assert_eq!(layout.pages(GLYPHS), 9);
	}

	#[test]
	fn table_follows_the_grid() {
		let narrow = TableLayout::of(40, ROWS);
		assert_eq!(narrow.per_page(), 280);
		assert_eq!(narrow.pages(GLYPHS), 18);

		let large = TableLayout::of(400, 120);
		assert_eq!(large.per_page(), 200 * 58);
		assert_eq!(large.pages(GLYPHS), 1);
	}

	#[test]
	fn incomplete_block_at_the_edge_is_not_used() {
		// Лишняя колонка справа и лишняя строка таблицы (34 − 3 = 31 — нечётно) пустуют.
		assert_eq!(TableLayout::of(81, 33), TableLayout::of(80, 33));
		assert_eq!(TableLayout::of(80, 34), TableLayout::of(80, 33));
	}

	#[test]
	fn neighbouring_blocks_differ_in_background_and_glyph_color() {
		for y in 1..20 {
			for x in 1..50 {
				let (bg, fg) = glyph_colors(x, y);
				assert_ne!(bg, fg, "({x}, {y})");
				for (nx, ny) in [(x + 1, y), (x, y + 1), (x + 1, y + 1), (x + 1, y - 1)] {
					let (nbg, nfg) = glyph_colors(nx, ny);
					assert_ne!(bg, nbg, "background ({x}, {y}) vs ({nx}, {ny})");
					assert_ne!(fg, nfg, "glyph color ({x}, {y}) vs ({nx}, {ny})");
				}
			}
		}
	}

	#[test]
	fn grid_below_the_table_has_no_cells_and_no_division_by_zero() {
		for rows in [1, TABLE_ROW0, TABLE_ROW0 + 1, TABLE_ROW0 + 2] {
			let layout = TableLayout::of(COLS, rows);
			assert_eq!(layout.cells(), 0, "rows = {rows}");
			assert_eq!(layout.per_page(), 1);
			assert_eq!(layout.pages(GLYPHS), GLYPHS);
		}
	}

	#[test]
	fn empty_glyph_list_is_one_page() {
		let layout = TableLayout::of(COLS, ROWS);
		assert_eq!(layout.pages(0), 1);
		assert_eq!(layout.page_of(0, 0), 0);
	}

	#[test]
	fn first_glyph_of_the_page_survives_a_grid_change() {
		let first_glyph = TableLayout::of(COLS, ROWS).per_page(); // страница 2 стартовой сетки
		let narrow = TableLayout::of(40, ROWS);
		let page = narrow.page_of(first_glyph, GLYPHS);
		assert_eq!(page, 2);
		let shown = page * narrow.per_page()..(page + 1) * narrow.per_page();
		assert!(shown.contains(&first_glyph));
	}

	#[test]
	fn page_past_the_end_clamps_to_the_last() {
		let large = TableLayout::of(400, 120);
		assert_eq!(large.pages(GLYPHS), 1);
		assert_eq!(large.page_of(3 * large.per_page(), GLYPHS), 0);
	}
}
