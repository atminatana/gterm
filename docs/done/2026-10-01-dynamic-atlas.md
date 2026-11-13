# Выполнение: динамический атлас, начертания в каналах, Bold/Italic, `assets/fonts/`

- План: `docs/plans/2026-10-01-dynamic-atlas.md`; ПОЧЕМУ — `docs/adr/0003-dynamic-atlas-style-channels.md`.
- Этап «ТЗ» (`docs/spec/`) не заводился — постановка в ТЗ проекта (теперь версия 1.2).
- Решения владельца в ходе задачи: без файла кэша; `BOLD` = глиф + осветление; страница 2048²; подстановка начертаний по цепочке, нет Regular — ошибка; поиск `assets/` крейтом `find_folder`; шрифты в `assets/fonts/`.

## WHAT

| Файл | Изменение |
|---|---|
| `Cargo.toml` | + `find_folder 0.3` |
| `src/cell.rs` | `CellFlags::ITALIC` (бит 5); `Style` {Regular, Bold, Italic, BoldItalic} — из флагов, из признаков шрифта, номер канала; `Cell::style()` |
| `src/font.rs` | переписан: `FontFamily::load(assets)` читает `assets/fonts/*.ttf|otf`, фильтр по имени семейства `FONT_FAMILY`, начертание по `OS/2` (вес ≥ 600, ITALIC), цепочка подстановки `style_source`, `CellMetrics` по Regular; `find_assets_dir` через `find_folder` (от exe, затем от cwd); ошибки `NoRegularFace`, `Io`, `TtfParse` |
| `src/rasterizer.rs` | новый (из `font_gen.rs`): хинтер на начертание-источник (интерпретатор при наличии инструкций, иначе автохинтер; `Target::Mono`), растеризация в предвыделенный буфер ячейки через `zeno::Mask::size/offset/render_into`, псевдографика процедурно |
| `src/atlas.rs` | новый: страница `RGBA8` `page²`, `page_px_for_limit`, `ensure(cp, style)` (промах → слот, ленивый канал; нет глифа → слот 0 без выделения; переполнение → сброс), `slot_of`, `pixel(slot, x, y, style)`, `is_ready`, `take_dirty_slots`, `resets` |
| `src/renderer/soft_backend.rs` | `prepare_glyphs` до горячего пути; повтор при сбросе; `RenderError`, если экран не помещается; `pixel(..., style)` |
| `src/bin/demo.rs` | загрузка семейства с отчётом о подстановке, строки Bold/Italic/BoldItalic, таблица символов |
| `src/font_gen.rs`, `assets/*.fnt`, `.gitignore` | удалены / очищены |
| `assets/fonts/` | четыре TTF перенесены из `assets/` |
| `tests/stage2_font.rs` | переписан: семейство (4 начертания, подстановка, нет Regular, нет каталога, `find_assets_dir`), растеризация, псевдографика, атлас — 24 теста |
| `tests/stage3_render.rs` | начертания по каналам, сброс атласа посреди кадра, ошибка при нехватке страницы; golden-сцена с Bold/Italic/BoldItalic — 20 тестов |
| `tests/golden/soft_golden_1x.bin` | перегенерирован, просмотрен |
| ТЗ 1.2, `AGENTS.md`, `PROJECT_MAP.md`, ADR-0001 (частично заменён), индекс ADR | обновлены |

## COMPLETENESS

| Требование | Код | Тест |
|---|---|---|
| FR-3 `ITALIC`, `BOLD` = глиф + осветление | `cell.rs`, `soft_backend::draw_cell` | `bold_and_italic_flags_select_atlas_channels`, `bold_lightens_fg_…` |
| FR-8/8.1 семейство из `assets/fonts/`, `find_folder` | `FontFamily::load`, `find_assets_dir` | `loads_all_four_faces_by_fs_selection_not_by_name`, `find_assets_dir_locates_repo_assets_from_test_cwd`; запуск demo из `target/debug` и из `/tmp` |
| FR-8.2 начертание по `OS/2`, не по имени | `Style::from_traits(weight ≥ 600, style ≠ Normal)` | тот же тест (4 файла → 4 начертания) |
| FR-8.3 подстановка | `from_faces` | `missing_faces_fall_back_along_the_chain`, `fallback_style_renders_exactly_like_its_source` |
| FR-8.5 нет Regular / нет каталога → ошибка | `NoRegularFace`, `Io` | `missing_regular_is_an_error_even_if_other_faces_exist` |
| FR-9 динамическая страница, каналы, ленивость, слот 0, сброс, лимит | `atlas.rs` | 8 тестов `atlas::*`, `atlas_reset_mid_frame_still_yields_the_same_frame`, `screen_larger_than_atlas_page_is_an_error_not_garbage` |
| FR-8.4 хинтинг (интерпретатор/автохинтер), процедурная псевдографика во всех каналах | `rasterizer.rs` | `latin_and_cyrillic_have_ink_with_full_and_partial_coverage`, `box_drawing_comes_from_procedural_generator_in_every_style`, `procedural_glyph_is_ready_in_all_channels_at_once` |
| NFR-6 горячий путь без аллокаций; промах — переиспользуемые буферы | `prepare_glyphs` отделён от `draw_*`; `PathPen` и `scratch` переиспользуются | по чтению кода |
| §11.2 golden | `tests/golden/` | `software_frame_matches_golden_dump` |

## CORRECTNESS

- Пиксельные тесты сравнивают кадр с `blend(bg, fg, покрытие канала)` для каждого из четырёх начертаний.
- Тест сброса посреди кадра: страница на 36 слотов, 30 заняты чужими глифами, экран на 20 — кадр побайтно равен кадру с большой страницей.
- Golden просмотрен как PNG: Bold жирнее и светлее, Italic наклонён, BoldItalic — оба, курсор, псевдографика.
- Подстановка проверена на подмножествах файлов во временных каталогах; `fallback` даёт байт-в-байт растр источника.

## EVIDENCE

См. `docs/test/2026-10-01-dynamic-atlas.md`, `docs/verify/2026-10-01-dynamic-atlas.md`.

## ⚠ WARNINGS

- Экран, где число разных глифов превышает ёмкость страницы, — `RenderError`, а не LRU. При `page ≥ 1024` ёмкость (4896 при 10×21) больше всего BMP-покрытия шрифта (3457), поэтому недостижимо; при смене шрифта на более крупный размер ячейки пересчитать.
- Промах кэша аллоцирует только при росте буфера контура (`PathPen`); `FontRef::new` и `outline_glyphs()` на каждый промах — разбор заголовков таблиц, без аллокаций, но это цена ≈ микросекунд на глиф сверх растеризации.
- Память: страница 2048² RGBA = 16 МБ в Software-рендерере; тесты используют 1024² (4 МБ) и меньше.
- `cargo test` временно создаёт каталоги `%TEMP%/gterm-stage2-*` с копиями TTF (2 МБ каждая) и удаляет их в `Drop`.
- GL-заглушка грузит атлас как `RGBA8` целиком — этап 4 заменит на `RGBA8UI` + `take_dirty_slots`.
- `find_folder` ищет от exe вверх до 4 уровней: из `target/debug/` находит `assets/` проекта; если у поставляемого exe рядом нет `assets/`, но он лежит внутри чужого дерева с `assets/` — найдётся чужой. Поведение осознанное (владелец выбрал крейт); при поставке класть `assets/` рядом с exe.
- `cargo bloat`: `find_folder` в бинарнике незаметен; release 919 552 Б с профилем §10.2.
