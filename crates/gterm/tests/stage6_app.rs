//! Этап 6 (ТЗ FR-23, FR-24, FR-26; ADR-0004, ADR-0005): геометрия сетки и смена DPI.
//!
//! Проверяются чистые функции расчёта и путь смены размера растеризации на месте: результат
//! обязан совпасть с растеризатором, собранным заново на том же размере, — иначе перенос окна
//! между мониторами менял бы картинку.

mod common;

use gterm::app::{Grid, auto_grid, centered_origin, pointer_cell};
use gterm::atlas::{Atlas, PAGE_PX_MIN};
use gterm::buffer::ScreenBuffer;
use gterm::cell::Color;
use gterm::font::{GLYPH_PX_MAX, GLYPH_PX_MIN};
use gterm::renderer::Renderer;
use gterm::renderer::soft_backend::SoftBackend;

use common::{
	BIG_FONT_PX, GOLDEN_H, GOLDEN_W, RenderWith, big_background, golden_scene, repo_atlas,
	repo_atlas_at, repo_rasterizer_at,
};

/// Сетка по окну: целые ячейки, но не меньше 1×1 (FR-23 `AutoGrid`).
mod grid_layout {
	use super::*;

	#[test]
	fn window_fits_whole_cells_and_never_less_than_one() {
		assert_eq!(auto_grid((250, 140), (10, 21)).cols, 25);
		assert_eq!(auto_grid((250, 140), (10, 21)).rows, 6);
		// Ровно по границе: последняя строка впритык.
		assert_eq!(auto_grid((30, 42), (10, 21)), Grid::new(3, 2));
		// Остаток в несколько пикселей не даёт неполной ячейки.
		assert_eq!(auto_grid((39, 62), (10, 21)), Grid::new(3, 2));
		assert_eq!(auto_grid((40, 63), (10, 21)), Grid::new(4, 3));
		// Окно меньше ячейки — одна ячейка, а ноль строк ломает бы рендерер.
		assert_eq!(auto_grid((3, 3), (10, 21)), Grid::new(1, 1));
		assert_eq!(auto_grid((0, 0), (10, 21)), Grid::new(1, 1));
	}

	#[test]
	fn degenerate_cell_size_does_not_divide_by_zero() {
		// Метрики никогда не нулевые (`cell_metrics` это проверяет), но расчёт должен быть
		// безопасным и при вырожденном входе (NFR-7).
		assert_eq!(auto_grid((250, 140), (0, 0)), Grid::new(250, 140));
		assert_eq!(auto_grid((250, 140), (0, 21)), Grid::new(250, 6));
	}

	#[test]
	fn bigger_font_means_fewer_cells_in_the_same_window() {
		// Смысл FR-24: DPI меняет размер ячейки, а не окно.
		let small = repo_atlas();
		let big = repo_atlas_at(BIG_FONT_PX);
		let window = (GOLDEN_W, GOLDEN_H);
		let a = auto_grid(window, (small.glyph_w(), small.glyph_h()));
		let b = auto_grid(window, (big.glyph_w(), big.glyph_h()));
		assert!(b.cols < a.cols && b.rows < a.rows, "{a:?} → {b:?}");
	}

	#[test]
	fn origin_centers_the_grid_and_never_goes_negative() {
		// Сетка меньше окна — отступы поровну.
		assert_eq!(centered_origin((240, 138), (250, 140)), (5, 1));
		// Сетка больше окна (FixedGrid): смещение 0, лишнее обрезается по границе окна (FR-23).
		assert_eq!(centered_origin((480, 252), (250, 140)), (0, 0));
		assert_eq!(centered_origin((250, 140), (250, 140)), (0, 0));
	}

	/// Ячейка под указателем (ADR-0026): смещение сетки вычитается, деление с округлением вниз,
	/// слева и сверху от сетки — отрицательные значения, негодные координаты — `(-1, -1)`.
	#[test]
	fn pointer_cell_subtracts_origin_and_floors() {
		let cell = (10, 21);
		let origin = (5, 1);
		// Левый верхний пиксель сетки и последний пиксель той же ячейки.
		assert_eq!(pointer_cell((5.0, 1.0), origin, cell), (0, 0));
		assert_eq!(pointer_cell((14.9, 21.9), origin, cell), (0, 0));
		// Следующая ячейка начинается ровно на границе.
		assert_eq!(pointer_cell((15.0, 22.0), origin, cell), (1, 1));
		// Поле слева и сверху от сетки — отрицательные индексы, а не ячейка 0.
		assert_eq!(pointer_cell((4.9, 0.0), origin, cell), (-1, -1));
		assert_eq!(pointer_cell((0.0, 0.0), origin, cell), (-1, -1));
		// Без смещения и до бэкенда (ячейка 1×1) — просто пиксели.
		assert_eq!(pointer_cell((7.0, 3.0), (0, 0), (1, 1)), (7, 3));
		// Вырожденная ячейка не делит на ноль (NFR-7).
		assert_eq!(pointer_cell((7.0, 3.0), (0, 0), (0, 0)), (7, 3));
		// Негодные координаты.
		assert_eq!(pointer_cell((f64::NAN, 3.0), origin, cell), (-1, 0));
		assert_eq!(pointer_cell((f64::INFINITY, 3.0), origin, cell), (-1, 0));
	}
}

/// Смена DPI: атлас пересобирается на месте и рисует ровно то же, что растеризатор, собранный
/// заново на этом размере (FR-24, ADR-0005 п.7).
mod dpi_change {
	use super::*;

	/// Окно golden для крупного шрифта: как в `stage3_render::golden`.
	fn big_window(size_px: u16) -> (u32, u32) {
		let m = repo_rasterizer_at(size_px).metrics();
		(24 * m.glyph_w as u32 + 21, 6 * m.glyph_h as u32 + 10)
	}

	#[test]
	fn set_size_matches_a_rasterizer_built_from_scratch() {
		for size in [GLYPH_PX_MIN, BIG_FONT_PX, GLYPH_PX_MAX] {
			let rasterizer = repo_rasterizer_at(size);
			let mut changed = Atlas::new(repo_rasterizer_at(16), PAGE_PX_MIN);
			changed.set_size(size).unwrap();
			let mut fresh = Atlas::new(rasterizer, PAGE_PX_MIN);

			assert_eq!(changed.glyph_w(), fresh.glyph_w(), "size {size}");
			assert_eq!(changed.glyph_h(), fresh.glyph_h(), "size {size}");
			assert_eq!(changed.capacity(), fresh.capacity(), "size {size}");
			assert_eq!(
				changed.page_px(),
				fresh.page_px(),
				"страница не перевыделяется"
			);

			// Пиксели глифа совпадают: хинтеры перестроены так же, как при сборке с нуля.
			for (cp, style) in [
				('A', gterm::cell::Style::Regular),
				('g', gterm::cell::Style::Bold),
				('щ', gterm::cell::Style::Italic),
			] {
				let a = changed.ensure(cp, style);
				let b = fresh.ensure(cp, style);
				assert_eq!(a, b, "слот {cp} size {size}");
				assert_eq!(
					changed.glyph_bytes(a, style),
					fresh.glyph_bytes(b, style),
					"глиф {cp} size {size}"
				);
			}
		}
	}

	#[test]
	fn set_size_to_the_same_value_changes_nothing() {
		let mut atlas = repo_atlas();
		let slot = atlas.ensure('A', gterm::cell::Style::Regular);
		let bytes = atlas
			.glyph_bytes(slot, gterm::cell::Style::Regular)
			.to_vec();
		// Счётчик берётся после растеризации: она сама могла сбросить страницу.
		let resets = atlas.resets();
		atlas.set_size(atlas.size_px()).unwrap();
		assert_eq!(atlas.resets(), resets, "слоты остаются в силе");
		assert_eq!(atlas.slot_of('A'), Some(slot));
		assert_eq!(
			atlas.glyph_bytes(slot, gterm::cell::Style::Regular),
			bytes.as_slice()
		);
	}

	#[test]
	fn frame_after_a_dpi_change_equals_the_reference_dump() {
		// Эталон `soft_golden_32px.bin` снят с растеризатора, собранного заново на 32 px.
		// Смена DPI с 16 px на 32 px обязана дать тот же кадр байт в байт.
		let (w, h) = big_window(BIG_FONT_PX);
		let mut backend = SoftBackend::new(repo_atlas());
		backend.resize(w, h).unwrap();
		backend.set_size(BIG_FONT_PX).unwrap();
		backend
			.render_with(&mut golden_scene(), &big_background())
			.unwrap();
		assert_atlas_geometry(&backend, BIG_FONT_PX);

		let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("tests/golden/soft_golden_32px.bin");
		let expected: Vec<u8> = std::fs::read(&path).expect("golden 32px");
		let actual: Vec<u8> = backend
			.frame()
			.iter()
			.flat_map(|p| p.to_le_bytes())
			.collect();
		assert_eq!(expected.len(), actual.len());
		let diff = expected
			.iter()
			.zip(&actual)
			.position(|(a, b)| a != b)
			.map(|i| i / 4);
		assert_eq!(diff, None, "кадр после смены DPI отличается от эталона");
	}

	#[test]
	fn grid_is_recalculated_after_a_dpi_change_in_the_same_window() {
		// Окно то же, ячейка крупнее — ячеек меньше; содержимое в пересечении сохраняется.
		let mut screen = ScreenBuffer::new(40, 10);
		screen.write_row(0, "остаётся", Color::WHITE, Color::BLACK);
		let mut backend = SoftBackend::new(repo_atlas());
		backend.resize(40 * 10, 10 * 21).unwrap();
		backend.render_with(&mut screen, &big_background()).unwrap();
		let before = auto_grid((40 * 10, 10 * 21), (10, 21));
		backend.set_size(BIG_FONT_PX).unwrap();
		backend.render_with(&mut screen, &big_background()).unwrap();
		let m = repo_rasterizer_at(BIG_FONT_PX).metrics();
		let after = auto_grid((40 * 10, 10 * 21), (m.glyph_w, m.glyph_h));
		assert!(after.cols < before.cols, "{before:?} → {after:?}");
		assert_eq!(screen.cell(0, 0).ch, 'о', "содержимое сохранилось");
	}

	fn assert_atlas_geometry(backend: &SoftBackend, size_px: u16) {
		assert_eq!(backend.atlas().size_px(), size_px);
		let m = repo_rasterizer_at(size_px).metrics();
		assert_eq!(backend.atlas().glyph_w(), m.glyph_w);
		assert_eq!(backend.atlas().glyph_h(), m.glyph_h);
	}
}

/// Окно golden остаётся эталоном эталонного рендерера: сцена без курсора (ADR-0004).
#[test]
fn golden_scene_has_no_cursor_in_the_library() {
	let mut backend = SoftBackend::new(repo_atlas_at(16));
	backend.resize(GOLDEN_W, GOLDEN_H).unwrap();
	let view = common::BORDER;
	backend.render_with(&mut golden_scene(), &view).unwrap();
	// Строка 5 содержит только текст приложения: курсор рисует приложение, не библиотека.
	let screen = golden_scene();
	assert_eq!(screen.cell(13, 5).ch, 'B');
}
