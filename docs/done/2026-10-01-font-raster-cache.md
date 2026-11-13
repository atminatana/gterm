# Выполнение: этап 2 — растр шрифта из TTF с кэшем `.fnt`

- План: `docs/plans/2026-10-01-font-raster-cache.md`; ТЗ §3.2 (FR-8…FR-11); ПОЧЕМУ — `docs/adr/0001-runtime-font-raster-cache.md`.
- Этап «ТЗ» (`docs/spec/`) пропущен: постановка живёт в ТЗ проекта `docs/TZ_terminal_emulator.md` (версия 1.1), отдельный spec не заводился — по согласованию с владельцем в ходе задачи.

## WHAT

| Файл | Изменение |
|---|---|
| `Cargo.toml` | `fontdue = "0.9.4"`, `default-features = false` |
| `.gitignore` | `assets/*.fnt` |
| `assets/font_8x16.bin` | удалён (битый 8×8-растр, единственный потребитель `include_bytes!` исчез) |
| `src/font.rs` | переписан: константы `FONT_FILE`/`FONT_SIZE_PX`, `FntHeader`, `FontData` (заголовок + таблица слотов + 1bpp), `to_fnt_bytes`/`from_fnt_bytes`, `FontError`, `Atlas` с таблицей «кодовая точка → слот» и `pixel()` без аллокаций |
| `src/font_gen.rs` | новый: `find_assets_dir`, `fnt_path`, `load_or_generate` (правило пересоздания FR-8.2, статус кэша FR-8.5), `rasterize` (fontdue, порог 50 %, все глифы BMP), `fnv1a64` |
| `src/lib.rs` | модуль `font_gen` |
| `src/main.rs` | временная точка входа: загрузка шрифта, отчёт в stderr, код ≠ 0 при ошибке |
| `src/renderer/soft_backend.rs` | `SoftBackend::new(atlas)`; размер глифа и пиксели — из атласа (убран вызов глобального `atlas()` на каждую ячейку); `frame.fill()` вместо цикла |
| `src/renderer/gl_backend.rs` | `GlBackend::new(atlas)`, текстура атласа из `self.atlas`; doc-комментарий модуля `//!` |
| `tests/stage2_font.rs` | переписан: 17 тестов (формат, растеризация, кэш во временном каталоге, атлас) |
| `tests/stage3_render.rs` | конструкторы бэкендов получают атлас репозитория; `!frame.is_empty()` |
| `docs/TZ_terminal_emulator.md` | 1.0 → 1.1 |
| `docs/adr/`, `docs/plans/`, `AGENTS.md`, `PROJECT_MAP.md` | созданы/обновлены |

## COMPLETENESS

| Требование | Код | Тест |
|---|---|---|
| FR-8 константы, размер глифа из метрик | `font.rs` константы; `font_gen::rasterize` | `repo_font_produces_cell_sized_glyphs_for_all_bmp_codepoints` |
| FR-8.1 путь `assets/<stem>_<px>px.fnt`, поиск `assets/` | `fnt_path`, `find_assets_dir` | `missing_cache_is_created_with_matching_header` (путь); `find_assets_dir` — только `cargo run` (см. ⚠) |
| FR-8.2 пересоздание: нет / битый / иной размер / иной хеш / иное имя; совпал → reuse | `load_or_generate`, `has_same_source` | 5 тестов модуля `cache` |
| FR-8.3 формат `.fnt` | `to_fnt_bytes`/`from_fnt_bytes` | `round_trip_…`, `rejects_bad_magic_version_and_truncation`, `rejects_unsorted_codepoint_table` |
| FR-8.4 все глифы BMP, порог 50 %, нет глифа → слот 0, детерминизм | `rasterize`, `bmp_codepoints_in_font`, `blit_glyph` | `latin_cyrillic_and_box_drawing_have_ink…`, `rasterization_is_deterministic`, `slot_lookup_…` |
| FR-8.5 TTF нет → ошибка без паники; битый TTF → `TtfParse` | `load_or_generate`, `rasterize` | `missing_ttf_is_an_io_error`, `garbage_ttf_is_a_typed_error_not_a_panic` |
| FR-8.5 кэш не записался → работа из памяти | `CacheStatus::RegeneratedUnsaved` | **не покрыто** (см. ⚠) |
| FR-9 атлас 16 слотов в ширину, слот `(s%16, s/16)`, O(1) поиск | `Atlas::from_font_data`, `slot_of`, `pixel` | 2 теста модуля `atlas` |
| FR-10 рендереры без констант 8/16 | `soft_backend.rs`, `gl_backend.rs` берут `atlas.glyph_w/h` | `cargo clippy` (компиляция); поведение — этап 3 |
| FR-11 `fontdue` обязателен | `Cargo.toml` | сборка |

## CORRECTNESS

- 27 тестов зелёные (7 + 17 + 3), с `--features gl` — те же + 3 ignored (унаследованные GL-заглушки).
- Тест reuse доказывает именно переиспользование: в файл с верным заголовком подложен растр `0xAA`, загрузчик вернул его, а не перерастеризовал.
- Растр проверен глазами (ASCII-дамп из `.fnt`): базовая линия, хвосты `g`/`y`, кириллица `Я`/`ё`, `─`/`┼` на всю ячейку, `█` — заливка.

## EVIDENCE

См. отчёт о тестах `docs/test/2026-10-01-font-raster-cache.md` и верификацию `docs/verify/2026-10-01-font-raster-cache.md`.

## ⚠ WARNINGS

- **Размер глифа 10×18, а не 8×16**: PT Mono при 16 px имеет advance 0.6 em. Чтобы получить ячейку ~8×14/8×16 — `FONT_SIZE_PX ≈ 13`; решение за владельцем, рендереры размер переживают (FR-10).
- `RegeneratedUnsaved` (каталог только для чтения) — без автотеста: на Windows под текущим пользователем нет надёжного способа сделать каталог read-only из теста без прав администратора. Проверяется вручную.
- `find_assets_dir` (приоритет `<exe>/assets` над `./assets`) — покрыт только запуском `cargo run`, в тесте путь задаётся явно.
- `cargo bloat` не установлен: вклад `fontdue` в размер не замерен (`[data needed: cargo install cargo-bloat → cargo bloat --release]`). Факт: release-бинарник `main` **280 064 байт** без `[profile.release]` из §10.2 — запас до 1,5 МБ большой.
- У `█` (U+2588) нижняя строка ячейки заполнена на 9 из 10 px — край покрытия ниже порога 50 %. Косметика; если важна сплошная заливка псевдографики — обсуждать порог или дорисовку по bbox.
- Вне рамок, не трогал: Software-рендерер по-прежнему не соответствует §7.3/§8 (dirty-строки, фон пустых ячеек, формат кадра, флаги) — этап 3; GL-бэкенд — заглушка не по §6–7; `[profile.release]` отсутствует; `PROJECT_MAP.md` создан впервые.
- Расширение scope на 1 строку: `tests/stage3_render.rs:81` `len() > 0` → `!is_empty()` — унаследованное clippy-замечание, иначе `cargo clippy --all-targets` красный.
- Унаследованный мёртвый код `RendererKind` под `#[allow(dead_code)]` в обоих бэкендах — не трогал; предлагается убрать на этапе 5 (цепочка fallback).
