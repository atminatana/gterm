//! Наложение клиентских буферов (ADR-0014): тип `Overlay`, формулы предумноженной альфы и кадр
//! Software с наложением.

mod common;

use gterm::buffer::ScreenBuffer;
use gterm::cell::{CellFlags, Color};
use gterm::overlay::Overlay;
use gterm::renderer::pixel::{composite, over};

/// Формулы `over` / `composite`. Ожидаемые значения посчитаны вручную по `mul(x, a) =
/// (x·a + 127)/255`: `t' = mul(c, α) + mul(t, 255 − α)`, `t_a' = α + mul(t_a, 255 − α)`,
/// `out = t + mul(F, 255 − t_a)`.
mod formulas {
	use super::*;

	#[test]
	fn half_transparent_pixel_over_empty_texture_is_premultiplied() {
		// (0xA0·128 + 127)/255 = 0x50, 0xB0 → 0x58, 0xC0 → 0x60; альфа 128.
		assert_eq!(over(0, 0xA0_B0C0, 128), 0x8050_5860);
	}

	#[test]
	fn second_layer_accumulates_over_the_first() {
		// r: mul(0x10, 64) + mul(0x50, 191) = 4 + 60 = 0x40; g: 8 + 66 = 0x4A; b: 12 + 72 = 0x54;
		// a: 64 + mul(128, 191) = 64 + 96 = 0xA0.
		assert_eq!(over(0x8050_5860, 0x10_2030, 64), 0xA040_4A54);
	}

	#[test]
	fn composite_puts_the_texture_over_the_frame() {
		// r: 0x50 + mul(0x12, 127) = 80 + 9 = 0x59; g: 88 + 26 = 0x72; b: 96 + 43 = 0x8B.
		assert_eq!(composite(0x12_3456, 0x8050_5860), 0x59_728B);
		// r: 0x40 + mul(0x12, 95) = 64 + 7 = 0x47; g: 74 + 19 = 0x5D; b: 84 + 32 = 0x74.
		assert_eq!(composite(0x12_3456, 0xA040_4A54), 0x47_5D74);
	}

	/// Свойства решения 3 плана: непрозрачный слой даёт ровно свой цвет, прозрачный ничего не
	/// меняет, пустая текстура не меняет кадр, переполнения нет (`t_rgb ≤ t_a`, `out ≤ 255`).
	#[test]
	fn properties_hold_for_every_alpha() {
		let colors = [0x00_0000, 0xFF_FFFF, 0x12_3456, 0xA0_B0C0, 0x01_FE80];
		let textures = [0, 0x8050_5860, 0xA040_4A54, 0xFF12_3456, 0x0100_0000];
		for alpha in 0..=255u8 {
			for &rgb in &colors {
				for &t in &textures {
					let out = over(t, rgb, alpha);
					let a = out >> 24;
					for shift in [16, 8, 0] {
						assert!((out >> shift) & 0xFF <= a, "{t:08X} + {rgb:06X}@{alpha}");
					}
					if alpha == 255 {
						assert_eq!(out, rgb | 0xFF00_0000);
					}
					if alpha == 0 {
						assert_eq!(out, t);
					}
					for &frame in &colors {
						let px = composite(frame, out);
						assert_eq!(px >> 24, 0, "кадр — 0x00RRGGBB");
					}
				}
			}
		}
		for &frame in &colors {
			assert_eq!(composite(frame, 0), frame);
		}
	}
}

mod overlay_type {
	use super::*;

	#[test]
	fn push_copies_cells_and_keeps_order() {
		let mut first = ScreenBuffer::transparent(2, 1);
		first.set_cell(0, 0, 'a', Color::WHITE, Color::BLACK, CellFlags::empty());
		let second = ScreenBuffer::new(1, 2);
		let mut overlay = Overlay::new();
		assert!(overlay.is_empty());
		overlay.push(&first, -1, 3);
		overlay.push(&second, 5, 0);
		// Буфер клиента можно менять сразу: запись — копия.
		first.set_cell(0, 0, 'b', Color::WHITE, Color::BLACK, CellFlags::empty());

		let layers: Vec<_> = overlay.layers().collect();
		assert_eq!(layers.len(), 2);
		assert_eq!(
			(layers[0].col, layers[0].row, layers[0].cols, layers[0].rows),
			(-1, 3, 2, 1)
		);
		assert_eq!(layers[0].cell(0, 0).ch, 'a');
		assert_eq!(layers[0].cell(1, 0).bg, Color::TRANSPARENT);
		assert_eq!((layers[1].cols, layers[1].rows), (1, 2));
	}

	#[test]
	fn repeated_frame_does_not_grow_memory() {
		let buffer = ScreenBuffer::transparent(40, 10);
		let mut overlay = Overlay::new();
		overlay.push(&buffer, 0, 0);
		overlay.push(&buffer, 3, 4);
		let capacity = overlay.capacity();
		for _ in 0..10 {
			overlay.clear();
			assert!(overlay.is_empty());
			overlay.push(&buffer, 0, 0);
			overlay.push(&buffer, 3, 4);
		}
		assert_eq!(overlay.capacity(), capacity);
	}

	/// Копия записей прошлого кадра в Software (ADR-0015) не выделяет память, если приёмнику её
	/// хватает.
	#[test]
	fn clone_from_reuses_memory_and_compares_equal() {
		let buffer = ScreenBuffer::transparent(40, 10);
		let mut source = Overlay::new();
		source.push(&buffer, 0, 0);
		source.push(&buffer, 3, 4);
		let mut copy = Overlay::new();
		copy.clone_from(&source);
		assert_eq!(copy, source);
		let capacity = copy.capacity();
		let mut small = Overlay::new();
		small.push(&ScreenBuffer::transparent(2, 2), 1, 1);
		for _ in 0..5 {
			copy.clone_from(&small);
			assert_eq!(copy, small);
			copy.clone_from(&source);
			assert_eq!(copy, source);
		}
		assert_eq!(copy.capacity(), capacity);
	}

	#[test]
	fn records_differ_by_cell_position_and_order() {
		let a = ScreenBuffer::transparent(2, 1);
		let mut b = ScreenBuffer::transparent(2, 1);
		b.set_cell(
			1,
			0,
			'x',
			Color::WHITE,
			Color::TRANSPARENT,
			CellFlags::empty(),
		);
		let overlay = |layers: &[(&ScreenBuffer, i32)]| {
			let mut overlay = Overlay::new();
			for &(buffer, col) in layers {
				overlay.push(buffer, col, 0);
			}
			overlay
		};
		assert_eq!(overlay(&[(&a, 0), (&b, 1)]), overlay(&[(&a, 0), (&b, 1)]));
		assert_ne!(overlay(&[(&a, 0)]), overlay(&[(&b, 0)]), "ячейка");
		assert_ne!(overlay(&[(&a, 0)]), overlay(&[(&a, 1)]), "позиция");
		assert_ne!(
			overlay(&[(&a, 0), (&b, 0)]),
			overlay(&[(&b, 0), (&a, 0)]),
			"порядок"
		);
	}

	#[test]
	fn rows_and_cols_are_clipped_by_the_grid() {
		let mut overlay = Overlay::new();
		overlay.push(&ScreenBuffer::transparent(4, 3), -2, 8);
		overlay.push(&ScreenBuffer::transparent(2, 2), 20, -5);
		let layers: Vec<_> = overlay.layers().collect();
		assert_eq!(layers[0].grid_cols(10), 0..2);
		assert_eq!(layers[0].grid_rows(10), 8..10);
		assert!(layers[1].grid_cols(10).is_empty());
		assert!(layers[1].grid_rows(10).is_empty());
	}

	#[test]
	fn transparent_buffer_has_transparent_blank_cells() {
		let mut buffer = ScreenBuffer::transparent(2, 2);
		assert_eq!(buffer.cell(1, 1).bg, Color::TRANSPARENT);
		assert_eq!(buffer.cell(1, 1).fg, Color::TRANSPARENT);
		buffer.clear_all();
		assert_eq!(buffer.cell(0, 0).bg, Color::TRANSPARENT);
	}
}

/// Кадр Software с наложением (ADR-0014). Основной экран — 4×3 ячейки однотонного фона
/// `UNDER` = (0x12, 0x34, 0x56); ожидаемые значения — из модуля `formulas`.
mod software {
	use super::*;
	use common::repo_atlas;
	use gterm::atlas::{Atlas, PAGE_PX_MIN};
	use gterm::cell::Style;
	use gterm::renderer::Renderer;
	use gterm::renderer::soft_backend::{RowBand, SoftBackend};

	const UNDER: Color = Color::from_rgb(0x12, 0x34, 0x56);
	const COLS: usize = 4;
	const ROWS: usize = 3;

	fn main_screen() -> ScreenBuffer {
		let mut screen = ScreenBuffer::new(COLS, ROWS);
		for row in 0..ROWS {
			screen.fill_row(row, Color::WHITE, UNDER);
		}
		screen
	}

	/// Прозрачный буфер 1×1 с одной ячейкой.
	fn one_cell(ch: char, fg: Color, bg: Color) -> ScreenBuffer {
		let mut buffer = ScreenBuffer::transparent(1, 1);
		buffer.set_cell(0, 0, ch, fg, bg, CellFlags::empty());
		buffer
	}

	fn rendered(screen: &mut ScreenBuffer, overlay: &Overlay) -> SoftBackend {
		let mut soft = SoftBackend::new(repo_atlas());
		soft.render_overlaid(screen, overlay).unwrap();
		soft
	}

	/// Пиксель `(gx, gy)` ячейки `(col, row)` — кадр ровно по сетке.
	fn px(soft: &SoftBackend, col: usize, row: usize, gx: usize, gy: usize) -> u32 {
		let (cw, ch) = (soft.atlas().glyph_w(), soft.atlas().glyph_h());
		soft.frame()[(row * ch + gy) * soft.width() as usize + col * cw + gx]
	}

	fn cell_color(soft: &SoftBackend, col: usize, row: usize) -> u32 {
		px(soft, col, row, 0, 0)
	}

	#[test]
	fn half_transparent_cell_blends_over_the_screen() {
		let mut overlay = Overlay::new();
		let bg = Color::from_rgba(0xA0, 0xB0, 0xC0, 128);
		overlay.push(&one_cell(' ', Color::TRANSPARENT, bg), 1, 1);
		let soft = rendered(&mut main_screen(), &overlay);
		assert_eq!(cell_color(&soft, 1, 1), 0x59_728B);
		assert_eq!(
			cell_color(&soft, 0, 1),
			0x12_3456,
			"соседняя ячейка не тронута"
		);
	}

	#[test]
	fn opaque_cell_replaces_the_screen_exactly() {
		let mut overlay = Overlay::new();
		let bg = Color::from_rgb(1, 2, 3);
		overlay.push(&one_cell(' ', Color::TRANSPARENT, bg), 2, 0);
		let soft = rendered(&mut main_screen(), &overlay);
		assert_eq!(cell_color(&soft, 2, 0), 0x01_0203);
	}

	#[test]
	fn later_buffer_lies_above_the_earlier_one() {
		let mut overlay = Overlay::new();
		let lower = Color::from_rgba(0xA0, 0xB0, 0xC0, 128);
		let upper = Color::from_rgba(0x10, 0x20, 0x30, 64);
		overlay.push(&one_cell(' ', Color::TRANSPARENT, lower), 1, 1);
		overlay.push(&one_cell(' ', Color::TRANSPARENT, upper), 1, 1);
		let soft = rendered(&mut main_screen(), &overlay);
		assert_eq!(cell_color(&soft, 1, 1), 0x47_5D74);
	}

	#[test]
	fn half_transparent_glyph_blends_by_coverage() {
		let mut overlay = Overlay::new();
		let fg = Color::from_rgba(0x10, 0x20, 0x30, 128);
		overlay.push(&one_cell('A', fg, Color::TRANSPARENT), 1, 1);
		let soft = rendered(&mut main_screen(), &overlay);
		let atlas = soft.atlas();
		let slot = atlas.slot_of('A').unwrap();
		let pixels =
			|| (0..atlas.glyph_h()).flat_map(|y| (0..atlas.glyph_w()).map(move |x| (x, y)));
		let coverage = |(x, y): (usize, usize)| atlas.pixel(slot, x, y, Style::Regular);
		let full = pixels()
			.find(|&p| coverage(p) == 255)
			.expect("полное покрытие");
		assert_eq!(px(&soft, 1, 1, full.0, full.1), 0x11_2A43);
		let empty = pixels()
			.find(|&p| coverage(p) == 0)
			.expect("пустой пиксель");
		assert_eq!(px(&soft, 1, 1, empty.0, empty.1), 0x12_3456);
	}

	#[test]
	fn negative_position_and_overflow_are_clipped_by_the_grid() {
		let x = Color::from_rgb(0x0A, 0x0B, 0x0C);
		let mut overlay = Overlay::new();
		overlay.push(&ScreenBuffer::with_default_colors(3, 2, x, x), -1, -1);
		overlay.push(&ScreenBuffer::with_default_colors(3, 3, x, x), 3, 2);
		let soft = rendered(&mut main_screen(), &overlay);
		for (col, row, covered) in [
			(0, 0, true),
			(1, 0, true),
			(2, 0, false),
			(0, 1, false),
			(3, 2, true),
			(2, 2, false),
		] {
			let expected = if covered { 0x0A_0B0C } else { 0x12_3456 };
			assert_eq!(cell_color(&soft, col, row), expected, "({col}, {row})");
		}
	}

	#[test]
	fn empty_buffer_does_not_change_the_frame() {
		let plain = rendered(&mut main_screen(), &Overlay::new());
		let mut overlay = Overlay::new();
		overlay.push(&ScreenBuffer::transparent(COLS, ROWS), 0, 0);
		let with_empty = rendered(&mut main_screen(), &overlay);
		assert_eq!(plain.frame(), with_empty.frame());
	}

	/// Полоса строк `first..first + count` кадра ровно по сетке.
	fn band(soft: &SoftBackend, first: usize, count: usize) -> RowBand {
		let ch = soft.atlas().glyph_h() as u32;
		RowBand {
			x: 0,
			y: first as u32 * ch,
			width: soft.width(),
			height: count as u32 * ch,
		}
	}

	#[test]
	fn moving_and_removing_the_overlay_redraws_only_touched_rows() {
		let red = Color::from_rgba(0xA0, 0, 0, 160);
		let marker = one_cell(' ', Color::TRANSPARENT, red);
		let mut screen = main_screen();
		let mut soft = SoftBackend::new(repo_atlas());
		let mut overlay = Overlay::new();
		overlay.push(&marker, 1, 0);
		soft.render_overlaid(&mut screen, &overlay).unwrap();

		// Сдвиг на строку 1: перерисованы строки 0 (была) и 1 (стала), строка 2 — нет.
		overlay.clear();
		overlay.push(&marker, 2, 1);
		soft.render_overlaid(&mut screen, &overlay).unwrap();
		assert_eq!(soft.damage(), [band(&soft, 0, 2)]);
		assert_eq!(soft.frame(), rendered(&mut main_screen(), &overlay).frame());

		// Наложение исчезло: перерисована только строка 1.
		overlay.clear();
		soft.render_overlaid(&mut screen, &overlay).unwrap();
		assert_eq!(soft.damage(), [band(&soft, 1, 1)]);
		assert_eq!(soft.frame(), rendered(&mut main_screen(), &overlay).frame());

		// Без наложения и изменений — ничего не перерисовано.
		soft.render_overlaid(&mut screen, &overlay).unwrap();
		assert!(soft.damage().is_empty());
	}

	#[test]
	fn unchanged_overlay_keeps_the_frame_identical() {
		let yellow = Color::from_rgba(0xFF, 0xFF, 0, 200);
		let marker = one_cell('Ж', yellow, Color::TRANSPARENT);
		let mut screen = main_screen();
		let mut soft = SoftBackend::new(repo_atlas());
		let mut overlay = Overlay::new();
		overlay.push(&marker, 0, 2);
		soft.render_overlaid(&mut screen, &overlay).unwrap();
		let first = soft.frame().to_vec();
		for _ in 0..3 {
			soft.render_overlaid(&mut screen, &overlay).unwrap();
			assert_eq!(soft.damage(), [band(&soft, 2, 1)]);
			assert_eq!(soft.frame(), first, "наложение не копится от кадра к кадру");
		}
	}

	/// Кэш текстуры (ADR-0015): при любой смене сцены кадр равен кадру с нуля.
	fn glyph_layer() -> ScreenBuffer {
		let mut layer = ScreenBuffer::transparent(2, 2);
		let fg = Color::from_rgba(0xFF, 0x80, 0x20, 140);
		let bg = Color::from_rgba(0x20, 0x40, 0xC0, 90);
		layer.write_row(0, "Ж█", fg, bg);
		layer.write_row(1, "░A", fg, bg);
		layer
	}

	fn glyph_overlay(col: i32, row: i32) -> Overlay {
		let mut overlay = Overlay::new();
		overlay.push(&glyph_layer(), col, row);
		overlay
	}

	#[test]
	fn same_overlay_over_a_changing_screen_matches_a_fresh_frame() {
		let overlay = glyph_overlay(1, 0);
		let screen_with = |text: &str| {
			let mut screen = main_screen();
			screen.write_row(1, text, Color::WHITE, UNDER);
			screen
		};
		let mut screen = main_screen();
		let mut soft = SoftBackend::new(repo_atlas());
		soft.render_overlaid(&mut screen, &overlay).unwrap();
		for text in ["abcd", "efgh", "ijkl"] {
			screen.write_row(1, text, Color::WHITE, UNDER);
			soft.render_overlaid(&mut screen, &overlay).unwrap();
			assert_eq!(
				soft.frame(),
				rendered(&mut screen_with(text), &overlay).frame(),
				"{text}"
			);
		}
	}

	#[test]
	fn changed_cell_at_the_same_position_rebuilds_the_texture() {
		let mut screen = main_screen();
		let mut soft = SoftBackend::new(repo_atlas());
		let first = glyph_overlay(1, 1);
		let mut changed_layer = glyph_layer();
		changed_layer.set_cell(1, 1, 'B', Color::WHITE, Color::TRANSPARENT, CellFlags::BOLD);
		let mut changed = Overlay::new();
		changed.push(&changed_layer, 1, 1);
		for overlay in [&first, &changed, &first] {
			soft.render_overlaid(&mut screen, overlay).unwrap();
			assert_eq!(soft.frame(), rendered(&mut main_screen(), overlay).frame());
		}
	}

	#[test]
	fn overlay_returning_after_empty_frames_matches_a_fresh_frame() {
		let mut screen = main_screen();
		let mut soft = SoftBackend::new(repo_atlas());
		let shown = glyph_overlay(2, 1);
		let empty = Overlay::new();
		for overlay in [&shown, &empty, &empty, &shown, &shown] {
			soft.render_overlaid(&mut screen, overlay).unwrap();
			assert_eq!(soft.frame(), rendered(&mut main_screen(), overlay).frame());
		}
	}

	#[test]
	fn overlay_after_window_resize_matches_a_fresh_frame() {
		let overlay = glyph_overlay(0, 1);
		let mut screen = main_screen();
		let mut soft = SoftBackend::new(repo_atlas());
		soft.render_overlaid(&mut screen, &overlay).unwrap();
		let (w, h) = (soft.width() + 13, soft.height() + 7);
		soft.resize(w, h).unwrap();
		soft.render_overlaid(&mut screen, &overlay).unwrap();

		let mut fresh = SoftBackend::new(repo_atlas());
		fresh.resize(w, h).unwrap();
		fresh.render_overlaid(&mut main_screen(), &overlay).unwrap();
		assert_eq!(soft.frame(), fresh.frame());
	}

	#[test]
	fn overlay_after_grid_change_matches_a_fresh_frame() {
		let overlay = glyph_overlay(3, 1);
		let wider = || {
			let mut screen = ScreenBuffer::new(COLS + 2, ROWS);
			for row in 0..ROWS {
				screen.fill_row(row, Color::WHITE, UNDER);
			}
			screen
		};
		let mut soft = SoftBackend::new(repo_atlas());
		soft.render_overlaid(&mut main_screen(), &overlay).unwrap();
		let mut screen = wider();
		let (w, h) = soft.grid_size_px(&screen);
		soft.resize(w, h).unwrap();
		soft.render_overlaid(&mut screen, &overlay).unwrap();
		assert_eq!(soft.frame(), rendered(&mut wider(), &overlay).frame());
	}

	/// Смена DPI в `AutoGrid`: сетка пересчитывается под новую ячейку, и её размер в пикселях
	/// может совпасть с прежним — текстуру всё равно нужно пересобрать.
	#[test]
	fn overlay_after_font_size_change_with_the_same_grid_px_matches_a_fresh_frame() {
		const BIG_PX: u16 = 24;
		let filled = |cols: usize, rows: usize| {
			let mut screen = ScreenBuffer::new(cols, rows);
			for row in 0..rows {
				screen.fill_row(row, Color::WHITE, UNDER);
			}
			screen
		};
		let overlay = glyph_overlay(1, 1);
		let small = repo_atlas();
		let big = common::repo_atlas_at(BIG_PX);
		let (small_w, small_h) = (small.glyph_w(), small.glyph_h());
		let (big_w, big_h) = (big.glyph_w(), big.glyph_h());
		assert_ne!((small_w, small_h), (big_w, big_h), "размер ячейки сменился");

		let mut soft = SoftBackend::new(small);
		let mut before = filled(big_w, big_h);
		soft.render_overlaid(&mut before, &overlay).unwrap();
		let grid_px = soft.grid_size_px(&before);
		soft.set_size(BIG_PX).unwrap();
		let mut after = filled(small_w, small_h);
		assert_eq!(
			soft.grid_size_px(&after),
			grid_px,
			"размер сетки в пикселях тот же"
		);
		soft.render_overlaid(&mut after, &overlay).unwrap();

		let mut fresh = SoftBackend::new(big);
		fresh
			.render_overlaid(&mut filled(small_w, small_h), &overlay)
			.unwrap();
		assert_eq!(soft.frame(), fresh.frame());
	}

	#[test]
	fn atlas_reset_while_preparing_overlay_glyphs_yields_the_same_frame() {
		// Страница на 36 слотов, 30 заняты: 10 глифов экрана и 10 глифов наложения не помещаются,
		// после сброса помещаются все 20.
		let mut small_atlas = Atlas::new(common::repo_rasterizer(), 96);
		assert_eq!(small_atlas.capacity(), 36);
		for ch in ('\u{0400}'..).take(30) {
			small_atlas.ensure(ch, Style::Regular);
		}
		let orange = Color::from_rgba(0xFF, 0x80, 0, 180);
		let scene = || {
			let mut screen = ScreenBuffer::new(10, 1);
			screen.write_row(0, "abcdefghij", Color::WHITE, UNDER);
			let mut layer = ScreenBuffer::transparent(10, 1);
			layer.write_row(0, "klmnopqrst", orange, Color::TRANSPARENT);
			let mut overlay = Overlay::new();
			overlay.push(&layer, 0, 0);
			(screen, overlay)
		};
		let (mut screen, overlay) = scene();
		let mut small = SoftBackend::new(small_atlas);
		small.render_overlaid(&mut screen, &overlay).unwrap();
		assert_eq!(small.atlas().resets(), 1);

		let (mut screen2, overlay2) = scene();
		let mut big = SoftBackend::new(Atlas::new(common::repo_rasterizer(), PAGE_PX_MIN));
		big.render_overlaid(&mut screen2, &overlay2).unwrap();
		assert_eq!(small.frame(), big.frame());
	}
}

/// Окно больше сетки: сетка по центру, наложение попадает в свою ячейку, поле вокруг не трогает.
mod software_centered {
	use super::*;
	use common::repo_atlas;
	use gterm::renderer::Renderer;
	use gterm::renderer::soft_backend::SoftBackend;

	#[test]
	fn overlay_follows_the_grid_origin_and_spares_the_border() {
		let under = Color::from_rgb(0x12, 0x34, 0x56);
		let mut screen = ScreenBuffer::new(4, 3);
		for row in 0..3 {
			screen.fill_row(row, Color::WHITE, under);
		}
		screen.set_background(Color::from_rgb(0x40, 0, 0x40));
		let mut cover = ScreenBuffer::transparent(1, 1);
		cover.fill_row(0, Color::TRANSPARENT, Color::from_rgb(1, 2, 3));
		let mut overlay = Overlay::new();
		overlay.push(&cover, 3, 2);

		let mut soft = SoftBackend::new(repo_atlas());
		let (cw, ch) = (soft.atlas().glyph_w() as u32, soft.atlas().glyph_h() as u32);
		let (w, h) = (4 * cw + 17, 3 * ch + 9);
		soft.resize(w, h).unwrap();
		soft.render_overlaid(&mut screen, &overlay).unwrap();
		let (ox, oy) = ((w - 4 * cw) / 2, (h - 3 * ch) / 2);
		let at = |x: u32, y: u32| soft.frame()[(y * w + x) as usize];
		assert_eq!(at(ox + 3 * cw, oy + 2 * ch), 0x01_0203, "ячейка (3, 2)");
		assert_eq!(at(ox + 3 * cw - 1, oy + 2 * ch), 0x12_3456, "ячейка (2, 2)");
		assert_eq!(at(w - 1, h - 1), 0x40_0040, "поле вокруг сетки");
		assert_eq!(at(ox + 4 * cw, oy + 2 * ch), 0x40_0040, "справа от сетки");
	}
}
