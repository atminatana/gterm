//! Фигуры (ADR-0022): круги и их части, треугольники, ромб, диагонали — выпуклые области в
//! ячейке с покрытием по 4×4 точкам на пиксель, как дуги [`crate::box_drawing`]. Части кругов
//! прижаты к стороне или углу так, что ◖+◗ дают круг в одной ячейке, U+1FBE9+U+1FBEB — круг
//! 2×1, U+1FBEA+U+1FBE8 — 1×2, U+1FBEC–U+1FBEF — 2×2 (трактовка таблиц PhMajerus «Blocks
//! tables»). Диагонали — угол ячейки по ту сторону прямой между точками сетки 3×3 из имени знака.

use crate::procedural::{Canvas, in_ranges};

/// Диапазоны фигур (включительно): ■-подобные геометрические фигуры, диагонали, треугольные
/// блоки, части кругов.
pub const RANGES: &[(char, char)] = &[
	('\u{25B2}', '\u{25B2}'),
	('\u{25B6}', '\u{25B6}'),
	('\u{25BC}', '\u{25BC}'),
	('\u{25C0}', '\u{25C0}'),
	('\u{25C6}', '\u{25C6}'),
	('\u{25CF}', '\u{25CF}'),
	('\u{25D6}', '\u{25D7}'),
	('\u{25E2}', '\u{25E5}'),
	('\u{1FB3C}', '\u{1FB6F}'),
	('\u{1FB9A}', '\u{1FB9B}'),
	('\u{1FBE8}', '\u{1FBEF}'),
];

/// Точек на пиксель по каждой оси при расчёте покрытия.
const SAMPLES: i32 = 4;

/// Диагонали U+1FB3C–U+1FB67: (угол, Ax, Ay, Bx, By) — угол ячейки (0 низ-лево, 1 низ-право,
/// 2 верх-лево, 3 верх-право), отрезанный прямой A–B; x в половинах ширины (0 left, 1 centre,
/// 2 right), y в третях высоты (0 upper, 1 upper middle, 2 lower middle, 3 lower).
/// Сгенерировано из имён Unicode `… BLOCK DIAGONAL <A> TO <B>` (`unicodedata`, Python 3.14).
#[rustfmt::skip]
const DIAGONALS: [(u8, u8, u8, u8, u8); 44] = [
    (0, 0, 2, 1, 3), // U+1FB3C
    (0, 0, 2, 2, 3), // U+1FB3D
    (0, 0, 1, 1, 3), // U+1FB3E
    (0, 0, 1, 2, 3), // U+1FB3F
    (0, 0, 0, 1, 3), // U+1FB40
    (1, 0, 1, 1, 0), // U+1FB41
    (1, 0, 1, 2, 0), // U+1FB42
    (1, 0, 2, 1, 0), // U+1FB43
    (1, 0, 2, 2, 0), // U+1FB44
    (1, 0, 3, 1, 0), // U+1FB45
    (1, 0, 2, 2, 1), // U+1FB46
    (1, 1, 3, 2, 2), // U+1FB47
    (1, 0, 3, 2, 2), // U+1FB48
    (1, 1, 3, 2, 1), // U+1FB49
    (1, 0, 3, 2, 1), // U+1FB4A
    (1, 1, 3, 2, 0), // U+1FB4B
    (0, 1, 0, 2, 1), // U+1FB4C
    (0, 0, 0, 2, 1), // U+1FB4D
    (0, 1, 0, 2, 2), // U+1FB4E
    (0, 0, 0, 2, 2), // U+1FB4F
    (0, 1, 0, 2, 3), // U+1FB50
    (0, 0, 1, 2, 2), // U+1FB51
    (3, 0, 2, 1, 3), // U+1FB52
    (3, 0, 2, 2, 3), // U+1FB53
    (3, 0, 1, 1, 3), // U+1FB54
    (3, 0, 1, 2, 3), // U+1FB55
    (3, 0, 0, 1, 3), // U+1FB56
    (2, 0, 1, 1, 0), // U+1FB57
    (2, 0, 1, 2, 0), // U+1FB58
    (2, 0, 2, 1, 0), // U+1FB59
    (2, 0, 2, 2, 0), // U+1FB5A
    (2, 0, 3, 1, 0), // U+1FB5B
    (2, 0, 2, 2, 1), // U+1FB5C
    (2, 1, 3, 2, 2), // U+1FB5D
    (2, 0, 3, 2, 2), // U+1FB5E
    (2, 1, 3, 2, 1), // U+1FB5F
    (2, 0, 3, 2, 1), // U+1FB60
    (2, 1, 3, 2, 0), // U+1FB61
    (3, 1, 0, 2, 1), // U+1FB62
    (3, 0, 0, 2, 1), // U+1FB63
    (3, 1, 0, 2, 2), // U+1FB64
    (3, 0, 0, 2, 2), // U+1FB65
    (3, 1, 0, 2, 3), // U+1FB66
    (3, 0, 1, 2, 2), // U+1FB67
];

type Point = (f32, f32);

/// Входит ли кодовая точка в фигуры.
pub fn is_shape(ch: char) -> bool {
	in_ranges(ch, RANGES)
}

/// Нарисовать фигуру в буфер покрытия `w × h` (row-major). `false` — не фигура или негодный
/// буфер, буфер не тронут.
pub fn draw(ch: char, w: usize, h: usize, out: &mut [u8]) -> bool {
	if !is_shape(ch) {
		return false;
	}
	let Some(mut c) = Canvas::cleared(w, h, out) else {
		return false;
	};
	let (w, h) = (w as f32, h as f32);
	let (cx, cy) = (w / 2.0, h / 2.0);
	// Четверти от стороны к центру: вершины против часовой стрелки в координатах y вниз.
	let left: [Point; 3] = [(0.0, 0.0), (0.0, h), (cx, cy)];
	let upper: [Point; 3] = [(0.0, 0.0), (w, 0.0), (cx, cy)];
	let right: [Point; 3] = [(w, 0.0), (w, h), (cx, cy)];
	let lower: [Point; 3] = [(0.0, h), (w, h), (cx, cy)];
	let cp = u32::from(ch);
	match cp {
		// Круг в ячейке и его половины.
		0x25CF => fill_region(&mut c, |x, y| inside_ellipse(cx, cy, cx, cy, x, y)),
		0x25D6 => fill_region(&mut c, |x, y| {
			x <= cx && inside_ellipse(cx, cy, cx, cy, x, y)
		}),
		0x25D7 => fill_region(&mut c, |x, y| {
			x >= cx && inside_ellipse(cx, cy, cx, cy, x, y)
		}),
		// Половины круга 2×1 и 1×2, прижатые к стороне: центр эллипса — на этой стороне.
		0x1FBE8 => fill_region(&mut c, |x, y| inside_ellipse(cx, 0.0, cx, h, x, y)),
		0x1FBE9 => fill_region(&mut c, |x, y| inside_ellipse(w, cy, w, cy, x, y)),
		0x1FBEA => fill_region(&mut c, |x, y| inside_ellipse(cx, h, cx, h, x, y)),
		0x1FBEB => fill_region(&mut c, |x, y| inside_ellipse(0.0, cy, w, cy, x, y)),
		// Четверти круга 2×2, прижатые к углу: центр эллипса — в этом углу.
		0x1FBEC => fill_region(&mut c, |x, y| inside_ellipse(w, 0.0, w, h, x, y)),
		0x1FBED => fill_region(&mut c, |x, y| inside_ellipse(0.0, h, w, h, x, y)),
		0x1FBEE => fill_region(&mut c, |x, y| inside_ellipse(w, h, w, h, x, y)),
		0x1FBEF => fill_region(&mut c, |x, y| inside_ellipse(0.0, 0.0, w, h, x, y)),
		// Треугольники на всю ячейку, ромб, половины по диагонали.
		0x25B2 => polygon(&mut c, &[(0.0, h), (w, h), (cx, 0.0)]),
		0x25B6 => polygon(&mut c, &[(0.0, 0.0), (0.0, h), (w, cy)]),
		0x25BC => polygon(&mut c, &[(0.0, 0.0), (w, 0.0), (cx, h)]),
		0x25C0 => polygon(&mut c, &[(w, 0.0), (w, h), (0.0, cy)]),
		0x25C6 => polygon(&mut c, &[(cx, 0.0), (w, cy), (cx, h), (0.0, cy)]),
		0x25E2 => polygon(&mut c, &[(w, 0.0), (w, h), (0.0, h)]),
		0x25E3 => polygon(&mut c, &[(0.0, 0.0), (0.0, h), (w, h)]),
		0x25E4 => polygon(&mut c, &[(0.0, 0.0), (w, 0.0), (0.0, h)]),
		0x25E5 => polygon(&mut c, &[(0.0, 0.0), (w, 0.0), (w, h)]),
		// Четверти, три четверти (ячейка без четверти) и пары противоположных четвертей.
		0x1FB6C => polygon(&mut c, &left),
		0x1FB6D => polygon(&mut c, &upper),
		0x1FB6E => polygon(&mut c, &right),
		0x1FB6F => polygon(&mut c, &lower),
		0x1FB68 => fill_region(&mut c, |x, y| !inside_convex(&left, x, y)),
		0x1FB69 => fill_region(&mut c, |x, y| !inside_convex(&upper, x, y)),
		0x1FB6A => fill_region(&mut c, |x, y| !inside_convex(&right, x, y)),
		0x1FB6B => fill_region(&mut c, |x, y| !inside_convex(&lower, x, y)),
		0x1FB9A => fill_region(&mut c, |x, y| {
			inside_convex(&upper, x, y) || inside_convex(&lower, x, y)
		}),
		0x1FB9B => fill_region(&mut c, |x, y| {
			inside_convex(&left, x, y) || inside_convex(&right, x, y)
		}),
		0x1FB3C..=0x1FB67 => {
			let (corner, ax, ay, bx, by) = DIAGONALS[(cp - 0x1FB3C) as usize];
			let a = (f32::from(ax) * cx, f32::from(ay) * h / 3.0);
			let b = (f32::from(bx) * cx, f32::from(by) * h / 3.0);
			let corner = match corner {
				0 => (0.0, h),
				1 => (w, h),
				2 => (0.0, 0.0),
				_ => (w, 0.0),
			};
			let corner_side = side(a, b, corner.0, corner.1);
			fill_region(&mut c, |x, y| side(a, b, x, y) * corner_side >= 0.0);
		}
		_ => return false,
	}
	true
}

/// Залить область `inside` (координаты в пикселях ячейки, y вниз): покрытие пикселя — доля его
/// 4×4 точек внутри области; стороны вдоль осей на целых координатах остаются чёткими.
fn fill_region(c: &mut Canvas, inside: impl Fn(f32, f32) -> bool) {
	let step = 1.0 / SAMPLES as f32;
	let per_pixel = SAMPLES * SAMPLES;
	for y in 0..c.h {
		for x in 0..c.w {
			let mut hits = 0;
			for j in 0..SAMPLES {
				for i in 0..SAMPLES {
					let px = x as f32 + (i as f32 + 0.5) * step;
					let py = y as f32 + (j as f32 + 0.5) * step;
					if inside(px, py) {
						hits += 1;
					}
				}
			}
			c.set(
				x as i32,
				y as i32,
				((hits * 255 + per_pixel / 2) / per_pixel) as u8,
			);
		}
	}
}

fn polygon(c: &mut Canvas, points: &[Point]) {
	fill_region(c, |x, y| inside_convex(points, x, y));
}

/// Точка внутри выпуклого многоугольника (или на его границе): все векторные произведения рёбер
/// одного знака.
fn inside_convex(points: &[Point], x: f32, y: f32) -> bool {
	let mut sign = 0.0f32;
	for i in 0..points.len() {
		let a = points[i];
		let b = points[(i + 1) % points.len()];
		let cross = side(a, b, x, y);
		if cross != 0.0 {
			if sign == 0.0 {
				sign = cross.signum();
			} else if cross.signum() != sign {
				return false;
			}
		}
	}
	true
}

/// Эллипс с центром `(cx, cy)` и полуосями `(rx, ry)`.
fn inside_ellipse(cx: f32, cy: f32, rx: f32, ry: f32, x: f32, y: f32) -> bool {
	let dx = (x - cx) / rx;
	let dy = (y - cy) / ry;
	dx * dx + dy * dy <= 1.0
}

/// Сторона прямой A–B, на которой лежит точка: знак векторного произведения.
fn side(a: Point, b: Point, x: f32, y: f32) -> f32 {
	(b.0 - a.0) * (y - a.1) - (b.1 - a.1) * (x - a.0)
}
