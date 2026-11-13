//! Режим псевдографики (ADR-0018): процедурный, шрифтовой и комбинированный источники
//! U+2500–U+259F, каналы атласа по режиму, смена режима на лету в Software.

mod common;

use common::{BG, FG, RenderWith, repo_atlas, repo_rasterizer};
use gterm::atlas::{Atlas, PAGE_PX_MIN};
use gterm::box_drawing::{BOX_FIRST, BOX_LAST};
use gterm::buffer::ScreenBuffer;
use gterm::cell::{CellFlags, Color, Style};
use gterm::rasterizer::{BoxDrawingMode, Rasterizer};
use gterm::renderer::Renderer;
use gterm::renderer::soft_backend::SoftBackend;
use skrifa::{FontRef, MetadataProvider};

/// Знак, который есть в любой таблице: крест одиночных линий.
const CROSS: char = '┼';

fn coverage(r: &mut Rasterizer, ch: char, style: Style) -> (bool, Vec<u8>) {
	let m = r.metrics();
	let mut buf = vec![0u8; m.glyph_w * m.glyph_h];
	let drawn = r.rasterize(ch, style, &mut buf);
	(drawn, buf)
}

/// Кодовые точки диапазона, у которых в `cmap` Regular-файла есть глиф, и у которых нет.
fn split_range_by_font(r: &Rasterizer) -> (Vec<char>, Vec<char>) {
	let font = FontRef::new(&r.family().face(Style::Regular).bytes).unwrap();
	let charmap = font.charmap();
	(BOX_FIRST..=BOX_LAST).partition(|&ch| charmap.map(ch).is_some())
}

/// Экран с рамками трёх видов и жирной строкой: все ветки режима.
fn box_screen() -> ScreenBuffer {
	let mut screen = ScreenBuffer::new(16, 4);
	screen.write_row(0, "┌─┬┐├┼┤└┴┘ ─│", FG, BG);
	screen.write_row(1, "╔═╦╗╠╬╣╚╩╝ ═║", FG, BG);
	screen.write_row(2, "┏━┯┓┠┼┨┗┷┛ ░▒▓█", FG, BG);
	for (i, ch) in "┼╬═━ bold".chars().enumerate() {
		screen.set_cell(i, 3, ch, Color::GREEN, Color::BLACK, CellFlags::BOLD);
	}
	screen
}

/// К2: режим по умолчанию — процедурный; `Font` берёт глиф шрифта (если он есть) и отличается от
/// формулы; на кодовой точке без глифа `Font` пуст, а `Combined` равен `Procedural`.
#[test]
fn modes_pick_their_source() {
	let mut r = repo_rasterizer();
	assert_eq!(r.box_drawing_mode(), BoxDrawingMode::Procedural);
	let (present, absent) = split_range_by_font(&r);
	eprintln!(
		"шрифт репозитория: {} глифов диапазона в cmap, {} отсутствуют",
		present.len(),
		absent.len()
	);

	if present.is_empty() {
		eprintln!("ветка «глиф есть в шрифте» не покрыта: в шрифте репозитория нет рамок");
	} else {
		// Простая горизонталь может совпасть с формулой пиксель в пиксель; различие ищется по
		// всему диапазону, а равенство `Combined` = `Font` при наличии глифа — для каждой точки.
		let mut differing = 0;
		for &ch in &present {
			r.set_box_drawing_mode(BoxDrawingMode::Procedural);
			let (drawn_p, procedural) = coverage(&mut r, ch, Style::Regular);
			r.set_box_drawing_mode(BoxDrawingMode::Font);
			let (drawn_f, from_font) = coverage(&mut r, ch, Style::Regular);
			assert!(drawn_p, "U+{:04X}: формула рисует", u32::from(ch));
			r.set_box_drawing_mode(BoxDrawingMode::Combined);
			assert_eq!(
				coverage(&mut r, ch, Style::Regular),
				(drawn_f, from_font.clone()),
				"U+{:04X}: Combined при наличии глифа — шрифт",
				u32::from(ch)
			);
			if procedural != from_font {
				differing += 1;
			}
		}
		eprintln!(
			"глиф шрифта отличается от формулы у {differing} из {}",
			present.len()
		);
		assert!(
			differing > 0,
			"ни один глиф шрифта не отличается от формулы"
		);
	}

	if let Some(&ch) = absent.first() {
		r.set_box_drawing_mode(BoxDrawingMode::Procedural);
		let (drawn_p, procedural) = coverage(&mut r, ch, Style::Regular);
		assert!(drawn_p);
		r.set_box_drawing_mode(BoxDrawingMode::Font);
		let (drawn_f, from_font) = coverage(&mut r, ch, Style::Regular);
		assert!(
			!drawn_f,
			"U+{:04X}: в режиме Font без глифа — пусто",
			u32::from(ch)
		);
		assert!(from_font.iter().all(|&v| v == 0));
		assert!(
			!r.has_glyph(ch, Style::Regular),
			"has_glyph в Font — по cmap"
		);
		r.set_box_drawing_mode(BoxDrawingMode::Combined);
		assert_eq!(
			coverage(&mut r, ch, Style::Regular),
			(true, procedural),
			"U+{:04X}: Combined без глифа — формула",
			u32::from(ch)
		);
	} else {
		eprintln!("ветка «глифа нет в шрифте» не покрыта: шрифт покрывает весь диапазон");
	}
}

/// К3: в `Procedural` псевдографика одинакова во всех каналах, в `Combined`/`Font` — по
/// начертаниям; смена режима сбрасывает страницу.
#[test]
fn style_independence_follows_mode() {
	let mut atlas = Atlas::new(repo_rasterizer(), PAGE_PX_MIN);
	let slot = atlas.ensure(CROSS, Style::Regular);
	assert!(
		Style::ALL.iter().all(|&s| atlas.is_ready(slot, s)),
		"Procedural: все четыре канала готовы после одного начертания"
	);
	assert_eq!(atlas.resets(), 0);

	atlas.set_box_drawing_mode(BoxDrawingMode::Combined);
	assert_eq!(atlas.resets(), 1, "смена режима сбрасывает страницу");
	assert_eq!(atlas.slot_of(CROSS), None, "прежний слот недействителен");
	let slot = atlas.ensure(CROSS, Style::Regular);
	assert!(atlas.is_ready(slot, Style::Regular));
	assert!(
		!atlas.is_ready(slot, Style::Bold),
		"Combined: канал Bold заполняется отдельно"
	);

	atlas.set_box_drawing_mode(BoxDrawingMode::Combined);
	assert_eq!(atlas.resets(), 1, "тот же режим — без сброса");
}

/// К4: после смены режима Software перерисовывает кадр целиком — он равен кадру свежего
/// рендерера в том же режиме, хотя у экрана нет dirty-строк.
#[test]
fn switching_mode_redraws_the_whole_frame() {
	let mut screen = box_screen();
	let (w, h) = (16 * 10 + 6, 4 * 21 + 4);
	let mut soft = SoftBackend::new(repo_atlas());
	soft.resize(w, h).unwrap();
	soft.render_with(&mut screen, &BG).unwrap();
	assert!(!screen.dirty_rows().iter().any(|&d| d));

	soft.set_box_drawing_mode(BoxDrawingMode::Combined);
	soft.render_with(&mut screen, &BG).unwrap();
	assert_eq!(soft.atlas().resets(), 1);

	let mut fresh = SoftBackend::new(repo_atlas());
	fresh.set_box_drawing_mode(BoxDrawingMode::Combined);
	fresh.resize(w, h).unwrap();
	fresh.render_with(&mut box_screen(), &BG).unwrap();
	assert_eq!(
		soft.frame(),
		fresh.frame(),
		"кадр после смены режима = кадр с нуля"
	);

	// Обратно в процедурный: кадр равен самому первому (та же сцена, тот же источник).
	let mut again = SoftBackend::new(repo_atlas());
	again.resize(w, h).unwrap();
	again.render_with(&mut box_screen(), &BG).unwrap();
	soft.set_box_drawing_mode(BoxDrawingMode::Procedural);
	soft.render_with(&mut screen, &BG).unwrap();
	assert_eq!(soft.frame(), again.frame());
}

/// Суммарное покрытие колонки `x` и строки `y` ячейки: «краска» линии, проходящей через них.
fn column_ink(buf: &[u8], w: usize, h: usize, x: usize) -> u32 {
	(0..h).map(|y| u32::from(buf[y * w + x])).sum()
}

fn row_ink(buf: &[u8], w: usize, y: usize) -> u32 {
	(0..w).map(|x| u32::from(buf[y * w + x])).sum()
}

/// Все значения равны с точностью до `tolerance` — линия одинаково плотна по всей длине.
fn assert_uniform(values: &[u32], tolerance: u32, what: &str) {
	let (min, max) = (*values.iter().min().unwrap(), *values.iter().max().unwrap());
	assert!(
		min > 0 && max - min <= tolerance,
		"{what}: краска по длине не равномерна: {values:?}"
	);
}

/// К2 (ADR-0020): в режиме `Font` рамки вписаны в ячейку — у `─` краска в крайних колонках равна
/// краске в середине (линия доходит до обоих краёв с полной плотностью), у `│` — то же по
/// крайним строкам, у `┼` — и то и другое. Без вписывания последняя колонка `─` получала бы лишь
/// долю advance за округлением ширины ячейки.
#[test]
fn font_box_glyphs_reach_cell_edges() {
	let mut r = repo_rasterizer();
	r.set_box_drawing_mode(BoxDrawingMode::Font);
	let m = r.metrics();
	let (w, h) = (m.glyph_w, m.glyph_h);
	let columns = |buf: &[u8]| {
		[
			column_ink(buf, w, h, 0),
			column_ink(buf, w, h, w / 2),
			column_ink(buf, w, h, w - 1),
		]
	};
	let rows = |buf: &[u8]| {
		[
			row_ink(buf, w, 0),
			row_ink(buf, w, h / 2),
			row_ink(buf, w, h - 1),
		]
	};

	let (drawn, horizontal) = coverage(&mut r, '─', Style::Regular);
	assert!(drawn);
	eprintln!("─ колонки (0, середина, край): {:?}", columns(&horizontal));
	assert_uniform(&columns(&horizontal), 2, "─");

	let (drawn, vertical) = coverage(&mut r, '│', Style::Regular);
	assert!(drawn);
	eprintln!("│ строки (0, середина, край): {:?}", rows(&vertical));
	assert_uniform(&rows(&vertical), 2, "│");

	// У креста середина пересекает второй штрих — сравниваются только края, причём с краями
	// одиночных линий: стык `┼` с соседними `─` и `│` одинаково плотный.
	let (drawn, cross) = coverage(&mut r, '┼', Style::Regular);
	assert!(drawn);
	let cross_columns = [
		column_ink(&cross, w, h, 0),
		column_ink(&cross, w, h, w - 1),
		column_ink(&horizontal, w, h, 0),
	];
	let cross_rows = [
		row_ink(&cross, w, 0),
		row_ink(&cross, w, h - 1),
		row_ink(&vertical, w, 0),
	];
	eprintln!("┼ края: колонки {cross_columns:?}, строки {cross_rows:?}");
	assert_uniform(&cross_columns, 2, "┼ по горизонтали против ─");
	assert_uniform(&cross_rows, 2, "┼ по вертикали против │");
}

/// К2 (ADR-0021): прямые рамки в `Font` после выравнивания по сетке чёткие — только 0 и 255,
/// и хотя бы один штрих целой толщины (краска самой плотной колонки или строки ≥ 255).
#[test]
fn fitted_box_glyphs_are_pixel_aligned() {
	let mut r = repo_rasterizer();
	r.set_box_drawing_mode(BoxDrawingMode::Font);
	let m = r.metrics();
	let (w, h) = (m.glyph_w, m.glyph_h);
	for ch in ['─', '│', '┼', '═', '║', '╬', '┏', '┃', '╪'] {
		let (drawn, buf) = coverage(&mut r, ch, Style::Regular);
		assert!(drawn, "{ch} нарисован");
		assert!(
			buf.iter().all(|&v| v == 0 || v == 255),
			"{ch}: полутона после выравнивания: {:?}",
			buf.iter()
				.filter(|&&v| v != 0 && v != 255)
				.take(4)
				.collect::<Vec<_>>()
		);
		// У ╬ центральные колонка и строка попадают в зазор — ищется самый плотный штрих.
		let max_column = (0..w).map(|x| column_ink(&buf, w, h, x)).max().unwrap();
		let max_row = (0..h).map(|y| row_ink(&buf, w, y)).max().unwrap();
		assert!(
			max_column >= 255 || max_row >= 255,
			"{ch}: нет целого штриха: колонка {max_column}, строка {max_row}"
		);
	}
}
