//! Этап 6 (ТЗ FR-20, FR-21, FR-23, FR-25; ADR-0004): события приложения на
//! настоящем окне в режиме [`GridMode::FixedGrid`].
//!
//! Чистые функции геометрии проверяет `stage6_app`, режим `AutoGrid` —
//! `stage6_events_auto`. Здесь проверяется то, что осталось между ними: окно создаётся, бэкенд
//! выбирается, обработчик получает уведомления, `Request` применяется, `ctx.exit()` останавливает
//! цикл.
//!
//! Тест открывает настоящее окно, поэтому нужен дисплей. Без дисплея он печатает причину и
//! проходит — как и `stage4_gl` (§11.2): это пропуск, а не падение.
//!
//! У бинарника свой `main` (`harness = false` в `Cargo.toml`): цикл событий `winit` обязан быть
//! создан на главном потоке, а обычный harness запускает тесты в отдельных потоках. Второй
//! запуск в том же процессе невозможен — `winit` разрешает цикл событий только один.

mod events_common;

use events_common::{
	WANTED, assert_frame_stats, assert_kept_drawing, assert_known_backends, run_once,
};
use gterm::app::{Grid, GridMode};
use gterm::config::Config;
use gterm::font::FONT_SIZE_PX;

fn main() {
	font_size_follows_dpi_within_bounds();
	println!("размер глифа следует DPI и остаётся в границах: ok");

	if fixed_grid_accepts_the_requested_grid() {
		println!("FixedGrid: запрошенная сетка применена: ok");
	}
}

/// Размер глифа обязан следовать DPI и оставаться в границах 8..=48 (ADR-0005 п.2); окно при
/// этом создаётся логическим размером, физический подставляет система (ADR-0005 п.5).
fn font_size_follows_dpi_within_bounds() {
	let at_1 = gterm::font::glyph_px(1.0);
	let at_2 = gterm::font::glyph_px(2.0);
	assert!(
		at_2 > at_1,
		"DPI должен увеличивать размер глифа: {at_1} → {at_2}"
	);
	for factor in [0.1, 0.5, 1.0, 1.25, 1.75, 2.0, 3.0, 8.0, 100.0] {
		let px = gterm::font::glyph_px(factor);
		assert!(
			(8..=48).contains(&px),
			"glyph_px({factor}) = {px} вне границ 8..=48"
		);
	}
	// Негодный DPI даёт базовый размер: `round` от NaN дал бы 0 и панику в растеризации (NFR-7).
	assert_eq!(
		gterm::font::glyph_px(f64::NAN),
		FONT_SIZE_PX,
		"NaN → базовый"
	);
	assert_eq!(gterm::font::glyph_px(0.0), FONT_SIZE_PX, "ноль → базовый");
	assert_eq!(
		gterm::font::glyph_px(-2.0),
		FONT_SIZE_PX,
		"отрицательный → базовый"
	);
}

/// `FixedGrid`: сетку задаёт приложение, окно на неё не влияет (FR-23).
fn fixed_grid_accepts_the_requested_grid() -> bool {
	let Some(log) = run_once(Config::default(), GridMode::FixedGrid, None) else {
		return false;
	};
	let log = log.lock().expect("log");
	assert_eq!(
		assert_known_backends(&log).len(),
		1,
		"отката не было, а уведомлений: {:?}",
		log.backends
	);
	let backend = log.backends[0];

	assert_eq!(
		log.last_grid,
		Some(WANTED),
		"запрошенная сетка не применена"
	);
	assert_eq!(
		log.resized_from,
		vec![Grid::new(24, 6)],
		"уведомление о сетке должно прийти один раз со старым размером"
	);
	assert_eq!(
		log.marker_kept,
		Some(true),
		"содержимое не пережило смену сетки"
	);
	assert_kept_drawing(&log);
	assert_frame_stats(&log);
	// ADR-0026: размер ячейки и модификаторы приходят в `Ctx`. Ввод с окна тест не инжектирует,
	// поэтому проверяется заполнение полей: ячейка — метрики шрифта (ширина ≤ высоты, обе
	// ненулевые), модификаторы без нажатий — пусты.
	let cell_px = log.cell_px.expect("cell_px в Ctx");
	assert!(
		cell_px.0 >= 1 && cell_px.1 >= cell_px.0,
		"размер ячейки в Ctx не похож на метрики моноширинного шрифта: {cell_px:?}"
	);
	assert_eq!(
		log.modifiers_empty,
		Some(true),
		"модификаторы без нажатий должны быть пусты"
	);
	eprintln!(
		"FixedGrid: бэкенд {backend}, {} кадров, сетка {WANTED:?}, ячейка {cell_px:?}",
		log.frames
	);
	true
}
