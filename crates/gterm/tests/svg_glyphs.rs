//! SVG-глифы (ТЗ FR-8.6, ADR-0008): имена файлов, загрузка, вписывание с сохранением пропорций,
//! приоритет SVG > псевдографика > шрифт, все каналы атласа, смена размера на месте.

mod common;

use gterm::atlas::Atlas;
use gterm::box_drawing;
use gterm::cell::Style;
use gterm::svg::{SvgGlyphs, codepoint_from_file_name};

use common::{repo_rasterizer_at, svg_atlas_at, svg_rasterizer_at, test_svg_dir};

/// Ячейка 1 : 2 — квадратный холст 10×10 вписывается с полосами фона по 5 px сверху и снизу.
const W: usize = 10;
const H: usize = 20;
const FULL: u8 = 255;

const SQUARE: char = '\u{E000}';
const RING: char = '\u{E001}';
const RIGHT_HALF: char = '\u{F0041}';
const FRAME_EVEN_ODD: char = '\u{10FFFD}';
const CIRCLE_OVER_BOX: char = '\u{2500}';
const BROKEN: char = '\u{E002}';

fn test_glyphs() -> SvgGlyphs {
	SvgGlyphs::load(&test_svg_dir())
}

fn render(glyphs: &mut SvgGlyphs, ch: char, w: usize, h: usize) -> Vec<u8> {
	let mut out = vec![0u8; w * h];
	assert!(
		glyphs.rasterize(ch, w, h, &mut out),
		"U+{:04X}",
		u32::from(ch)
	);
	out
}

fn at(buf: &[u8], x: usize, y: usize) -> u8 {
	buf[y * W + x]
}

mod file_names {
	use super::*;

	#[test]
	fn hex_codepoint_of_four_to_six_digits_is_accepted() {
		assert_eq!(codepoint_from_file_name("E000.svg"), Some('\u{E000}'));
		assert_eq!(codepoint_from_file_name("e000.SVG"), Some('\u{E000}'));
		assert_eq!(codepoint_from_file_name("F0001.svg"), Some('\u{F0001}'));
		assert_eq!(codepoint_from_file_name("100000.svg"), Some('\u{100000}'));
		assert_eq!(codepoint_from_file_name("10FFFF.svg"), Some(char::MAX));
		assert_eq!(codepoint_from_file_name("0041.svg"), Some('A'));
	}

	#[test]
	fn anything_else_is_rejected() {
		for name in [
			"U+E000.svg",
			"uniE000.svg",
			"E000_tree.svg",
			"E00.svg",
			"1000000.svg",
			"110000.svg",
			"D800.svg",
			"E000.png",
			"E000",
			".svg",
			"",
			"ЖЖЖЖ.svg",
		] {
			assert_eq!(codepoint_from_file_name(name), None, "{name}");
		}
	}
}

mod loading {
	use super::*;

	#[test]
	fn valid_files_load_and_bad_ones_are_skipped_without_panic() {
		let glyphs = test_glyphs();
		// `tree.svg` — неверное имя, `E002.svg` — битый XML, `E000.svg` — повтор `0E000.svg`.
		assert_eq!(
			glyphs.codepoints().collect::<Vec<_>>(),
			[CIRCLE_OVER_BOX, SQUARE, RING, RIGHT_HALF, FRAME_EVEN_ODD]
		);
		assert!(!glyphs.contains(BROKEN));
		assert!(!glyphs.contains('A'));
	}

	#[test]
	fn missing_directory_gives_empty_set() {
		let glyphs = SvgGlyphs::load(&test_svg_dir().join("missing"));
		assert!(glyphs.is_empty());
	}

	#[test]
	fn first_file_name_wins_on_duplicate_codepoint() {
		// `0E000.svg` (полный квадрат) сортируется раньше `E000.svg` (пустой холст).
		let out = render(&mut test_glyphs(), SQUARE, W, H);
		assert_eq!(at(&out, 5, 10), FULL);
	}
}

mod rasterization {
	use super::*;

	#[test]
	fn square_canvas_keeps_proportions_and_is_centered() {
		let out = render(&mut test_glyphs(), SQUARE, W, H);
		for y in 0..H {
			let expected = if (5..15).contains(&y) { FULL } else { 0 };
			assert!(
				out[y * W..(y + 1) * W].iter().all(|&v| v == expected),
				"row {y}"
			);
		}
	}

	#[test]
	fn colors_are_ignored_and_fill_none_is_respected() {
		// Кольцо: обводка шириной 2 вокруг квадрата 1…9, заливки нет.
		let out = render(&mut test_glyphs(), RING, W, H);
		assert_eq!(at(&out, 0, 10), FULL, "left side of the stroke");
		assert_eq!(at(&out, 5, 5), FULL, "top side of the stroke");
		assert_eq!(at(&out, 5, 10), 0, "fill=none leaves the center empty");
		assert_eq!(at(&out, 5, 2), 0, "outside the canvas");
	}

	#[test]
	fn group_transform_is_applied() {
		let out = render(&mut test_glyphs(), RIGHT_HALF, W, H);
		assert_eq!(at(&out, 7, 10), FULL);
		assert_eq!(at(&out, 2, 10), 0);
	}

	#[test]
	fn even_odd_rule_cuts_a_hole() {
		let out = render(&mut test_glyphs(), FRAME_EVEN_ODD, W, H);
		assert_eq!(at(&out, 1, 10), FULL);
		assert_eq!(at(&out, 5, 10), 0);
	}

	#[test]
	fn rasterization_is_deterministic() {
		let (mut a, mut b) = (test_glyphs(), test_glyphs());
		for ch in [SQUARE, RING, RIGHT_HALF, FRAME_EVEN_ODD, CIRCLE_OVER_BOX] {
			assert_eq!(render(&mut a, ch, 13, 29), render(&mut b, ch, 13, 29));
		}
	}

	#[test]
	fn codepoint_without_svg_leaves_buffer_untouched() {
		let mut out = vec![7u8; W * H];
		assert!(!test_glyphs().rasterize('A', W, H, &mut out));
		assert!(out.iter().all(|&v| v == 7));
	}
}

mod priority {
	use super::*;

	#[test]
	fn svg_overrides_font_glyph_and_box_drawing() {
		let mut rasterizer = svg_rasterizer_at(16);
		let m = rasterizer.metrics();
		let mut glyphs = test_glyphs();
		let mut out = vec![0u8; m.glyph_w * m.glyph_h];
		for ch in [RIGHT_HALF, CIRCLE_OVER_BOX] {
			assert!(rasterizer.has_font_glyph(ch, Style::Regular), "{ch:?}");
			assert!(rasterizer.has_glyph(ch, Style::Regular));
			assert!(rasterizer.rasterize(ch, Style::Regular, &mut out));
			assert_eq!(out, render(&mut glyphs, ch, m.glyph_w, m.glyph_h), "{ch:?}");
		}
		let mut line = vec![0u8; m.glyph_w * m.glyph_h];
		assert!(box_drawing::draw(
			CIRCLE_OVER_BOX,
			m.glyph_w,
			m.glyph_h,
			&mut line
		));
		assert!(rasterizer.rasterize(CIRCLE_OVER_BOX, Style::Regular, &mut out));
		assert_ne!(out, line, "SVG wins over procedural box drawing");
	}

	#[test]
	fn font_glyphs_without_svg_are_unchanged() {
		let mut with_svg = svg_rasterizer_at(16);
		let mut plain = repo_rasterizer_at(16);
		let m = plain.metrics();
		let (mut a, mut b) = (
			vec![0u8; m.glyph_w * m.glyph_h],
			vec![0u8; m.glyph_w * m.glyph_h],
		);
		for ch in ['A', 'g', 'Ж', '\u{2550}', '\u{F0001}'] {
			assert_eq!(
				with_svg.rasterize(ch, Style::Bold, &mut a),
				plain.rasterize(ch, Style::Bold, &mut b)
			);
			assert_eq!(a, b, "{ch:?}");
		}
		assert!(!with_svg.is_style_independent('A'));
		assert!(with_svg.is_style_independent(SQUARE));
	}
}

mod atlas {
	use super::*;

	fn assert_same_in_all_channels(atlas: &mut Atlas, ch: char) {
		let slot = atlas.ensure(ch, Style::Regular);
		let regular = atlas.glyph_bytes(slot, Style::Regular);
		assert!(regular.iter().any(|&v| v > 0));
		for style in Style::ALL {
			assert!(atlas.is_ready(slot, style), "{style:?}");
			assert_eq!(atlas.glyph_bytes(slot, style), regular, "{style:?}");
		}
	}

	#[test]
	fn svg_glyph_fills_every_style_channel_at_once() {
		let mut atlas = svg_atlas_at(16);
		assert_same_in_all_channels(&mut atlas, SQUARE);
		assert_same_in_all_channels(&mut atlas, RIGHT_HALF);
	}

	#[test]
	fn svg_glyphs_survive_size_change_in_place() {
		let mut atlas = svg_atlas_at(16);
		atlas.ensure(SQUARE, Style::Regular);
		atlas.set_size(32).unwrap();
		let (w, h) = (atlas.glyph_w(), atlas.glyph_h());
		let slot = atlas.ensure(SQUARE, Style::Italic);
		assert_eq!(
			atlas.glyph_bytes(slot, Style::Italic),
			render(&mut test_glyphs(), SQUARE, w, h)
		);
		assert_same_in_all_channels(&mut atlas, SQUARE);
	}
}
