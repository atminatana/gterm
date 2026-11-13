//! Процедурные мозаики и фигуры (ADR-0022): сетки секстантов и октантов, «Centered» и
//! восьмушки, части кругов, треугольники, диагонали, диспетчер и режимы растеризатора.

mod common;

use common::repo_rasterizer;
use gterm::block_mosaic;
use gterm::cell::Style;
use gterm::procedural::{self, is_procedural};
use gterm::rasterizer::BoxDrawingMode;
use gterm::shapes;

const W: usize = 10;
const H: usize = 21;

fn drawn(cp: u32) -> Vec<u8> {
	let mut out = vec![0u8; W * H];
	assert!(
		procedural::draw(char::from_u32(cp).unwrap(), W, H, &mut out),
		"U+{cp:04X} не нарисован"
	);
	out
}

fn ink(buf: &[u8]) -> u32 {
	buf.iter().map(|&v| u32::from(v)).sum()
}

/// Попиксельная сумма покрытий нескольких знаков.
fn sum_of(cps: &[u32]) -> Vec<u32> {
	let mut total = vec![0u32; W * H];
	for &cp in cps {
		for (t, v) in total.iter_mut().zip(drawn(cp)) {
			*t += u32::from(v);
		}
	}
	total
}

fn assert_all(total: &[u32], expected: u32, tolerance: u32, what: &str) {
	for (i, &v) in total.iter().enumerate() {
		assert!(
			v.abs_diff(expected) <= tolerance,
			"{what}: пиксель ({}, {}) = {v}, ожидалось {expected}",
			i % W,
			i / W
		);
	}
}

/// К2: одиночные клетки секстантов и октантов разбивают ячейку без зазоров и наложений, узоры
/// только 0/255, дополнение по таблице — █; октанты ложатся на квадранты.
#[test]
fn mosaics_tile_the_cell_exactly() {
	// Одноклеточные секстанты: узоры 1, 2, 4, 8, 16, 32 → U+1FB00, 01, 03, 07, 0F, 1E.
	assert_all(
		&sum_of(&[0x1FB00, 0x1FB01, 0x1FB03, 0x1FB07, 0x1FB0F, 0x1FB1E]),
		255,
		0,
		"секстанты",
	);
	// Одноклеточные октанты: 1 и 2 — U+1CEA8/AB, 4 — U+1CD00, 8 — U+1CD03, 16 — U+1CD09,
	// 32 — U+1CD18, 64 и 128 — U+1CEA3/A0.
	assert_all(
		&sum_of(&[
			0x1CEA8, 0x1CEAB, 0x1CD00, 0x1CD03, 0x1CD09, 0x1CD18, 0x1CEA3, 0x1CEA0,
		]),
		255,
		0,
		"октанты",
	);
	// Дополнения: U+1FB3B = все клетки кроме первой; U+1CDE5 = все октанты кроме первого.
	assert_all(
		&sum_of(&[0x1FB00, 0x1FB3B]),
		255,
		0,
		"секстант + дополнение",
	);
	assert_all(&sum_of(&[0x1CEA8, 0x1CDE5]), 255, 0, "октант + дополнение");
	// Верхние два левых октанта (узор 0x05 в таблице исключён — это ▘) в сумме равны квадранту
	// ▘ U+2598: сетка 2×4 ложится на сетку 2×2.
	let upper_left = sum_of(&[0x1CEA8, 0x1CD00]);
	let quadrant: Vec<u32> = drawn(0x2598).into_iter().map(u32::from).collect();
	assert_eq!(upper_left, quadrant, "октанты 1+3 = квадрант ▘");
	for cp in [
		0x1FB00, 0x1FB2A, 0x1CD77, 0x1CDE5, 0x25A0, 0x1FBE4, 0x1FB85, 0x1FB8B,
	] {
		assert!(
			drawn(cp).iter().all(|&v| v == 0 || v == 255),
			"U+{cp:04X}: полутона в мозаике"
		);
	}
}

/// К2: «Centered» и восьмушки — положение блоков по сетке.
#[test]
fn centered_blocks_and_eighths_follow_the_grid() {
	let square = drawn(0x25A0);
	// Центральный блок 2×2 сетки 4×4: колонки [edge(10,1,4), edge(10,3,4)) = [3, 8),
	// строки [edge(21,1,4), edge(21,3,4)) = [5, 16).
	assert_eq!(square[5 * W + 3], 255);
	assert_eq!(square[5 * W + 2], 0);
	assert_eq!(square[15 * W + 7], 255);
	assert_eq!(square[16 * W + 7], 0);
	let upper_centre = drawn(0x1FBE4);
	assert_eq!(upper_centre[3], 255);
	assert_eq!(upper_centre[2], 0);
	assert_eq!(upper_centre[11 * W + 3], 0);
	// Верхние 7/8 и правые 7/8.
	let upper = drawn(0x1FB86);
	assert_eq!(ink(&upper), 255 * (W as u32) * ((21 * 7 + 4) / 8));
	let right = drawn(0x1FB8B);
	assert_eq!(right[0], 0);
	assert_eq!(right[W - 1], 255);
}

/// К4: круг симметричен, половины складываются в круг, четверти 2×2 — в эллипс площади π·w·h.
#[test]
fn circle_parts_compose() {
	let circle = drawn(0x25CF);
	for y in 0..H {
		for x in 0..W {
			assert_eq!(
				circle[y * W + x],
				circle[y * W + (W - 1 - x)],
				"● по горизонтали"
			);
			assert_eq!(
				circle[y * W + x],
				circle[(H - 1 - y) * W + x],
				"● по вертикали"
			);
		}
	}
	let halves = sum_of(&[0x25D6, 0x25D7]);
	let full: Vec<u32> = circle.iter().map(|&v| u32::from(v)).collect();
	assert_eq!(halves, full, "◖ + ◗ = ●");
	// Четверти круга 2×2: четыре ячейки — эллипс с полуосями (w, h).
	let quarters: u32 = [0x1FBEC, 0x1FBED, 0x1FBEE, 0x1FBEF]
		.iter()
		.map(|&cp| ink(&drawn(cp)))
		.sum();
	let area = quarters as f64 / 255.0;
	let expected = std::f64::consts::PI * W as f64 * H as f64;
	assert!(
		(area - expected).abs() / expected < 0.02,
		"четверти 2×2: площадь {area:.1} против π·w·h = {expected:.1}"
	);
	// Половины 2×1 и 1×2 — по половине того же эллипса.
	let horizontal = (ink(&drawn(0x1FBE9)) + ink(&drawn(0x1FBEB))) as f64 / 255.0;
	assert!(
		(horizontal - std::f64::consts::PI * W as f64 * H as f64 / 2.0).abs() / horizontal < 0.02
	);
}

/// К4: треугольники — симметрия и дополнения; пары диагоналей дают полную ячейку.
#[test]
fn triangles_and_diagonals_complement() {
	let up = drawn(0x25B2);
	for y in 0..H {
		for x in 0..W {
			assert_eq!(up[y * W + x], up[y * W + (W - 1 - x)], "▲ симметричен");
		}
	}
	// Общее ребро считается обеими половинами — до 1/16 покрытия сверх.
	assert_all(&sum_of(&[0x25E2, 0x25E4]), 255, 16, "◢ + ◤");
	assert_all(&sum_of(&[0x25E3, 0x25E5]), 255, 16, "◣ + ◥");
	assert_all(
		&sum_of(&[0x1FB6C, 0x1FB68]),
		255,
		16,
		"четверть + три четверти",
	);
	assert_all(
		&sum_of(&[0x1FB6C, 0x1FB6D, 0x1FB6E, 0x1FB6F]),
		255,
		32,
		"четыре четверти",
	);
	assert_all(&sum_of(&[0x1FB9A, 0x1FB9B]), 255, 32, "две пары четвертей");
	// Диагонали: нижний левый угол U+1FB3C–40 и верхний правый U+1FB52–56 по тем же прямым.
	for i in 0..5 {
		assert_all(
			&sum_of(&[0x1FB3C + i, 0x1FB52 + i]),
			255,
			16,
			&format!("диагонали U+{:04X} + U+{:04X}", 0x1FB3C + i, 0x1FB52 + i),
		);
	}
	// Диагональ U+1FB3C: угол низ-лево, прямая от (0, 2/3·h) к (w/2, h) — маленький треугольник.
	let small = ink(&drawn(0x1FB3C)) as f64 / 255.0;
	let expected = (W as f64 / 2.0) * (H as f64 / 3.0) / 2.0;
	assert!(
		(small - expected).abs() < 1.0,
		"площадь U+1FB3C {small} против {expected}"
	);
}

/// К5: диспетчер и режимы растеризатора: диапазоны, `Combined` без глифа → формула,
/// независимость от начертания в `Procedural`.
#[test]
fn dispatcher_and_rasterizer_modes() {
	for cp in [
		0x2500, 0x25A0, 0x25CF, 0x25E2, 0x1CD00, 0x1CEAB, 0x1FB00, 0x1FB3C, 0x1FB6F, 0x1FB8B,
		0x1FB9B, 0x1FBE4, 0x1FBEF,
	] {
		assert!(is_procedural(char::from_u32(cp).unwrap()), "U+{cp:04X}");
	}
	for ch in [
		'A',
		'\u{25A1}',
		'\u{1FB70}',
		'\u{1FB9C}',
		'\u{1FBF0}',
		'\u{1CDE6}',
		'\u{2800}',
	] {
		assert!(!is_procedural(ch), "U+{:04X} не процедурный", u32::from(ch));
	}
	assert!(block_mosaic::is_mosaic('\u{1FB00}') && !shapes::is_shape('\u{1FB00}'));
	assert!(shapes::is_shape('\u{1FB3C}') && !block_mosaic::is_mosaic('\u{1FB3C}'));
	let total: usize = procedural::ranges()
		.map(|(a, b)| u32::from(b) as usize - u32::from(a) as usize + 1)
		.sum();
	// Рамки 160; мозаики: ■ 1 + октанты 230 + половины четвертей 4 + секстанты 60 + восьмушки 10
	// + Centered 4 = 309; фигуры: 12 знаков Geometric Shapes + 52 (U+1FB3C–6F) + 2 + 8 = 74.
	assert_eq!(total, 160 + 309 + 74, "число процедурных знаков");

	let mut r = repo_rasterizer();
	let m = r.metrics();
	let mut procedural_buf = vec![0u8; m.glyph_w * m.glyph_h];
	let octant = '\u{1CD00}';
	assert!(
		r.is_style_independent(octant),
		"в Procedural октант одинаков во всех каналах"
	);
	assert!(r.rasterize(octant, Style::Regular, &mut procedural_buf));
	r.set_box_drawing_mode(BoxDrawingMode::Combined);
	let mut combined = vec![0u8; m.glyph_w * m.glyph_h];
	assert!(r.rasterize(octant, Style::Bold, &mut combined));
	assert_eq!(combined, procedural_buf, "Combined без глифа → формула");
	r.set_box_drawing_mode(BoxDrawingMode::Font);
	assert!(
		!r.has_glyph(octant, Style::Regular),
		"в Font октанта Unicode 16 в шрифте нет"
	);
}
