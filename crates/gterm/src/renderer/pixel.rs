//! Попиксельная арифметика Software-рендерера (§7.3): смешивание по покрытию, альфа символа,
//! предумноженная текстура наложения и BOLD. Всё целочисленно и бит в бит совпадает с шейдером
//! (ADR-0002, ADR-0013, ADR-0014); каналы R и B считаются парой в одном умножении (ADR-0015).

use crate::cell::Color;

/// Полное покрытие пикселя.
pub(crate) const COVERAGE_FULL: u8 = 255;
/// Каналы R и B пикселя — младшие байты двух 16-битных половин: пара считается одним
/// умножением (ADR-0015).
const RB: u32 = 0x00FF_00FF;
/// Поправка округления `+127` для обеих половин пары.
const ROUND_PAIR: u32 = 0x007F_007F;
/// Прибавка к каналу для BOLD (§7.3): `c' = min(c + BOLD_BOOST, 255)`.
const BOLD_BOOST: u32 = 0x55;

/// RGB цвета в формате кадра `0x00RRGGBB`; альфа учитывается отдельно (ADR-0013).
pub(crate) fn encode_color(color: Color) -> u32 {
	color.0 & 0x00FF_FFFF
}

// Попиксельные функции ниже встраиваются принудительно: при `opt-level = "z"` вызов на пиксель
// стоит до трети кадра (замер, ADR-0014).

/// `y / 255` для `y ≤ 65534` без деления: `(y + 1 + (y >> 8)) >> 8` совпадает с частным на всём
/// диапазоне (проверено перебором); у формул ниже `y ≤ 255·255 + 127 = 65152`. При
/// `opt-level = "z"` деление на константу не заменяется умножением и стоит десятки тактов на
/// канал (замер, ADR-0014). Шейдер делит как есть — результат тот же бит в бит.
#[inline(always)]
pub fn div255(y: u32) -> u32 {
	debug_assert!(y < 65535);
	(y + 1 + (y >> 8)) >> 8
}

/// `div255` для двух 16-битных половин сразу (ADR-0015). У каждой половины `y ≤ 65152`, поэтому
/// `y + 1 + (y >> 8) ≤ 65407` не переносится в соседнюю.
#[inline(always)]
fn div255_pair(y: u32) -> u32 {
	debug_assert!((y & 0xFFFF) < 65535 && (y >> 16) < 65535);
	((y + 0x0001_0001 + ((y >> 8) & RB)) >> 8) & RB
}

/// `mul` для пары каналов в половинах `x` (каждый ≤ 255).
#[inline(always)]
fn mul_pair(x: u32, a: u32) -> u32 {
	div255_pair(x * a + ROUND_PAIR)
}

/// Смешивание по покрытию (§7.3, ADR-0002): `c = (bg·(255 − a) + fg·a + 127) / 255` по каналам,
/// целочисленно — та же формула в шейдере даёт бит-идентичный результат. R и B — парой.
#[inline(always)]
pub fn blend(bg: u32, fg: u32, a: u8) -> u32 {
	match a {
		0 => bg,
		COVERAGE_FULL => fg,
		_ => {
			let a = a as u32;
			let inv = 255 - a;
			let rb = div255_pair((bg & RB) * inv + (fg & RB) * a + ROUND_PAIR);
			let g = div255(((bg >> 8) & 0xFF) * inv + ((fg >> 8) & 0xFF) * a + 127);
			rb | (g << 8)
		}
	}
}

/// Покрытие с учётом альфы символа (ADR-0013): `(cov · alpha + 127) / 255`, целочисленно — как
/// в шейдере. При `alpha = 255` покрытие не меняется, при `alpha = 0` символа нет.
#[inline(always)]
pub fn fade(coverage: u8, alpha: u8) -> u8 {
	match alpha {
		0 => 0,
		COVERAGE_FULL => coverage,
		_ => div255(coverage as u32 * alpha as u32 + 127) as u8,
	}
}

/// `(x · a + 127) / 255` — умножение канала на долю 0…255 с округлением (ADR-0002).
#[inline(always)]
fn mul(x: u32, a: u32) -> u32 {
	div255(x * a + 127)
}

/// Пиксель с цветом `rgb` (`0x00RRGGBB`) и альфой `alpha` поверх пикселя текстуры наложения `t`
/// (`0xAARRGGBB`, RGB предумножен на альфу; ADR-0014): `t' = mul(c, α) + mul(t, 255 − α)` по
/// каналам и альфе. `α = 255` даёт ровно `rgb`, `α = 0` не меняет `t`.
///
/// Каналы считаются парами (R, B) и (A, G). Альфа слоя — канал со значением 255:
/// `mul(255, α) = α`, так что `t_a' = α + mul(t_a, 255 − α)` — та же формула. Сумма в половине
/// не больше `mul(255, α) + mul(255, 255 − α) = 255`.
#[inline(always)]
pub fn over(t: u32, rgb: u32, alpha: u8) -> u32 {
	match alpha {
		0 => t,
		COVERAGE_FULL => rgb | 0xFF00_0000,
		_ => {
			let a = alpha as u32;
			let inv = 255 - a;
			let rb = mul_pair(rgb & RB, a) + mul_pair(t & RB, inv);
			let layer_ag = 0x00FF_0000 | ((rgb >> 8) & 0xFF);
			let ag = mul_pair(layer_ag, a) + mul_pair((t >> 8) & RB, inv);
			(ag << 8) | rb
		}
	}
}

/// Пиксель текстуры наложения `t` (предумноженный) поверх пикселя кадра `frame` (`0x00RRGGBB`):
/// `out = t + mul(frame, 255 − t_a)` по каналам. Пустая текстура (`t = 0`) кадр не меняет.
/// R и B — парой; канал не переполняется, потому что `t_c ≤ t_a` (предумножение).
#[inline(always)]
pub fn composite(frame: u32, t: u32) -> u32 {
	let inv = 255 - (t >> 24);
	match inv {
		255 => frame,
		0 => t & 0x00FF_FFFF,
		_ => {
			let rb = (t & RB) + mul_pair(frame & RB, inv);
			let g = ((t >> 8) & 0xFF) + mul((frame >> 8) & 0xFF, inv);
			rb | (g << 8)
		}
	}
}

/// Непрозрачный старший байт пикселя кадра `0x00RRGGBB` (ADR-0024).
pub const OPAQUE_BYTE: u32 = 0xFF00_0000;

/// Копия пикселей кадра в буфер окна с непрозрачным старшим байтом (ADR-0024): KWin читает
/// буфер `Xrgb8888` как ARGB с предумноженной альфой, и нулевой байт даёт сложение кадров.
/// Длины срезов равны — иначе паника до записи (Fail Fast).
pub fn copy_opaque(dst: &mut [u32], src: &[u32]) {
	assert_eq!(dst.len(), src.len(), "copy_opaque: slice lengths differ");
	for (d, s) in dst.iter_mut().zip(src) {
		*d = *s | OPAQUE_BYTE;
	}
}

/// BOLD как осветление (§7.3): каждый канал `min(c + 0x55, 255)`, целочисленно — как в шейдере.
pub(crate) fn lighten(color: u32) -> u32 {
	let r = ((color >> 16) & 0xFF) + BOLD_BOOST;
	let g = ((color >> 8) & 0xFF) + BOLD_BOOST;
	let b = (color & 0xFF) + BOLD_BOOST;
	(r.min(255) << 16) | (g.min(255) << 8) | b.min(255)
}
