//! Процедурная псевдографика (ADR-0002, ТЗ FR-8.4): рамки U+2500–U+257F и блоки/тени U+2580–U+259F
//! рисуются по формулам на сетке ячейки, а не берутся из шрифта — линии ложатся ровно на пиксели
//! и стыкуются между соседними ячейками без зазоров.

use crate::procedural::Canvas;

/// Первая кодовая точка процедурного диапазона.
pub const BOX_FIRST: char = '\u{2500}';
/// Последняя кодовая точка процедурного диапазона.
pub const BOX_LAST: char = '\u{259F}';

const SIDE_NONE: u8 = 0;
const SIDE_LIGHT: u8 = 1;
const SIDE_HEAVY: u8 = 2;
const SIDE_DOUBLE: u8 = 3;

/// Число штрихов штриховой линии (2, 3 или 4) — в младших битах флагов; 0 — линия сплошная.
const DASH_COUNT_MASK: u8 = 0x0F;
/// Скруглённые углы ╭ ╮ ╯ ╰.
const FLAG_ARC: u8 = 0x20;
const FLAG_DIAG_UP: u8 = 0x40;
const FLAG_DIAG_DOWN: u8 = 0x80;

/// Точек на пиксель по каждой оси при расчёте покрытия дуг и диагоналей.
const COVERAGE_SAMPLES: i32 = 4;
/// Единиц координат на пиксель: точки стоят в нечётных единицах, центры и края — в чётных.
const SAMPLE_UNIT: i32 = 2 * COVERAGE_SAMPLES;

/// Покрытие теней ░ ▒ ▓.
const SHADE_LIGHT: u8 = 64;
const SHADE_MEDIUM: u8 = 128;
const SHADE_DARK: u8 = 192;

/// Стороны рамки U+2500–U+257F: по 2 бита на сторону (up, down, left, right — от младших битов),
/// 0 нет / 1 light / 2 heavy / 3 double; второй байт — число штрихов и флаги дуги, диагоналей.
/// Сгенерировано из имён Unicode (`unicodedata`), сверено по 17 известным кодам.
#[rustfmt::skip]
const BOX_SIDES: [(u8, u8); 128] = [
    (0x50, 0x00), (0xA0, 0x00), (0x05, 0x00), (0x0A, 0x00), (0x50, 0x03), (0xA0, 0x03), (0x05, 0x03), (0x0A, 0x03), // U+2500
    (0x50, 0x04), (0xA0, 0x04), (0x05, 0x04), (0x0A, 0x04), (0x44, 0x00), (0x84, 0x00), (0x48, 0x00), (0x88, 0x00), // U+2508
    (0x14, 0x00), (0x24, 0x00), (0x18, 0x00), (0x28, 0x00), (0x41, 0x00), (0x81, 0x00), (0x42, 0x00), (0x82, 0x00), // U+2510
    (0x11, 0x00), (0x21, 0x00), (0x12, 0x00), (0x22, 0x00), (0x45, 0x00), (0x85, 0x00), (0x46, 0x00), (0x49, 0x00), // U+2518
    (0x4A, 0x00), (0x86, 0x00), (0x89, 0x00), (0x8A, 0x00), (0x15, 0x00), (0x25, 0x00), (0x16, 0x00), (0x19, 0x00), // U+2520
    (0x1A, 0x00), (0x26, 0x00), (0x29, 0x00), (0x2A, 0x00), (0x54, 0x00), (0x64, 0x00), (0x94, 0x00), (0xA4, 0x00), // U+2528
    (0x58, 0x00), (0x68, 0x00), (0x98, 0x00), (0xA8, 0x00), (0x51, 0x00), (0x61, 0x00), (0x91, 0x00), (0xA1, 0x00), // U+2530
    (0x52, 0x00), (0x62, 0x00), (0x92, 0x00), (0xA2, 0x00), (0x55, 0x00), (0x65, 0x00), (0x95, 0x00), (0xA5, 0x00), // U+2538
    (0x56, 0x00), (0x59, 0x00), (0x5A, 0x00), (0x66, 0x00), (0x96, 0x00), (0x69, 0x00), (0x99, 0x00), (0xA6, 0x00), // U+2540
    (0xA9, 0x00), (0x6A, 0x00), (0x9A, 0x00), (0xAA, 0x00), (0x50, 0x02), (0xA0, 0x02), (0x05, 0x02), (0x0A, 0x02), // U+2548
    (0xF0, 0x00), (0x0F, 0x00), (0xC4, 0x00), (0x4C, 0x00), (0xCC, 0x00), (0x34, 0x00), (0x1C, 0x00), (0x3C, 0x00), // U+2550
    (0xC1, 0x00), (0x43, 0x00), (0xC3, 0x00), (0x31, 0x00), (0x13, 0x00), (0x33, 0x00), (0xC5, 0x00), (0x4F, 0x00), // U+2558
    (0xCF, 0x00), (0x35, 0x00), (0x1F, 0x00), (0x3F, 0x00), (0xF4, 0x00), (0x5C, 0x00), (0xFC, 0x00), (0xF1, 0x00), // U+2560
    (0x53, 0x00), (0xF3, 0x00), (0xF5, 0x00), (0x5F, 0x00), (0xFF, 0x00), (0x44, 0x20), (0x14, 0x20), (0x11, 0x20), // U+2568
    (0x41, 0x20), (0x00, 0x40), (0x00, 0x80), (0x00, 0xC0), (0x10, 0x00), (0x01, 0x00), (0x40, 0x00), (0x04, 0x00), // U+2570
    (0x20, 0x00), (0x02, 0x00), (0x80, 0x00), (0x08, 0x00), (0x90, 0x00), (0x09, 0x00), (0x60, 0x00), (0x06, 0x00), // U+2578
];

/// Входит ли кодовая точка в процедурный диапазон.
pub fn is_box_drawing(ch: char) -> bool {
	(BOX_FIRST..=BOX_LAST).contains(&ch)
}

/// Нарисовать символ в буфер покрытия `w × h` (row-major). `false` — код вне диапазона, буфер не тронут.
pub fn draw(ch: char, w: usize, h: usize, out: &mut [u8]) -> bool {
	if !is_box_drawing(ch) || out.len() < w * h || w == 0 || h == 0 {
		return false;
	}
	out[..w * h].fill(0);
	let mut canvas = Canvas { w, h, out };
	let cp = u32::from(ch);
	if cp < 0x2580 {
		let (sides, flags) = BOX_SIDES[(cp - u32::from(BOX_FIRST)) as usize];
		draw_lines(&mut canvas, sides, flags);
	} else {
		draw_block(&mut canvas, cp);
	}
	true
}

/// Толщины штрихов: light по высоте ячейки, heavy вдвое толще плюс пиксель, double — две light с зазором light.
struct Strokes {
	light: i32,
	heavy: i32,
	/// Смещение штрихов двойной линии от центра.
	double_offset: i32,
}

impl Strokes {
	fn for_cell(w: usize, h: usize) -> Self {
		let light = (h as i32 / 16).max(1);
		let heavy = (2 * light + 1).min(w as i32);
		Self {
			light,
			heavy,
			double_offset: light,
		}
	}

	/// Поперечный размер линии данного веса.
	fn span(&self, side: u8) -> i32 {
		match side {
			SIDE_LIGHT => self.light,
			SIDE_HEAVY => self.heavy,
			SIDE_DOUBLE => 2 * self.double_offset + self.light,
			_ => 0,
		}
	}
}

fn side(sides: u8, shift: u8) -> u8 {
	(sides >> shift) & 3
}

/// Рамки — четыре «руки» от центра к краям ячейки.
///
/// Одинарная/жирная рука идёт от края сквозь центр до дальней кромки перпендикулярных рук.
/// Исключение — тройник на двойной линии (╤ ╧ ╟ ╢): одиночная рука без руки напротив начинается
/// у **ближнего к ней** штриха двойной, зазор между штрихами остаётся пустым — так рисуют шрифты
/// (сверено с RobotoMono Nerd Font и Cascadia Mono); в крестах ╪ ╫ и углах ╒ ╓ одиночная линия,
/// как и в шрифтах, проходит сквозь зазор.
/// У двойной руки каждый из двух штрихов начинается у **дальней** кромки перпендикуляра, если
/// с его стороны есть рука (внутренний угол, тройник, ╬), иначе у **ближней** (внешний угол).
fn draw_lines(c: &mut Canvas, sides: u8, flags: u8) {
	if flags & (FLAG_DIAG_UP | FLAG_DIAG_DOWN) != 0 {
		draw_diagonals(c, flags);
		return;
	}
	if flags & FLAG_ARC != 0 {
		draw_arc(c, sides);
		return;
	}
	let s = Strokes::for_cell(c.w, c.h);
	let (up, down, left, right) = (
		side(sides, 0),
		side(sides, 2),
		side(sides, 4),
		side(sides, 6),
	);
	let w = c.w as i32;
	let h = c.h as i32;
	let cx = w / 2;
	let cy = h / 2;
	let v_half = s.span(up).max(s.span(down)) / 2;
	let h_half = s.span(left).max(s.span(right)) / 2;
	// Тройник на двойной линии: одиночная рука упирается в ближний штрих, не заходя в зазор.
	let onto_double_vertical = up == SIDE_DOUBLE && down == SIDE_DOUBLE;
	let onto_double_horizontal = left == SIDE_DOUBLE && right == SIDE_DOUBLE;
	let near_stroke_edge = s.double_offset - s.light / 2;

	// Горизонтальные руки: left идёт [0, end), right — [start, w).
	for (arm, is_left) in [(left, true), (right, false)] {
		match arm {
			SIDE_LIGHT | SIDE_HEAVY => {
				let t = s.span(arm);
				let (x0, x1) = match (is_left, onto_double_vertical) {
					(true, true) if right == SIDE_NONE => (0, cx - near_stroke_edge),
					(true, _) => (0, cx + v_half + 1),
					(false, true) if left == SIDE_NONE => (cx + near_stroke_edge, w),
					(false, _) => (cx - v_half, w),
				};
				c.rect(x0, cy - t / 2, x1, cy - t / 2 + t, 255);
			}
			SIDE_DOUBLE => {
				for (dy, perpendicular) in [(-s.double_offset, up), (s.double_offset, down)] {
					let blocked = perpendicular != SIDE_NONE;
					let (x0, x1) = match (is_left, blocked) {
						(true, true) => (0, cx - v_half + 1),
						(true, false) => (0, cx + v_half + 1),
						(false, true) => (cx + v_half, w),
						(false, false) => (cx - v_half, w),
					};
					let y0 = cy + dy - s.light / 2;
					c.rect(x0, y0, x1, y0 + s.light, 255);
				}
			}
			_ => {}
		}
	}
	// Вертикальные руки: up идёт [0, end), down — [start, h).
	for (arm, is_up) in [(up, true), (down, false)] {
		match arm {
			SIDE_LIGHT | SIDE_HEAVY => {
				let t = s.span(arm);
				let (y0, y1) = match (is_up, onto_double_horizontal) {
					(true, true) if down == SIDE_NONE => (0, cy - near_stroke_edge),
					(true, _) => (0, cy + h_half + 1),
					(false, true) if up == SIDE_NONE => (cy + near_stroke_edge, h),
					(false, _) => (cy - h_half, h),
				};
				c.rect(cx - t / 2, y0, cx - t / 2 + t, y1, 255);
			}
			SIDE_DOUBLE => {
				for (dx, perpendicular) in [(-s.double_offset, left), (s.double_offset, right)] {
					let blocked = perpendicular != SIDE_NONE;
					let (y0, y1) = match (is_up, blocked) {
						(true, true) => (0, cy - h_half + 1),
						(true, false) => (0, cy + h_half + 1),
						(false, true) => (cy + h_half, h),
						(false, false) => (cy - h_half, h),
					};
					let x0 = cx + dx - s.light / 2;
					c.rect(x0, y0, x0 + s.light, y1, 255);
				}
			}
			_ => {}
		}
	}
	let dash_count = (flags & DASH_COUNT_MASK) as i32;
	if dash_count != 0 {
		cut_dash_gaps(c, dash_count, left != SIDE_NONE);
	}
}

/// Штриховая линия: сплошная уже нарисована, зазоры вырезаются поперёк всей ячейки.
///
/// Длина делится на `count` периодов «штрих + зазор» с целыми границами, зазор — в конце периода:
/// ряд одинаковых символов идёт с ровным шагом через границы ячеек. Если период выходит короче
/// двух пикселей, штрихов становится меньше; линия короче двух пикселей остаётся сплошной.
fn cut_dash_gaps(c: &mut Canvas, count: i32, is_horizontal: bool) {
	let w = c.w as i32;
	let h = c.h as i32;
	let length = if is_horizontal { w } else { h };
	let count = count.min(length / 2).max(1);
	let period = length / count;
	if period < 2 {
		return;
	}
	let gap = ((period + 1) / 3).max(1);
	for i in 1..=count {
		let end = length * i / count;
		if is_horizontal {
			c.rect(end - gap, 0, end, h, 0);
		} else {
			c.rect(0, end - gap, w, end, 0);
		}
	}
}

/// Дуги ╭ ╮ ╯ ╰: сглаженная четверть окружности между осями двух рук и прямые хвосты до краёв.
///
/// Радиус один на все четыре символа — наибольший, при котором окружность не выходит за ячейку, —
/// поэтому пары `╭╮` и `╰╯` симметричны. Концы дуги ложатся в те же пиксели, что `─` и `│`.
fn draw_arc(c: &mut Canvas, sides: u8) {
	let w = c.w as i32;
	let h = c.h as i32;
	let t = Strokes::for_cell(c.w, c.h).light;
	// Левый верхний угол пересечения рук — как у прямых линий в `draw_lines`.
	let (x0, y0) = (w / 2 - t / 2, h / 2 - t / 2);
	// Оси рук и радиус — в половинах пикселя: ось линии нечётной толщины идёт через центры пикселей.
	let (axis_x, axis_y) = (2 * x0 + t, 2 * y0 + t);
	let radius = axis_x.min(2 * w - axis_x).min(axis_y).min(2 * h - axis_y);
	let goes_right = side(sides, 6) != SIDE_NONE;
	let goes_down = side(sides, 2) != SIDE_NONE;
	// Центр окружности; чётность оси и радиуса совпадает, поэтому он всегда на границе пикселей.
	let center_x = (axis_x + if goes_right { radius } else { -radius }) / 2;
	let center_y = (axis_y + if goes_down { radius } else { -radius }) / 2;

	let (arc_x0, arc_x1) = if goes_right {
		c.rect(center_x, y0, w, y0 + t, 255);
		(x0, center_x)
	} else {
		c.rect(0, y0, center_x, y0 + t, 255);
		(center_x, x0 + t)
	};
	let (arc_y0, arc_y1) = if goes_down {
		c.rect(x0, center_y, x0 + t, h, 255);
		(y0, center_y)
	} else {
		c.rect(x0, 0, x0 + t, center_y, 255);
		(center_y, y0 + t)
	};
	let (ring_x, ring_y) = (center_x * SAMPLE_UNIT, center_y * SAMPLE_UNIT);
	let inner_squared = ((radius - t).max(0) * COVERAGE_SAMPLES).pow(2);
	let outer_squared = ((radius + t) * COVERAGE_SAMPLES).pow(2);
	for y in arc_y0..arc_y1 {
		for x in arc_x0..arc_x1 {
			let coverage = sampled_coverage(x, y, |sx, sy| {
				let (dx, dy) = (sx - ring_x, sy - ring_y);
				(inner_squared..=outer_squared).contains(&(dx * dx + dy * dy))
			});
			c.set(x, y, coverage);
		}
	}
}

/// Диагонали ╱ ╲ ╳ из угла в угол ячейки: толщина light, покрытие сглажено так же, как у дуг.
fn draw_diagonals(c: &mut Canvas, flags: u8) {
	// i64: квадрат отклонения точки от оси при ячейке 48 px не помещается в i32 с запасом.
	let w = c.w as i64;
	let h = c.h as i64;
	let t = Strokes::for_cell(c.w, c.h).light as i64;
	let unit = SAMPLE_UNIT as i64;
	// Точка лежит в полосе, если её расстояние до оси не больше t/2; сравниваются квадраты,
	// умноженные на длину диагонали, — без корня и деления.
	let limit = (t * unit / 2).pow(2) * (w * w + h * h);
	let is_in_band = |deviation: i64| deviation * deviation <= limit;
	for y in 0..c.h as i32 {
		for x in 0..c.w as i32 {
			let coverage = sampled_coverage(x, y, |sx, sy| {
				let (sx, sy) = (sx as i64, sy as i64);
				let from_down = h * sx - w * sy; // ╲: верх-лево → низ-право
				let from_up = h * sx + w * sy - w * h * unit; // ╱: низ-лево → верх-право
				(flags & FLAG_DIAG_DOWN != 0 && is_in_band(from_down))
					|| (flags & FLAG_DIAG_UP != 0 && is_in_band(from_up))
			});
			c.set(x, y, coverage);
		}
	}
}

/// Покрытие пикселя — доля из `COVERAGE_SAMPLES²` его точек, для которых `contains` истинно.
///
/// Точки передаются в целых координатах, `SAMPLE_UNIT` единиц на пиксель: без плавающей точки
/// результат не зависит от платформы.
fn sampled_coverage(x: i32, y: i32, contains: impl Fn(i32, i32) -> bool) -> u8 {
	let mut hits = 0;
	for j in 0..COVERAGE_SAMPLES {
		for i in 0..COVERAGE_SAMPLES {
			if contains(SAMPLE_UNIT * x + 2 * i + 1, SAMPLE_UNIT * y + 2 * j + 1) {
				hits += 1;
			}
		}
	}
	(hits * 255 / (COVERAGE_SAMPLES * COVERAGE_SAMPLES)) as u8
}

/// Блоки, восьмые доли, тени и квадранты U+2580–U+259F.
fn draw_block(c: &mut Canvas, cp: u32) {
	let w = c.w as i32;
	let h = c.h as i32;
	let eighth_h = |n: i32| (h * n + 4) / 8;
	let eighth_w = |n: i32| (w * n + 4) / 8;
	match cp {
		0x2580 => c.rect(0, 0, w, eighth_h(4), 255),
		0x2581..=0x2588 => {
			let n = (cp - 0x2580) as i32;
			c.rect(0, h - eighth_h(n), w, h, 255);
		}
		0x2589..=0x258F => {
			let n = 8 - (cp - 0x2588) as i32;
			c.rect(0, 0, eighth_w(n), h, 255);
		}
		0x2590 => c.rect(w - eighth_w(4), 0, w, h, 255),
		0x2591 => c.rect(0, 0, w, h, SHADE_LIGHT),
		0x2592 => c.rect(0, 0, w, h, SHADE_MEDIUM),
		0x2593 => c.rect(0, 0, w, h, SHADE_DARK),
		0x2594 => c.rect(0, 0, w, eighth_h(1), 255),
		0x2595 => c.rect(w - eighth_w(1), 0, w, h, 255),
		0x2596..=0x259F => draw_quadrants(c, cp),
		_ => {}
	}
}

/// Квадранты: биты (верх-лево, верх-право, низ-лево, низ-право) по таблице Unicode.
fn draw_quadrants(c: &mut Canvas, cp: u32) {
	const QUADRANTS: [u8; 10] = [
		0b0010, // ▖ U+2596 низ-лево
		0b0001, // ▗ низ-право
		0b1000, // ▘ верх-лево
		0b1011, // ▙ верх-лево + низ-лево + низ-право
		0b1001, // ▚ верх-лево + низ-право
		0b1110, // ▛ верх-лево + верх-право + низ-лево
		0b1101, // ▜ верх-лево + верх-право + низ-право
		0b0100, // ▝ верх-право
		0b0110, // ▞ верх-право + низ-лево
		0b0111, // ▟ верх-право + низ-лево + низ-право
	];
	let bits = QUADRANTS[(cp - 0x2596) as usize];
	let w = c.w as i32;
	let h = c.h as i32;
	let (mx, my) = ((w + 1) / 2, (h + 1) / 2);
	if bits & 0b1000 != 0 {
		c.rect(0, 0, mx, my, 255);
	}
	if bits & 0b0100 != 0 {
		c.rect(mx, 0, w, my, 255);
	}
	if bits & 0b0010 != 0 {
		c.rect(0, my, mx, h, 255);
	}
	if bits & 0b0001 != 0 {
		c.rect(mx, my, w, h, 255);
	}
}
