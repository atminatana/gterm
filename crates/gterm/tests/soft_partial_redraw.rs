//! Частичная перерисовка Software (ADR-0010, ТЗ NFR-5, §8): только dirty-строки; полосы
//! перерисованного; кадр после частичных перерисовок равен кадру с нуля.

mod common;

use gterm::buffer::ScreenBuffer;
use gterm::cell::{CellFlags, Color};
use gterm::renderer::Renderer;
use gterm::renderer::soft_backend::{RowBand, SoftBackend};

use common::{RenderWith, repo_atlas};

const COLS: usize = 10;
const ROWS: usize = 5;
/// Строка, которую тесты меняют, чтобы получить частичный кадр.
const CHANGED_ROW: usize = 2;

fn background() -> Color {
	Color::from_rgb(0x12, 0x34, 0x56)
}

/// Все строки — обычный текст.
fn scene() -> ScreenBuffer {
	let mut screen = ScreenBuffer::new(COLS, ROWS);
	for row in 0..ROWS {
		screen.write_row(row, "row text!", Color::WHITE, Color::BLACK);
	}
	screen
}

/// Записать в строку текст с другими цветами и флагами: строка становится dirty.
fn change_row(screen: &mut ScreenBuffer, row: usize) {
	for (col, ch) in "CHANGED".chars().enumerate() {
		screen.set_cell(
			col,
			row,
			ch,
			Color::GREEN,
			Color::BLACK,
			CellFlags::UNDERLINE,
		);
	}
}

/// Рендерер окна `w × h`, уже нарисовавший первый (полный) кадр.
fn rendered(screen: &mut ScreenBuffer, w: u32, h: u32, view: &Color) -> SoftBackend {
	let mut soft = SoftBackend::new(repo_atlas());
	soft.resize(w, h).unwrap();
	soft.render_with(screen, view).unwrap();
	soft
}

/// Кадр того же экрана и вида, нарисованный новым рендерером с нуля.
fn from_scratch(screen: &ScreenBuffer, w: u32, h: u32, view: &Color) -> Vec<u32> {
	let mut copy = ScreenBuffer::new(screen.cols(), screen.rows());
	for row in 0..screen.rows() {
		for col in 0..screen.cols() {
			let c = screen.cell(col, row);
			copy.set_cell(col, row, c.ch, c.fg, c.bg, c.flags);
		}
	}
	rendered(&mut copy, w, h, view).frame().to_vec()
}

fn cell_size(soft: &SoftBackend) -> (u32, u32) {
	(soft.atlas().glyph_w() as u32, soft.atlas().glyph_h() as u32)
}

/// Полоса строк `first..first + count` сетки, вписанной в окно `w × h` по центру.
fn band(soft: &SoftBackend, w: u32, h: u32, first: usize, count: usize) -> RowBand {
	let (cw, ch) = cell_size(soft);
	let (grid_w, grid_h) = (COLS as u32 * cw, ROWS as u32 * ch);
	let (ox, oy) = (w.saturating_sub(grid_w) / 2, h.saturating_sub(grid_h) / 2);
	let y = oy + first as u32 * ch;
	RowBand {
		x: ox,
		y,
		width: grid_w.min(w - ox),
		height: (count as u32 * ch).min(h - y),
	}
}

#[test]
fn first_frame_and_resize_damage_the_whole_frame() {
	let mut screen = scene();
	let mut soft = rendered(&mut screen, 200, 150, &background());
	let whole = RowBand {
		x: 0,
		y: 0,
		width: 200,
		height: 150,
	};
	assert_eq!(soft.damage(), [whole]);
	soft.resize(210, 160).unwrap();
	soft.render_with(&mut screen, &background()).unwrap();
	assert_eq!(soft.damage()[0].width, 210);
	assert_eq!(soft.damage().len(), 1);
}

#[test]
fn unchanged_frame_has_no_damage() {
	let mut screen = scene();
	let mut soft = rendered(&mut screen, 200, 150, &background());
	let before = soft.frame().to_vec();
	soft.render_with(&mut screen, &background()).unwrap();
	assert!(soft.damage().is_empty());
	assert_eq!(soft.frame(), before);
}

#[test]
fn dirty_row_redraws_only_that_row() {
	let (w, h) = (200, 150);
	let mut screen = scene();
	let mut soft = rendered(&mut screen, w, h, &background());
	change_row(&mut screen, CHANGED_ROW);
	soft.render_with(&mut screen, &background()).unwrap();
	assert_eq!(soft.damage(), [band(&soft, w, h, CHANGED_ROW, 1)]);
	assert_eq!(soft.frame(), from_scratch(&screen, w, h, &background()));
}

#[test]
fn adjacent_dirty_rows_merge_into_bands() {
	let (w, h) = (200, 150);
	let mut screen = scene();
	let mut soft = rendered(&mut screen, w, h, &background());
	// Строки 1 и 2 изменились — одна полоса из двух строк.
	screen.write_row(1, "changed", Color::CYAN, Color::BLACK);
	change_row(&mut screen, 2);
	soft.render_with(&mut screen, &background()).unwrap();
	assert_eq!(soft.damage(), [band(&soft, w, h, 1, 2)]);
	// Строки 0 и 4 изменились — две полосы.
	screen.write_row(0, "zero", Color::CYAN, Color::BLACK);
	screen.write_row(4, "four", Color::CYAN, Color::BLACK);
	soft.render_with(&mut screen, &background()).unwrap();
	assert_eq!(
		soft.damage(),
		[band(&soft, w, h, 0, 1), band(&soft, w, h, 4, 1)]
	);
	assert_eq!(soft.frame(), from_scratch(&screen, w, h, &background()));
}

#[test]
fn background_change_redraws_the_whole_frame() {
	let mut screen = scene();
	let mut soft = rendered(&mut screen, 200, 150, &background());
	let other = Color::from_rgb(0x40, 0, 0x40);
	soft.render_with(&mut screen, &other).unwrap();
	assert_eq!(soft.damage().len(), 1);
	assert_eq!(
		(soft.damage()[0].width, soft.damage()[0].height),
		(200, 150)
	);
	assert_eq!(soft.frame(), from_scratch(&screen, 200, 150, &other));
}

/// Смена фона буфера при прозрачных ячейках (ADR-0013): строки не dirty, но ячейки обязаны
/// перерисоваться поверх новой подложки — и поле вокруг сетки тоже.
#[test]
fn background_change_shows_through_transparent_cells() {
	let (w, h) = (200, 150);
	let mut screen = scene();
	screen.fill_row(
		CHANGED_ROW,
		Color::WHITE,
		Color::from_rgba(0xA0, 0xB0, 0xC0, 0),
	);
	let mut soft = rendered(&mut screen, w, h, &background());
	let other = Color::from_rgb(0x40, 0, 0x40);
	soft.render_with(&mut screen, &other).unwrap();
	assert_eq!(soft.frame(), from_scratch(&screen, w, h, &other));
	let (_, ch) = cell_size(&soft);
	let row_band = band(&soft, w, h, CHANGED_ROW, 1);
	let inside = soft.frame()[((row_band.y + ch / 2) * w + row_band.x) as usize];
	assert_eq!(inside, 0x40_0040, "прозрачная ячейка показывает новый фон");
	assert_eq!(soft.frame()[0], 0x40_0040, "поле вокруг сетки — новый фон");
}

#[test]
fn bands_are_clipped_by_a_window_smaller_than_the_grid() {
	let mut screen = scene();
	let probe = rendered(&mut screen, 1, 1, &background());
	let (cw, ch) = cell_size(&probe);
	// Окно на полторы колонки уже сетки и обрезает изменённую строку посередине.
	let (w, h) = (
		COLS as u32 * cw - cw - cw / 2,
		CHANGED_ROW as u32 * ch + ch / 2,
	);
	let mut soft = rendered(&mut screen, w, h, &background());
	change_row(&mut screen, CHANGED_ROW);
	soft.render_with(&mut screen, &background()).unwrap();
	let damage = soft.damage();
	assert_eq!(damage.len(), 1);
	assert_eq!((damage[0].x, damage[0].width), (0, w));
	assert_eq!(damage[0].y + damage[0].height, h);
	assert_eq!(soft.frame(), from_scratch(&screen, w, h, &background()));
}

#[test]
fn row_below_the_window_gives_no_band() {
	let mut screen = scene();
	let probe = rendered(&mut screen, 1, 1, &background());
	let (_, ch) = cell_size(&probe);
	// Видны только строки 0 и 1: изменённая строка 2 целиком за окном.
	let (w, h) = (300, 2 * ch);
	let mut soft = rendered(&mut screen, w, h, &background());
	change_row(&mut screen, CHANGED_ROW);
	soft.render_with(&mut screen, &background()).unwrap();
	assert!(soft.damage().is_empty());
}

#[test]
fn long_sequence_of_partial_frames_equals_frame_from_scratch() {
	let (w, h) = (217, 163); // окно больше сетки: сетка по центру, нечётные поля
	let mut screen = scene();
	let mut soft = rendered(&mut screen, w, h, &background());
	for step in 0..12usize {
		let row = step % ROWS;
		match step % 3 {
			0 => change_row(&mut screen, row),
			1 => screen.write_row(row, &format!("step {step}"), Color::CYAN, Color::BLACK),
			_ => screen.set_cell(
				step % COLS,
				row,
				'*',
				Color::GREEN,
				Color::BLACK,
				CellFlags::BOLD | CellFlags::ITALIC,
			),
		}
		soft.render_with(&mut screen, &background()).unwrap();
		assert_eq!(
			soft.frame(),
			from_scratch(&screen, w, h, &background()),
			"step {step}"
		);
	}
}
