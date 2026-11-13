//! Этап 3 (ТЗ §8, §11.2): Software-рендерер — алгоритм §7.3 по пикселям (покрытие, начертания) и golden-тест.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use gterm::atlas::{Atlas, EMPTY_SLOT, PAGE_PX_MIN};
use gterm::buffer::ScreenBuffer;
use gterm::cell::{CellFlags, Color, Style};
use gterm::font::FontFamily;
use gterm::rasterizer::Rasterizer;
use gterm::renderer::Renderer;
use gterm::renderer::pixel::blend;
use gterm::renderer::soft_backend::SoftBackend;

use common::RenderWith;

const BORDER: Color = Color::from_rgb(0x12, 0x34, 0x56);
const FG: Color = Color::from_rgb(0x10, 0x20, 0x30);
const BG: Color = Color::from_rgb(0xA0, 0xB0, 0xC0);
const FULL: u8 = 255;
/// Переменная окружения, по которой golden-эталон перезаписывается текущим кадром.
const UPDATE_GOLDEN_ENV: &str = "GTERM_UPDATE_GOLDEN";
/// Окно golden: вмещает сетку 24×6 при ячейке до 10×23 с полями под рамку.
const GOLDEN_W: u32 = 250;
const GOLDEN_H: u32 = 140;

fn repo_rasterizer() -> Rasterizer {
	Rasterizer::new(
		FontFamily::load(
			&crate::common::repo_fonts_dir(),
			crate::common::REPO_FONT_FAMILY,
		)
		.unwrap(),
	)
	.unwrap()
}

fn repo_atlas() -> Atlas {
	// Минимальная страница: тесты создают много атласов, 16 МБ на каждый ни к чему.
	Atlas::new(repo_rasterizer(), PAGE_PX_MIN)
}

fn rgb(color: Color) -> u32 {
	color.0 & 0x00FF_FFFF
}

fn background() -> Color {
	BORDER
}

/// Кадр, отрендеренный в точный размер сетки.
struct Rendered {
	backend: SoftBackend,
}

impl Rendered {
	fn new(screen: &mut ScreenBuffer, view: &Color) -> Self {
		let mut backend = SoftBackend::new(repo_atlas());
		backend.render_with(screen, view).unwrap();
		Self { backend }
	}

	/// Кадр при другом физическом размере растеризации (ADR-0005): ячейка больше, окно — сетка.
	fn new_at(screen: &mut ScreenBuffer, view: &Color, size_px: u16) -> Self {
		let family = FontFamily::load(
			&crate::common::repo_fonts_dir(),
			crate::common::REPO_FONT_FAMILY,
		)
		.unwrap();
		let rasterizer = Rasterizer::new_at(family, size_px).unwrap();
		let mut backend = SoftBackend::new(Atlas::new(rasterizer, PAGE_PX_MIN));
		backend.render_with(screen, view).unwrap();
		Self { backend }
	}

	fn with_window(screen: &mut ScreenBuffer, view: &Color, w: u32, h: u32) -> Self {
		let mut backend = SoftBackend::new(repo_atlas());
		backend.resize(w, h).unwrap();
		backend.render_with(screen, view).unwrap();
		Self { backend }
	}

	fn px(&self, x: u32, y: u32) -> u32 {
		self.backend.frame()[(y * self.backend.width() + x) as usize]
	}

	fn atlas(&self) -> &Atlas {
		self.backend.atlas()
	}

	/// Пиксель `(gx, gy)` внутри ячейки `(col, row)` при масштабе 1 и origin 0.
	fn cell_px(&self, col: u32, row: u32, gx: u32, gy: u32) -> u32 {
		let a = self.atlas();
		self.px(col * a.glyph_w() as u32 + gx, row * a.glyph_h() as u32 + gy)
	}
}

/// Первый пиксель глифа (уже загруженного в атлас) с **полным** покрытием.
fn first_full_ink(atlas: &Atlas, cp: char, style: Style) -> (u32, u32) {
	let slot = atlas.slot_of(cp).expect("glyph must be loaded");
	assert_ne!(slot, EMPTY_SLOT, "glyph for {cp:?} must exist");
	for y in 0..atlas.glyph_h() {
		for x in 0..atlas.glyph_w() {
			if atlas.pixel(slot, x, y, style) == FULL {
				return (x as u32, y as u32);
			}
		}
	}
	panic!("glyph for {cp:?} has no fully covered pixel");
}

/// Первый пиксель глифа с **частичным** покрытием.
fn first_partial_ink(atlas: &Atlas, cp: char, style: Style) -> (u32, u32, u8) {
	let slot = atlas.slot_of(cp).expect("glyph must be loaded");
	for y in 0..atlas.glyph_h() {
		for x in 0..atlas.glyph_w() {
			let a = atlas.pixel(slot, x, y, style);
			if a > 0 && a < FULL {
				return (x as u32, y as u32, a);
			}
		}
	}
	panic!("glyph for {cp:?} has no partially covered pixel");
}

fn single_cell(ch: char, flags: CellFlags) -> ScreenBuffer {
	let mut screen = ScreenBuffer::new(3, 2);
	screen.set_cell(1, 1, ch, FG, BG, flags);
	screen
}

/// Альфа fg и bg и фон буфера (ADR-0013). Ожидаемые значения посчитаны вручную по
/// `(x·(255 − a) + y·a + 127) / 255` для фона буфера `BORDER` = (0x12, 0x34, 0x56),
/// `BG` = (0xA0, 0xB0, 0xC0), `FG` = (0x10, 0x20, 0x30); при альфе 128 покрытие 255 → 128.
mod alpha {
	use super::*;
	use gterm::renderer::pixel::fade;

	fn cell(ch: char, fg: Color, bg: Color, flags: CellFlags) -> Rendered {
		let mut screen = ScreenBuffer::new(3, 2);
		screen.set_cell(1, 1, ch, fg, bg, flags);
		Rendered::new(&mut screen, &background())
	}

	fn with_alpha(c: Color, a: u8) -> Color {
		Color::from_rgba(c.r(), c.g(), c.b(), a)
	}

	#[test]
	fn fade_scales_coverage_by_alpha() {
		assert_eq!(fade(200, 255), 200);
		assert_eq!(fade(200, 0), 0);
		assert_eq!(fade(255, 128), 128);
		assert_eq!(fade(100, 128), 50); // (12800 + 127) / 255 = 50
	}

	#[test]
	fn transparent_background_shows_the_buffer_background() {
		let r = cell(' ', FG, with_alpha(BG, 0), CellFlags::empty());
		assert_eq!(r.cell_px(1, 1, 0, 0), 0x12_3456);
	}

	#[test]
	fn half_transparent_background_blends_with_the_buffer_background() {
		// (0x12·127 + 0xA0·128 + 127)/255 = 0x59; 0x34 → 0x72; 0x56 → 0x8B.
		let r = cell(' ', FG, with_alpha(BG, 128), CellFlags::empty());
		assert_eq!(r.cell_px(1, 1, 0, 0), 0x59_728B);
	}

	#[test]
	fn transparent_glyph_is_not_drawn() {
		let r = cell('A', with_alpha(FG, 0), BG, CellFlags::empty());
		let (ix, iy) = first_full_ink(r.atlas(), 'A', Style::Regular);
		assert_eq!(r.cell_px(1, 1, ix, iy), rgb(BG));
	}

	#[test]
	fn half_transparent_glyph_is_blended_by_coverage_times_alpha() {
		// Полное покрытие × 128 → 128: (0xA0·127 + 0x10·128 + 127)/255 = 0x58, …
		let r = cell('A', with_alpha(FG, 128), BG, CellFlags::empty());
		let (ix, iy) = first_full_ink(r.atlas(), 'A', Style::Regular);
		assert_eq!(r.cell_px(1, 1, ix, iy), 0x58_6878);
	}

	#[test]
	fn bold_lightens_rgb_and_keeps_the_glyph_alpha() {
		// fg + 0x55 = (0x65, 0x75, 0x85), альфа 128 остаётся: → (0x82, 0x92, 0xA2).
		let r = cell('A', with_alpha(FG, 128), BG, CellFlags::BOLD);
		let (ix, iy) = first_full_ink(r.atlas(), 'A', Style::Bold);
		assert_eq!(r.cell_px(1, 1, ix, iy), 0x82_92A2);
	}

	#[test]
	fn underline_takes_the_glyph_alpha() {
		let r = cell(' ', with_alpha(FG, 128), BG, CellFlags::UNDERLINE);
		let last = r.atlas().glyph_h() as u32 - 1;
		assert_eq!(r.cell_px(1, 1, 0, last), 0x58_6878);
		assert_eq!(r.cell_px(1, 1, 0, last - 1), rgb(BG));
	}

	#[test]
	fn both_alphas_compose_over_the_buffer_background() {
		// Фон ячейки прозрачен → подложка (0x12, 0x34, 0x56); подчёркивание fg × 128 поверх неё.
		let r = cell(
			' ',
			with_alpha(FG, 128),
			with_alpha(BG, 0),
			CellFlags::UNDERLINE,
		);
		let last = r.atlas().glyph_h() as u32 - 1;
		assert_eq!(r.cell_px(1, 1, 0, last), 0x11_2A43);
		assert_eq!(r.cell_px(1, 1, 0, 0), 0x12_3456);
	}
}

mod algorithm {
	use super::*;

	#[test]
	fn glyph_pixels_follow_atlas_coverage_and_background_is_always_drawn() {
		let mut screen = single_cell('A', CellFlags::empty());
		let r = Rendered::new(&mut screen, &background());
		let atlas = r.atlas();
		let slot = atlas.slot_of('A').unwrap();

		for gy in 0..atlas.glyph_h() as u32 {
			for gx in 0..atlas.glyph_w() as u32 {
				let a = atlas.pixel(slot, gx as usize, gy as usize, Style::Regular);
				assert_eq!(
					r.cell_px(1, 1, gx, gy),
					blend(rgb(BG), rgb(FG), a),
					"px ({gx},{gy})"
				);
			}
		}
		// Пустая ячейка (ch = 0) — фон буфера по умолчанию, не рамка.
		assert_eq!(r.cell_px(0, 0, 0, 0), rgb(Color::BLACK));
		assert_eq!(
			r.backend.frame().len(),
			atlas.glyph_w() * 3 * atlas.glyph_h() * 2
		);
	}

	#[test]
	fn blend_is_integer_exact_and_symmetric_at_ends() {
		assert_eq!(blend(0x00A0B0C0, 0x00102030, 0), 0x00A0B0C0);
		assert_eq!(blend(0x00A0B0C0, 0x00102030, 255), 0x00102030);
		// (0xA0·(255−128) + 0x10·128 + 127)/255 = (20320 + 2048 + 127)/255 = 88 = 0x58
		assert_eq!(blend(0x00A0B0C0, 0x00102030, 128), 0x00586878);
		assert_eq!(blend(0, 0x00FFFFFF, 1), 0x00010101);
		assert_eq!(blend(0, 0x00FFFFFF, 254), 0x00FEFEFE);
	}

	#[test]
	fn partial_coverage_pixels_are_blended_not_thresholded() {
		let mut screen = single_cell('g', CellFlags::empty());
		let r = Rendered::new(&mut screen, &background());
		let (x, y, a) = first_partial_ink(r.atlas(), 'g', Style::Regular);
		let px = r.cell_px(1, 1, x, y);
		assert_eq!(px, blend(rgb(BG), rgb(FG), a));
		assert_ne!(px, rgb(FG));
		assert_ne!(px, rgb(BG));
	}

	#[test]
	fn bold_and_italic_flags_select_atlas_channels() {
		let mut screen = ScreenBuffer::new(4, 1);
		screen.set_cell(0, 0, 'A', FG, BG, CellFlags::empty());
		screen.set_cell(1, 0, 'A', FG, BG, CellFlags::BOLD);
		screen.set_cell(2, 0, 'A', FG, BG, CellFlags::ITALIC);
		screen.set_cell(3, 0, 'A', FG, BG, CellFlags::BOLD | CellFlags::ITALIC);
		let r = Rendered::new(&mut screen, &background());
		let atlas = r.atlas();
		let slot = atlas.slot_of('A').unwrap();
		for style in Style::ALL {
			assert!(
				atlas.is_ready(slot, style),
				"{style:?} must be rasterized by render"
			);
		}
		// Каждая ячейка — пиксели своего канала.
		for (col, style) in Style::ALL.iter().enumerate() {
			for gy in 0..atlas.glyph_h() {
				for gx in 0..atlas.glyph_w() {
					let a = atlas.pixel(slot, gx, gy, *style);
					let fg = if style.is_bold() {
						0x0065_7585
					} else {
						rgb(FG)
					};
					assert_eq!(
						r.cell_px(col as u32, 0, gx as u32, gy as u32),
						blend(rgb(BG), fg, a),
						"{style:?} px ({gx},{gy})"
					);
				}
			}
		}
		// Начертания действительно разные.
		assert_ne!(
			r.atlas().glyph_bytes(slot, Style::Regular),
			r.atlas().glyph_bytes(slot, Style::Italic)
		);
		assert_ne!(
			r.atlas().glyph_bytes(slot, Style::Regular),
			r.atlas().glyph_bytes(slot, Style::Bold)
		);
	}

	#[test]
	fn frame_format_is_00rrggbb_everywhere() {
		let mut screen = single_cell('A', CellFlags::empty());
		let r = Rendered::with_window(&mut screen, &background(), 200, 100);
		assert!(r.backend.frame().iter().all(|&p| p >> 24 == 0));
	}

	#[test]
	fn underline_fills_last_glyph_row_with_fg() {
		let mut screen = single_cell(' ', CellFlags::UNDERLINE);
		let r = Rendered::new(&mut screen, &background());
		let a = r.atlas();
		let last = a.glyph_h() as u32 - 1;
		for gx in 0..a.glyph_w() as u32 {
			assert_eq!(r.cell_px(1, 1, gx, last), rgb(FG));
			assert_eq!(r.cell_px(1, 1, gx, last - 1), rgb(BG));
		}
	}

	#[test]
	fn bold_lightens_fg_by_0x55_per_channel_with_saturation() {
		let mut screen = ScreenBuffer::new(2, 1);
		screen.set_cell(0, 0, 'A', FG, BG, CellFlags::BOLD);
		screen.set_cell(
			1,
			0,
			'A',
			Color::from_rgb(0xF0, 0x00, 0xFF),
			BG,
			CellFlags::BOLD,
		);
		let r = Rendered::new(&mut screen, &background());
		let (ix, iy) = first_full_ink(r.atlas(), 'A', Style::Bold);

		assert_eq!(r.cell_px(0, 0, ix, iy), 0x0065_7585);
		assert_eq!(r.cell_px(1, 0, ix, iy), 0x00FF_55FF);
	}

	#[test]
	fn cell_size_is_the_rasterization_metric_and_nothing_is_scaled() {
		// ADR-0005: ячейка = метрики растеризации, блока увеличения нет (FR-13). Размер шрифта
		// вместо масштаба — единственный способ сделать текст крупнее.
		let mut screen = single_cell('A', CellFlags::empty());
		let mut prev = (0usize, 0usize);
		for size in [16u16, 24, 32, 48] {
			let r = Rendered::new_at(&mut screen, &background(), size);
			let a = r.atlas();
			assert_eq!(r.backend.width(), 3 * a.glyph_w() as u32, "size {size}");
			assert_eq!(r.backend.height(), 2 * a.glyph_h() as u32, "size {size}");
			assert_eq!(
				r.backend.frame().len(),
				3 * a.glyph_w() * 2 * a.glyph_h(),
				"size {size}"
			);
			assert!(
				(a.glyph_w(), a.glyph_h()) > prev,
				"size {size}: cell must grow"
			);
			prev = (a.glyph_w(), a.glyph_h());
		}
	}

	#[test]
	fn grid_is_centered_and_leftover_is_border() {
		let mut screen = single_cell('A', CellFlags::empty());
		let a = repo_atlas();
		let (gw, gh) = (3 * a.glyph_w() as u32, 2 * a.glyph_h() as u32);
		let (w, h) = (gw + 21, gh + 10);
		let r = Rendered::with_window(&mut screen, &background(), w, h);
		let (ox, oy) = (21 / 2, 10 / 2);

		assert_eq!(r.px(0, 0), rgb(BORDER));
		assert_eq!(r.px(w - 1, h - 1), rgb(BORDER));
		assert_eq!(r.px(ox - 1, oy), rgb(BORDER));
		assert_eq!(r.px(ox, oy), rgb(Color::BLACK)); // ячейка (0,0): фон чёрный
		assert_eq!(r.px(ox + gw, oy), rgb(BORDER));
		assert_eq!(r.px(ox + gw - 1, oy + gh - 1), rgb(Color::BLACK)); // ячейка (2,1) пустая
	}

	#[test]
	fn window_smaller_than_grid_clips_without_panic() {
		// FR-23 `FixedGrid`: сетка больше окна центрируется и обрезается по границе окна.
		let mut screen = single_cell('A', CellFlags::empty());
		let r = Rendered::with_window(&mut screen, &background(), 7, 5);
		assert_eq!(r.backend.frame().len(), 35);
		assert_eq!(r.px(0, 0), rgb(Color::BLACK));
	}

	#[test]
	fn one_pixel_window_renders_without_panic() {
		// NFR-7: крайние размеры не приводят к панике и не выходят за кадр.
		let mut screen = single_cell('A', CellFlags::empty());
		let r = Rendered::with_window(&mut screen, &background(), 1, 1);
		assert_eq!(r.backend.frame(), &[rgb(Color::BLACK)]);
	}
}

mod dirty_rows {
	use super::*;

	#[test]
	fn second_render_redraws_only_dirty_rows_and_keeps_the_rest() {
		let mut screen = ScreenBuffer::new(4, 3);
		screen.write_row(0, "AAAA", FG, BG);
		screen.write_row(2, "BBBB", FG, BG);
		let mut backend = SoftBackend::new(repo_atlas());
		let v = background();
		backend.render_with(&mut screen, &v).unwrap();
		let first = backend.frame().to_vec();
		assert!(screen.dirty_rows().iter().all(|&d| !d));

		// Меняем только строку 1: строки 0 и 2 должны остаться как были, а не стереться рамкой.
		screen.write_row(1, "CCCC", FG, BG);
		backend.render_with(&mut screen, &v).unwrap();
		let second = backend.frame();
		let a = backend.atlas();
		let row_px = a.glyph_h() * a.glyph_w() * 4;

		assert_eq!(&second[..row_px], &first[..row_px]);
		assert_eq!(&second[2 * row_px..], &first[2 * row_px..]);
		assert_ne!(&second[row_px..2 * row_px], &first[row_px..2 * row_px]);
		let (ix, iy) = first_full_ink(a, 'C', Style::Regular);
		assert_eq!(
			second[(a.glyph_h() as u32 + iy) as usize * a.glyph_w() * 4 + ix as usize],
			rgb(FG)
		);
	}

	#[test]
	fn view_change_forces_full_redraw() {
		let mut screen = ScreenBuffer::new(2, 1);
		screen.write_row(0, "AB", FG, BG);
		let mut backend = SoftBackend::new(repo_atlas());
		backend.render_with(&mut screen, &background()).unwrap();

		let v = Color::WHITE;
		backend.resize(100, 50).unwrap();
		backend.render_with(&mut screen, &v).unwrap();
		assert_eq!(backend.frame()[0], rgb(Color::WHITE));
		assert_eq!(backend.frame().len(), 5000);
	}

	#[test]
	fn atlas_reset_mid_frame_still_yields_the_same_frame() {
		// Страница на 36 слотов, 30 заняты чужими глифами: экран из 20 разных глифов
		// переполняет её посреди подготовки кадра, после сброса помещается целиком.
		let mut small_atlas = Atlas::new(repo_rasterizer(), 96);
		assert_eq!(small_atlas.capacity(), 36);
		for ch in ('\u{0400}'..).take(30) {
			small_atlas.ensure(ch, Style::Regular);
		}
		assert_eq!(small_atlas.resets(), 0);
		let mut screen = ScreenBuffer::new(10, 2);
		screen.write_row(0, "abcdefghij", FG, BG);
		screen.write_row(1, "klmnopqrst", FG, BG);
		let mut small = SoftBackend::new(small_atlas);
		small.render_with(&mut screen, &background()).unwrap();
		assert_eq!(small.atlas().resets(), 1);

		let mut screen2 = ScreenBuffer::new(10, 2);
		screen2.write_row(0, "abcdefghij", FG, BG);
		screen2.write_row(1, "klmnopqrst", FG, BG);
		let big = Rendered::new(&mut screen2, &background());
		assert_eq!(small.frame(), big.backend.frame());
	}

	#[test]
	fn screen_larger_than_atlas_page_is_an_error_not_garbage() {
		let tiny = Atlas::new(repo_rasterizer(), 64); // 18 слотов
		assert!(tiny.capacity() < 20);
		let mut screen = ScreenBuffer::new(10, 2);
		screen.write_row(0, "abcdefghij", FG, BG);
		screen.write_row(1, "klmnopqrst", FG, BG);
		let mut backend = SoftBackend::new(tiny);
		assert!(backend.render_with(&mut screen, &background()).is_err());
	}
}

mod golden {
	use super::*;

	/// Компактная сцена со всеми ветками алгоритма: текст, флаги, начертания, цвета,
	/// псевдографика, пустые ячейки. Курсор рисует приложение (ADR-0004), поэтому в сцене его нет.
	fn golden_scene() -> ScreenBuffer {
		let mut screen = ScreenBuffer::new(24, 6);
		screen.write_row(
			0,
			"gterm golden Привет ─┼█",
			Color::WHITE,
			Color::from_rgb(0, 0, 0x80),
		);
		screen.write_row(1, "plain ░▒▓ ╔═╦╗ ▚", FG, BG);
		let flagged = [
			(2, "Bold", CellFlags::BOLD),
			(
				3,
				"UNDERLINE Italic",
				CellFlags::UNDERLINE | CellFlags::ITALIC,
			),
			(4, "BOLD UNDERLINE", CellFlags::BOLD | CellFlags::UNDERLINE),
		];
		for (row, text, flags) in flagged {
			for (i, ch) in text.chars().enumerate() {
				screen.set_cell(i, row, ch, Color::GREEN, Color::BLACK, flags);
			}
		}
		screen.write_row(5, "app cursor>", Color::CYAN, Color::BLACK);
		for (i, ch) in "BoldIt".chars().enumerate() {
			screen.set_cell(
				13 + i,
				5,
				ch,
				Color::CYAN,
				Color::BLACK,
				CellFlags::BOLD | CellFlags::ITALIC,
			);
		}
		screen
	}

	fn golden_background() -> Color {
		BORDER
	}

	/// Сцена «крупный шрифт» вместо прежней сцены «масштаб 2» (ADR-0005): та же сетка, но ячейка
	/// выведена из другого физического размера растеризации, окно больше сетки — центрирование.
	fn big_background() -> Color {
		Color::from_rgb(0x40, 0x00, 0x40)
	}

	fn golden_path(name: &str) -> PathBuf {
		Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/golden/{name}.bin"))
	}

	fn frame_bytes(frame: &[u32]) -> Vec<u8> {
		frame.iter().flat_map(|p| p.to_le_bytes()).collect()
	}

	fn assert_matches_or_update(name: &str, frame: &[u32]) {
		let actual = frame_bytes(frame);
		let path = golden_path(name);

		if std::env::var_os(UPDATE_GOLDEN_ENV).is_some() {
			fs::write(&path, &actual).unwrap();
			eprintln!(
				"golden updated: {} ({} bytes)",
				path.display(),
				actual.len()
			);
			return;
		}
		let expected = fs::read(&path).unwrap_or_else(|e| {
			panic!(
				"golden {} not readable ({e}); regenerate with {UPDATE_GOLDEN_ENV}=1",
				path.display()
			)
		});
		assert_eq!(expected.len(), actual.len(), "golden size mismatch");
		let first_diff = expected.iter().zip(&actual).position(|(a, b)| a != b);
		assert!(
			first_diff.is_none(),
			"frame differs from golden at byte {} (pixel {})",
			first_diff.unwrap(),
			first_diff.unwrap() / 4
		);
	}

	/// Golden-тест §11.2: кадр Software-рендерера побайтно равен эталону из репозитория.
	/// Обновление эталона: `GTERM_UPDATE_GOLDEN=1 cargo test --test stage3_render golden`.
	#[test]
	fn software_frame_matches_golden_dump() {
		let r = Rendered::with_window(
			&mut golden_scene(),
			&golden_background(),
			GOLDEN_W,
			GOLDEN_H,
		);
		assert_matches_or_update("soft_golden_16px", r.backend.frame());
	}

	/// Тот же эталон для крупного шрифта: геометрия выводится из метрик (ADR-0005), поэтому
	/// картинка другая, а не растянутая.
	#[test]
	fn big_font_frame_matches_golden_dump() {
		let family = FontFamily::load(
			&crate::common::repo_fonts_dir(),
			crate::common::REPO_FONT_FAMILY,
		)
		.unwrap();
		let rasterizer = Rasterizer::new_at(family, 32).unwrap();
		let m = rasterizer.metrics();
		let w = 24 * m.glyph_w as u32 + 21;
		let h = 6 * m.glyph_h as u32 + 10;
		let mut backend = SoftBackend::new(Atlas::new(rasterizer, PAGE_PX_MIN));
		backend.resize(w, h).unwrap();
		backend
			.render_with(&mut golden_scene(), &big_background())
			.unwrap();
		assert_matches_or_update("soft_golden_32px", backend.frame());
	}

	#[test]
	fn rendering_the_same_scene_twice_is_bit_identical() {
		let a = Rendered::with_window(
			&mut golden_scene(),
			&golden_background(),
			GOLDEN_W,
			GOLDEN_H,
		);
		let b = Rendered::with_window(
			&mut golden_scene(),
			&golden_background(),
			GOLDEN_W,
			GOLDEN_H,
		);
		assert_eq!(a.backend.frame(), b.backend.frame());
	}
}
