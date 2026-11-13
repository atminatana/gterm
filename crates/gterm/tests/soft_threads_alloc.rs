//! NFR-6 для многопоточной отрисовки Software (ADR-0016): после разогрева кадр не выделяет память
//! ни в одном, ни в нескольких потоках. Счётчик — глобальный аллокатор этого тест-бинарника,
//! поэтому тест в бинарнике один: параллельные тесты исказили бы счёт.

mod common;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use common::repo_atlas;
use gterm::buffer::ScreenBuffer;
use gterm::cell::Color;
use gterm::overlay::Overlay;
use gterm::renderer::Renderer;
use gterm::renderer::soft_backend::SoftBackend;

struct CountingAllocator;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

// SAFETY: делегирует системному аллокатору без изменений, только считает вызовы.
unsafe impl GlobalAlloc for CountingAllocator {
	unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
		ALLOCATIONS.fetch_add(1, Relaxed);
		// SAFETY: контракт `alloc` передаётся системному аллокатору как есть.
		unsafe { System.alloc(layout) }
	}

	unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
		// SAFETY: `ptr` выделен `System` через этот же аллокатор.
		unsafe { System.dealloc(ptr, layout) }
	}

	unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
		ALLOCATIONS.fetch_add(1, Relaxed);
		// SAFETY: `ptr` выделен `System` через этот же аллокатор.
		unsafe { System.realloc(ptr, layout, new_size) }
	}
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

const COLS: usize = 40;
const ROWS: usize = 20;
const WARMUP_FRAMES: usize = 3;
const MEASURED_FRAMES: usize = 20;

/// Кадры с полной перезаписью экрана и наложением, которое меняется каждый кадр: работают и
/// основной слой, и сборка текстуры.
fn allocations_in_frames(threads: usize) -> usize {
	let mut soft = SoftBackend::with_threads(repo_atlas(), threads);
	assert_eq!(soft.threads(), threads);
	let mut screen = ScreenBuffer::new(COLS, ROWS);
	let mut layer = ScreenBuffer::transparent(COLS, ROWS);
	let mut overlay = Overlay::new();
	let texts = ["abcdefgh", "ijklmnop"];
	let mut frame = |soft: &mut SoftBackend, n: usize| {
		for row in 0..ROWS {
			screen.write_row(row, texts[n % 2], Color::WHITE, Color::BLACK);
			layer.write_row(
				row,
				texts[(n + 1) % 2],
				Color::from_rgba(0xFF, 0, 0, 120),
				Color::TRANSPARENT,
			);
		}
		overlay.clear();
		overlay.push(&layer, (n % 3) as i32, 0);
		soft.render_overlaid(&mut screen, &overlay).unwrap();
	};
	for n in 0..WARMUP_FRAMES {
		frame(&mut soft, n);
	}
	let before = ALLOCATIONS.load(Relaxed);
	for n in WARMUP_FRAMES..WARMUP_FRAMES + MEASURED_FRAMES {
		frame(&mut soft, n);
	}
	ALLOCATIONS.load(Relaxed) - before
}

#[test]
fn frames_do_not_allocate_in_one_or_many_threads() {
	assert_eq!(allocations_in_frames(1), 0, "один поток");
	assert_eq!(allocations_in_frames(4), 0, "4 потока");
}
