# Верификация: этап 2 — растр шрифта из TTF с кэшем `.fnt`

Второй проход по критериям ТЗ 1.1 (BP-09). Каждый пункт — ссылка на код/тест либо расхождение.

| Критерий | Статус | Подтверждение |
|---|---|---|
| FR-8: константы `FONT_FILE`, `FONT_SIZE_PX`; размер глифа из метрик | ✅ | `src/font.rs` константы; `font_gen::rasterize` — `advance_width('M')`, `ascent − descent`; `cargo run` → `glyph 10x18` |
| FR-8.1: `assets/<stem>_<px>px.fnt`; поиск `<exe>/assets`, затем `./assets` | ✅ | `font_gen::fnt_path`, `find_assets_dir`; `cargo run` создал `assets/pt-mono_regular_16px.fnt` |
| FR-8.2: пересоздание при отсутствии / нечитаемости / иной версии, имени, размере, хеше; иначе reuse | ✅ | `has_same_source` + `read_matching_cache`; 5 тестов `cache::*` |
| FR-8.3: формат `GFNT`, заголовок, таблица слотов, 1bpp растр, LE | ✅ | `to_fnt_bytes`/`from_fnt_bytes`; независимый разбор файла скриптом (Python) дал `raster=29520==820·36` |
| FR-8.4: все глифы BMP, порог 50 %, детерминизм, нет глифа → слот 0 | ✅ | `bmp_codepoints_in_font`, `COVERAGE_THRESHOLD = 128`; тесты `rasterization::*`; 814 из 819 слотов непустые |
| FR-8.5: TTF нет → код ≠ 0 и сообщение; запись не удалась → предупреждение, работа из памяти | ✅ / ⚠ | `main.rs` → `ExitCode::FAILURE`; `CacheStatus::RegeneratedUnsaved` — без автотеста |
| FR-9: атлас `16·w × ⌈n/16⌉·h`, слот `(s%16, s/16)`, O(1) поиск | ✅ | `Atlas::from_font_data`, `slot_by_codepoint` (65 536 `u16`); тесты `atlas::*`; `cargo run` → `atlas 160x936` = 16·10 × 52·18 |
| FR-10: рендереры без констант 8/16 | ✅ | `soft_backend.rs` / `gl_backend.rs`: `self.atlas.glyph_w/h`; `grep -n "8u32\|16u32\|= 8;\|= 16;" src/renderer` — 0 |
| FR-11: `fontdue` обязателен, feature `ttf` нет | ✅ | `Cargo.toml` |
| NFR-1: размер ≤ 1,5 МБ | ✅ (частично) | release `gterm.exe` 280 064 байт **без** `[profile.release]`; вклад `fontdue` отдельно не замерен (`cargo bloat` не установлен) |
| NFR-6: нет аллокаций в горячем пути | ✅ для шрифта | `Atlas::pixel`, `slot_of` — индексация без аллокаций; `Atlas::glyph()` аллоцирует и помечен как тестовый |
| NFR-7: нет паники при ошибках | ✅ | все пути — `Result<_, FontError>`; `garbage_ttf_is_a_typed_error_not_a_panic`; `expect` в `from_fnt_bytes` (срез 8 байт после проверки длины) и `rasterize` (символ из валидной таблицы) — инварианты, не внешний ввод |
| NFR-9: `unsafe` только в `gl_backend` | ✅ | `grep -rn unsafe src/ --include=*.rs` → только `gl_backend.rs` |
| §4.5: fmt / clippy `-D warnings` / тесты | ✅ | все три команды чисто в обеих конфигурациях features |
| §11.1 тесты шрифта по списку ТЗ | ✅ | round-trip, атлас/позиции, поиск слота, детерминизм, кириллица/псевдографика, кэш: создан / не перезаписан / пересоздан |
| Пересмотренные `file:line` при проходе | — | `font.rs`, `font_gen.rs`, `main.rs`, оба бэкенда, `stage2_font.rs` — открыты повторно целиком; 0 ложных пунктов снято |

Расхождений с ТЗ не найдено. Открытые вопросы — в ⚠ WARNINGS отчёта `docs/done/`.
