//! Попиксельные функции Software считают каналы R и B парой в одном 32-битном слове (ADR-0015).
//! Эталон — прежние поканальные формулы ADR-0002 / ADR-0013 / ADR-0014 с делением как есть, как в
//! шейдере. Перебор: все входы одного канала, записанные во все каналы сразу, плюс псевдослучайные
//! пиксели с разными каналами — проверка, что половины пары не влияют друг на друга.

use gterm::renderer::pixel::{OPAQUE_BYTE, blend, composite, copy_opaque, over};

/// `(x·a + 127) / 255`.
fn mul(x: u32, a: u32) -> u32 {
	(x * a + 127) / 255
}

fn channel(px: u32, shift: u32) -> u32 {
	(px >> shift) & 0xFF
}

fn blend_reference(bg: u32, fg: u32, a: u8) -> u32 {
	let a = a as u32;
	let mix = |shift| (channel(bg, shift) * (255 - a) + channel(fg, shift) * a + 127) / 255;
	(mix(16) << 16) | (mix(8) << 8) | mix(0)
}

fn over_reference(t: u32, rgb: u32, alpha: u8) -> u32 {
	let a = alpha as u32;
	let ch = |shift| mul(channel(rgb, shift), a) + mul(channel(t, shift), 255 - a);
	((a + mul(t >> 24, 255 - a)) << 24) | (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

fn composite_reference(frame: u32, t: u32) -> u32 {
	let inv = 255 - (t >> 24);
	let ch = |shift| channel(t, shift) + mul(channel(frame, shift), inv);
	(ch(16) << 16) | (ch(8) << 8) | ch(0)
}

/// Значение канала во всех трёх каналах цвета.
fn gray(c: u32) -> u32 {
	c * 0x01_0101
}

/// Детерминированный генератор (LCG) — тест воспроизводим.
struct Lcg(u64);

impl Lcg {
	fn next(&mut self) -> u32 {
		self.0 = self
			.0
			.wrapping_mul(6_364_136_223_846_793_005)
			.wrapping_add(1_442_695_040_888_963_407);
		(self.0 >> 32) as u32
	}
}

const RANDOM_PIXELS: usize = 1 << 20;

#[test]
fn blend_matches_the_per_channel_formula_for_every_input() {
	for a in 0..=255u8 {
		for b in 0..=255 {
			for f in 0..=255 {
				let (bg, fg) = (gray(b), gray(f));
				assert_eq!(blend(bg, fg, a), blend_reference(bg, fg, a), "{b} {f} {a}");
			}
		}
	}
	let mut rng = Lcg(1);
	for _ in 0..RANDOM_PIXELS {
		let (bg, fg, a) = (
			rng.next() & 0xFF_FFFF,
			rng.next() & 0xFF_FFFF,
			rng.next() as u8,
		);
		assert_eq!(blend(bg, fg, a), blend_reference(bg, fg, a));
	}
}

#[test]
fn over_matches_the_per_channel_formula_for_every_input() {
	for alpha in 0..=255u8 {
		for x in 0..=255 {
			// Текстура — то же значение во всех четырёх каналах, включая альфу.
			let t = x * 0x0101_0101;
			for c in 0..=255 {
				let rgb = gray(c);
				assert_eq!(
					over(t, rgb, alpha),
					over_reference(t, rgb, alpha),
					"{x} {c} {alpha}"
				);
			}
		}
	}
	let mut rng = Lcg(2);
	for _ in 0..RANDOM_PIXELS {
		let (t, rgb, alpha) = (rng.next(), rng.next() & 0xFF_FFFF, rng.next() as u8);
		assert_eq!(over(t, rgb, alpha), over_reference(t, rgb, alpha));
	}
}

/// Текстура наложения предумножена: канал не больше альфы (`t_c ≤ t_a`) — на других входах
/// формула переполняет канал и не определена.
#[test]
fn composite_matches_the_per_channel_formula_for_every_input() {
	for t_a in 0..=255u32 {
		for x in 0..=t_a {
			let t = (t_a << 24) | gray(x);
			for f in 0..=255 {
				let frame = gray(f);
				assert_eq!(
					composite(frame, t),
					composite_reference(frame, t),
					"{t:08X} {f}"
				);
			}
		}
	}
	let mut rng = Lcg(3);
	for _ in 0..RANDOM_PIXELS {
		let t_a = rng.next() & 0xFF;
		let premultiplied = |c: u32| (c & 0xFF) % (t_a + 1);
		let t = (t_a << 24)
			| (premultiplied(rng.next()) << 16)
			| (premultiplied(rng.next()) << 8)
			| premultiplied(rng.next());
		let frame = rng.next() & 0xFF_FFFF;
		assert_eq!(composite(frame, t), composite_reference(frame, t));
	}
}

/// Копия в буфер окна Wayland (ADR-0024): цвет без изменений, старший байт `0xFF` у каждого
/// пикселя — и у нулевого, и у уже непрозрачного; псевдослучайные пиксели с любым старшим байтом.
#[test]
fn copy_opaque_sets_top_byte_and_keeps_color() {
	let mut rng = Lcg(7);
	let src: Vec<u32> = std::iter::once(0)
		.chain(std::iter::once(0xFF00_0000))
		.chain((0..RANDOM_PIXELS).map(|_| rng.next()))
		.collect();
	let mut dst = vec![0x1234_5678; src.len()];
	copy_opaque(&mut dst, &src);
	for (d, s) in dst.iter().zip(&src) {
		assert_eq!(d & 0x00FF_FFFF, s & 0x00FF_FFFF, "цвет изменился");
		assert_eq!(d & OPAQUE_BYTE, OPAQUE_BYTE, "старший байт не 0xFF");
	}
}

/// Разные длины срезов — паника до записи (Fail Fast), а не тихая копия части.
#[test]
#[should_panic(expected = "slice lengths differ")]
fn copy_opaque_panics_on_length_mismatch() {
	let mut dst = [0u32; 3];
	copy_opaque(&mut dst, &[1, 2]);
}
