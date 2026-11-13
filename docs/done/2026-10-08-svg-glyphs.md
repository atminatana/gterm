# Выполнение: SVG-глифы и размер шрифта демо-бинарников

- План: `docs/plans/2026-10-08-svg-glyphs.md`; ТЗ 1.7 (FR-8, FR-8.6, FR-9, §1.1, §13); ПОЧЕМУ — `docs/adr/0008-svg-glyphs.md`. Зависит от `docs/done/2026-10-08-codepoint-u32.md` (ADR-0009).

## WHAT

| Файл | Изменение |
|---|---|
| `Cargo.toml` | `usvg = { version = "0.48", default-features = false }` (без текста, системных шрифтов, svgz, writer) |
| `src/svg.rs` (новый) | `SvgGlyphs::load(dir)`, `codepoint_from_file_name`, `svg_dir`, `SvgError`; разбор `usvg` → контуры на холсте (`abs_transform`, заливка с правилом файла, обводка и штрихи → заливка `tiny-skia-path`); `rasterize`: вписывание с сохранением пропорций, центр, `zeno` по путям, объединение по максимуму покрытия; буферы переиспользуются |
| `src/lib.rs` | модуль `svg` |
| `src/rasterizer.rs` | поле `svg`; `set_svg_glyphs`, `svg_glyphs`, `has_font_glyph`, `is_style_independent`; `has_glyph`/`rasterize` — SVG первым |
| `src/atlas.rs` | `fill_channel`: все 4 канала, если `is_style_independent` (SVG или псевдографика) |
| `src/app.rs` | `AppOptions::font_size_px` (по умолчанию `FONT_SIZE_PX`) вместо константы в трёх местах |
| `src/bin/demo/common.rs` | `DEMO_FONT_SIZE_PX = 26`; `load_family(font_size_px)`; `load_rasterizer()` — шрифт сразу на 26 px + SVG + лог перекрытий; `demo_options` ставит 26 |
| `src/bin/demo/{main,bench}.rs`, `src/bin/full_table/main.rs` | `demo`/`full_table` — `load_rasterizer`; `--bench` — `load_family(FONT_SIZE_PX)`, 16 px; `full_table` добавляет SVG-глифы в перечень |
| `src/bin/demo/scene.rs` | фрагмент карты 24×7 из тайлов в колонках 52–75, строках 2–8 (на узкой сетке не рисуется) |
| `assets/svg/100000…100003.svg` (новые) | тайлы: трава, вода, стена, дерево (`viewBox 0 0 10 20`) |
| `tests/svg_glyphs.rs`, `tests/data/svg/*` (новые), `tests/common/mod.rs`, `tests/stage4_gl.rs`, `tests/events_common/mod.rs` | тесты; сцена `svg 26px`; `..AppOptions::default()` в литерале |
| ТЗ 1.7, `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md`, план; `docs/research/2026-10-08-svg-map.png` | обновлены; кадр карты Software-рендерера |

## COMPLETENESS

| Требование (ADR-0008 / FR-8.6) | Код | Тест |
|---|---|---|
| п.1: `usvg` → контуры, `zeno` | `svg.rs` `parse`/`push_path`/`rasterize` | `rasterization::*` |
| п.2: имя — 4–6 HEX, иначе пропуск | `codepoint_from_file_name` | `file_names::*` (12 отказов, 6 допустимых) |
| п.2: битый файл, чужое имя, повтор → stderr и пропуск | `SvgGlyphs::load_file` | `loading::valid_files_load_and_bad_ones_are_skipped_without_panic`, `first_file_name_wins_on_duplicate_codepoint` |
| п.2: нет каталога — не ошибка | `SvgGlyphs::load` | `loading::missing_directory_gives_empty_set` |
| п.3: SVG > псевдографика > шрифт | `Rasterizer::has_glyph`/`rasterize` | `priority::svg_overrides_font_glyph_and_box_drawing`, `font_glyphs_without_svg_are_unchanged` |
| п.4: вписывание с сохранением пропорций, центр | `SvgGlyphs::rasterize` | `square_canvas_keeps_proportions_and_is_centered` (полосы 5 px, точные 0/255) |
| п.5: цвета игнорируются, `fill="none"` учитывается, `transform`, `evenodd` | `push_path` | `colors_are_ignored_and_fill_none_is_respected`, `group_transform_is_applied`, `even_odd_rule_cuts_a_hole` |
| п.6: все 4 канала; GL ≡ Software | `Atlas::fill_channel` | `atlas::svg_glyph_fills_every_style_channel_at_once`; `stage4_gl` сцена `svg 26px` (флаги `BOLD`/`ITALIC`/`INVERSE`) |
| п.7: переживает смену DPI | поле в `Rasterizer` | `atlas::svg_glyphs_survive_size_change_in_place` |
| п.8: `AppOptions::font_size_px`, демо 26, `--bench` 16 | `app.rs`, `common.rs` | `stage6_*` (размер по умолчанию не изменился); запуск `demo`: `logical 26px → cell 16x34`, `glyph_px 26` |
| Карта из SVG | `scene.rs`, `assets/svg/` | кадр Software-рендерера `docs/research/2026-10-08-svg-map.png` |

## CORRECTNESS

Тесты растра используют ячейку 10×20 и квадратный холст: вписывание даёт целые границы (полосы ровно 5 px), поэтому утверждения — точные 0/255, а не «есть чернила». Тест приоритета сравнивает выход растеризатора с выходом отдельного `SvgGlyphs` и проверяет, что он **отличается** от процедурной линии U+2500. Тест повтора кодовой точки устроен так, что проигравший файл — пустой холст: при неверном выборе центр был бы 0. Предупреждения загрузчика проверены в выводе (`--nocapture`): повтор, битый XML, неверное имя — по строке на файл.

## EVIDENCE

- `cargo fmt --check` ✓; `cargo clippy --all-targets -- -D warnings` ✓ (0 предупреждений); `cargo test` — **121 тест** (stage1 11, stage2 44, stage3 20, stage4 1, stage5 8, stage6_app 9, stage6_blink 7, full_table 6, svg_glyphs 15) + 4 оконных бинарника (`stage8_faults` — 5 сценариев) — всё ok.
- `stage4_gl --nocapture`: `OpenGL 3.3: GL ↔ Software identical, 6 scenes`, `OpenGL ES 3.0: GL ↔ Software identical, 6 scenes` (NVIDIA GTX 1060, драйвер 581.57).
- Запуски (debug, 5–6 с по таймауту): `demo` — `logical 26px → cell 16x34, baseline 27; glyph_px at dpi 1.0 = 26`, `svg: 4 glyphs from …\assets\svg (no overrides)`, бэкенд OpenGL 3.3 с `glyph_px 26`, без перестройки хинтеров; `full_table` — `глифов: 11919` (11 915 шрифта + 4 SVG), 7 страниц.
- Кадр карты Software-рендерера при 26 px просмотрен: тайлы стыкуются, швы кладки и волны различимы, цвета — из ячеек (`docs/research/2026-10-08-svg-map.png`).
- Размер release (Windows, профиль §10.2): `demo.exe` 1 438 720 Б (было 1 080 832, **+357 888 Б, +33 %**), `full_table.exe` 1 433 600 Б (было 1 075 200).
- Зависимости `usvg` без фич: `roxmltree`, `svgtypes`, `simplecss`, `kurbo`, `tiny-skia-path`, `strict-num`, `siphasher`, `imagesize`, `data-url`, `log`, `pico-args` (`cargo tree -p usvg --depth 1`).
- LOC (QG-06): `svg.rs` 291 (новый: один модуль — загрузка и растеризация SVG-глифов), `full_table/main.rs` 412, `common.rs` 203; `app.rs` 1110 — порог превышен **до** задачи (1106), +4 строки.

## ⚠ WARNINGS

- Глазами в настоящем окне не проверено (окна открывались на 5–6 с): карта в `demo`, SVG в `full_table`, окно `full_table` 80×45 при ячейке 16×34 (~1280×1530 логических px — выше экрана 1080p; `AutoGrid` подстроится, но положение окна не проверено) — пункты в `TODO.md`.
- Размер release вырос на 33 % из-за `usvg` (XML/CSS/path-парсеры) — лимита нет (NFR-1), но если размер важен, альтернатива из ADR-0008 (свой разбор `<path>`) остаётся.
- Не поддерживаются (по ADR-0008): цвета, прозрачность, `<image>`, текст, маски, `clip-path`, фильтры — тихо игнорируются, без сообщения в stderr.
- `app.rs` (1110 строк) превышает порог QG-06 с прошлых этапов — не делился (BP-07.2); предлагается отдельной задачей.
- Изменения не закоммичены — коммит по запросу владельца.
