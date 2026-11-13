//! Многопоточная отрисовка Software (ADR-0016): кадр при 4 потоках бит в бит равен кадру при одном
//! — на полных и частичных кадрах, с наложением, при окне больше и меньше сетки.

mod common;

use common::repo_atlas;
use gterm::buffer::ScreenBuffer;
use gterm::cell::{CellFlags, Color};
use gterm::overlay::Overlay;
use gterm::renderer::Renderer;
use gterm::renderer::soft_backend::SoftBackend;

const THREADS: usize = 4;
/// Строк больше порога параллельной отрисовки — полосы действительно раздаются потокам.
const COLS: usize = 30;
const ROWS: usize = 20;

/// Экран кадра `frame`: разные символы, цвета, альфа и флаги по строкам.
fn screen_for(frame: usize) -> ScreenBuffer {
	let mut screen = ScreenBuffer::new(COLS, ROWS);
	screen.set_background(Color::from_rgb(0x10, 0x18, 0x20));
	for row in 0..ROWS {
		let text: String = (0..COLS)
			.map(|col| char::from(b'!' + ((row * 7 + col + frame) % 90) as u8))
			.collect();
		let bg = Color::from_rgba(0x30, (row * 9) as u8, 0x60, 120 + (row * 6) as u8);
		screen.write_row(row, &text, Color::WHITE, bg);
		let flags = [CellFlags::BOLD, CellFlags::ITALIC, CellFlags::UNDERLINE][row % 3];
		screen.set_cell(row % COLS, row, '█', Color::GREEN, bg, flags);
	}
	screen
}

/// Наложение кадра `frame`: два полупрозрачных слоя, один сдвигается от кадра к кадру и выходит
/// за сетку.
fn overlay_for(frame: usize) -> Overlay {
	let mut lower = ScreenBuffer::transparent(12, 9);
	let mut upper = ScreenBuffer::transparent(8, 14);
	let fg = Color::from_rgba(0xFF, 0x80, 0x20, 140);
	let bg = Color::from_rgba(0x20, 0x40, 0xC0, 90);
	for row in 0..9 {
		lower.write_row(row, "Ж░█Aqj", fg, bg);
	}
	for row in 0..14 {
		upper.fill_row(row, Color::TRANSPARENT, Color::from_rgba(0xA0, 0, 0, 100));
	}
	let mut overlay = Overlay::new();
	overlay.push(&lower, 3, 2);
	overlay.push(&upper, 20 + frame as i32 * 3, -3 + frame as i32 * 2);
	overlay
}

fn backends(size: Option<(u32, u32)>) -> (SoftBackend, SoftBackend) {
	let mut one = SoftBackend::with_threads(repo_atlas(), 1);
	let mut many = SoftBackend::with_threads(repo_atlas(), THREADS);
	assert_eq!((one.threads(), many.threads()), (1, THREADS));
	if let Some((w, h)) = size {
		one.resize(w, h).unwrap();
		many.resize(w, h).unwrap();
	}
	(one, many)
}

/// Серия кадров: полный, частичные (строки меняются, наложение движется), без наложения.
fn assert_same_frames(size: Option<(u32, u32)>) {
	let (mut one, mut many) = backends(size);
	let (mut screen_one, mut screen_many) = (screen_for(0), screen_for(0));
	for frame in 0..6 {
		if frame > 0 {
			for row in (frame % 3..ROWS).step_by(3) {
				let changed = screen_for(frame);
				for col in 0..COLS {
					let cell = changed.cell(col, row);
					for screen in [&mut screen_one, &mut screen_many] {
						screen.set_cell(col, row, cell.ch, cell.fg, cell.bg, cell.flags);
					}
				}
			}
		}
		let overlay = if frame == 4 {
			Overlay::new()
		} else {
			overlay_for(frame)
		};
		one.render_overlaid(&mut screen_one, &overlay).unwrap();
		many.render_overlaid(&mut screen_many, &overlay).unwrap();
		assert_eq!(one.damage(), many.damage(), "кадр {frame}: полосы");
		assert!(one.frame() == many.frame(), "кадр {frame}: пиксели");
	}
}

#[test]
fn frames_match_one_thread_with_the_window_equal_to_the_grid() {
	assert_same_frames(None);
}

#[test]
fn frames_match_one_thread_with_the_window_larger_than_the_grid() {
	let atlas = repo_atlas();
	let (w, h) = (atlas.glyph_w() as u32, atlas.glyph_h() as u32);
	assert_same_frames(Some((COLS as u32 * w + 37, ROWS as u32 * h + 23)));
}

/// `FixedGrid` в маленьком окне: сетка обрезана справа и снизу, нижние полосы короче.
#[test]
fn frames_match_one_thread_with_the_window_smaller_than_the_grid() {
	let atlas = repo_atlas();
	let (w, h) = (atlas.glyph_w() as u32, atlas.glyph_h() as u32);
	assert_same_frames(Some((COLS as u32 * w - 13, ROWS as u32 * h / 2 + 5)));
}

#[test]
fn default_threads_are_capped() {
	let soft = SoftBackend::new(repo_atlas());
	assert!((1..=4).contains(&soft.threads()), "{}", soft.threads());
}
