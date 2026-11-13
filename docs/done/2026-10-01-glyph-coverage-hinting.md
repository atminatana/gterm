# Выполнение: сглаживание глифов — 8-бит покрытие, хинтинг skrifa, процедурная псевдографика

- План: `docs/plans/2026-10-01-glyph-coverage-hinting.md`; ПОЧЕМУ — `docs/adr/0002-glyph-coverage-hinting.md`; факты — `docs/research/2026-10-01-glyph-alpha.md`.
- Этап «ТЗ» (`docs/spec/`) не заводился — постановка в ТЗ проекта (правки §1.2, FR-8, FR-8.3, FR-8.4, FR-11, §5.1, §6.3, §7.3, §10.1, §12, NFR-2).
- Попутно по решению владельца: `FONT_SIZE_PX = 16` (без изменений), цвета пустых ячеек (`ScreenBuffer::with_default_colors`, `DEFAULT_FG/BG`), рамка демо — чёрная, альфа цветов ячейки в 1.0 игнорируется (FR-2).

## WHAT

| Файл | Изменение |
|---|---|
| `Cargo.toml` | `fontdue` → `skrifa 0.47` (`std`) + `zeno 0.3` (`eval`, `std`) |
| `src/font.rs` | `FNT_FORMAT_VERSION = 2`; растр `coverage: Vec<u8>` (байт/пиксель), `coverage()`, `set_coverage()`, `slot_coverage_mut()`, `COVERAGE_FULL`; `Atlas` копирует строки покрытия |
| `src/font_gen.rs` | растеризация через `skrifa` (`HintingInstance`, `Engine::AutoFallback`, `Target::Mono`) → `zeno::Mask` (поправка на семантику `placement.top` при `BottomLeft`); таблица слотов = глифы шрифта ∪ U+2500–U+259F; процедурные слоты перекрывают шрифт |
| `src/box_drawing.rs` | новый: таблица сторон 128 кодов (сгенерирована из `unicodedata`, сверена по 17 кодам), руки light/heavy/double с правилами стыковки, диагонали, блоки/восьмые/тени/квадранты |
| `src/renderer/soft_backend.rs` | `a: u8` из атласа, `pub fn blend(bg, fg, a)` — `(bg·(255−a) + fg·a + 127)/255`; UNDERLINE/курсор `255`, BLINK `0` |
| `src/buffer.rs` | `DEFAULT_FG/BG`, `with_default_colors`, `default_colors()`; очистка/прокрутка/`resize`/`clear_region` используют `blank` |
| `src/bin/demo.rs` | рамка чёрная |
| `tests/stage1_buffer.rs` | +1 тест цветов по умолчанию, 1 переписан |
| `tests/stage2_font.rs` | переписан под покрытие: +6 тестов псевдографики, +тест «процедурный слот = генератор», AA-тест (есть полные и частичные пиксели) |
| `tests/stage3_render.rs` | ожидания через `blend` и покрытие атласа; +тесты `blend` (ручные значения) и частичного покрытия; golden-сцена дополнена `░▒▓ ╔═╦╗ ▚` |
| `tests/golden/soft_golden_1x.bin` | перегенерирован, проверен глазами |
| `AGENTS.md`, `PROJECT_MAP.md`, ТЗ, `docs/adr/README.md` | обновлены |

## COMPLETENESS

| Требование | Код | Тест |
|---|---|---|
| FR-8/8.3 растр 8 бит, формат v2 | `font.rs` | `round_trip_preserves_header_table_and_coverage`, `rejects_…` |
| FR-8.4 хинтинг + покрытие без порога, детерминизм | `font_gen::rasterize` | `latin_and_cyrillic_have_ink_with_partial_coverage…` (есть 255 и 1…254), `rasterization_is_deterministic` |
| FR-8.4 процедурная псевдографика перекрывает шрифт, весь диапазон присутствует | `box_drawing`, `codepoints_for_raster` | `box_drawing_comes_from_procedural_generator_not_font` |
| Геометрия линий/блоков (ADR-0002 п.4) | `box_drawing.rs` | 6 тестов `procedural_box_drawing::*` (─, │, ┼, ┌, ━, ═, █, ░, ▌, ▄, ▘, вне диапазона) |
| §7.3 целочисленное смешивание | `soft_backend::blend` | `blend_is_integer_exact_and_symmetric_at_ends` (ручной расчёт 0x58), `partial_coverage_pixels_are_blended_not_thresholded`, `glyph_pixels_follow_atlas_coverage…` |
| §7.3 флаги/курсор при покрытии | `draw_cell` | тесты INVERSE/UNDERLINE/BLINK/BOLD/курсор на пикселях с полным покрытием |
| §11.2 golden | `tests/golden/` | `software_frame_matches_golden_dump`, детерминизм |
| Пустые ячейки — цвет фона буфера | `buffer.rs` | `custom_default_colors_fill_blank_cells_after_clear_scroll_and_resize` |
| NFR-1 | — | release с профилем §10.2: **894 464 Б** (< 1,5 МБ) |
| Штриховые линии и дуги — точная форма | `box_drawing.rs` | **приближение** (сплошные / прямые углы), см. ⚠ |

## CORRECTNESS

- Пиксельные тесты сравнивают кадр с `blend(bg, fg, покрытие_атласа)`; `blend` проверен вручную посчитанными значениями (`(0xA0·127 + 0x10·128 + 127)/255 = 0x58`) и краями 0/255/1/254.
- Golden просмотрен как PNG: сглаженный текст с хинтованными штрихами, `─┼█` и `╔═╦╗` чёткие и стыкуются, тени трёх уровней, квадрант, курсор.
- Лист всех 160 процедурных символов U+2500–U+259F просмотрен глазами (проверочный проход BP-09), см. `docs/verify/`.

## EVIDENCE

См. `docs/test/2026-10-01-glyph-coverage-hinting.md` и `docs/verify/2026-10-01-glyph-coverage-hinting.md`.

## ⚠ WARNINGS

- **Штриховые** линии (U+2504–U+250B, U+254C–U+254F) рисуются сплошными, **дуги** (U+256D–U+2570) — прямыми углами. Форма неверна, но стыковка и толщина правильные; доработка — отдельная задача, если понадобится.
- Внутренние углы двойных линий (`╔` и т. п.) строятся по правилу «блокирующая рука → дальняя кромка»; сверено глазами, формального эталона нет.
- Хинтинг `Target::Mono` слегка «угловатит» буквы (отмечено в ADR); при необходимости переключение на `Smooth` — одна константа, но golden регенерировать.
- Golden и `.fnt` зависят от версий `skrifa`/`zeno` — при обновлении зависимостей golden может измениться; версии фиксируются `Cargo.lock` (репозиторий всё ещё без коммитов).
- `[profile.release]` в `Cargo.toml` не добавлен (этап 7); 894 КБ получены через `--config`.
- `cargo bloat` не установлен: распределение размера по крейтам не измерено.
- Тест отказа записи кэша (`RegeneratedUnsaved`) по-прежнему только ручной.
- Расширение scope: `AGENTS.md` и `PROJECT_MAP.md` обновлены под новый стек (документация, не код).
