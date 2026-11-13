# Верификация: SVG-глифы и размер шрифта демо-бинарников

| Критерий (ADR-0008 / ТЗ 1.7 FR-8.6) | Статус | Подтверждение |
|---|---|---|
| Разбор `usvg` без фич, растеризация `zeno` | ✅ | `Cargo.toml` (`default-features = false`); `svg.rs` `parse`, `rasterize` |
| Каталог `assets/svg/`, нет каталога — не ошибка | ✅ | `common::load_rasterizer`, `SvgGlyphs::load` (`NotFound` → пусто); тест `missing_directory_gives_empty_set` |
| Имя — только 4–6 HEX и `.svg`, ≤ U+10FFFF, не суррогат | ✅ | `codepoint_from_file_name`; `file_names::*` |
| Неверное имя / повтор / ошибка разбора → stderr и пропуск, без паники | ✅ | `load_file`; тест загрузки; вывод `--nocapture` — 3 строки на 3 файла |
| Приоритет SVG > псевдографика > шрифт | ✅ | `Rasterizer::has_glyph`/`rasterize`; `priority::*` |
| Вписывание с сохранением пропорций, центр | ✅ | `SvgGlyphs::rasterize`; точный тест полос 5 px |
| Цвета игнорируются, `fill/stroke="none"` учитываются | ✅ | `push_path` (цвет не читается; `fill()`/`stroke()` = `None` при `none`); тест кольца |
| Цвет на экране как у глифа, флаги действуют | ✅ | рендереры не менялись (`git diff -- src/renderer` пуст); `stage4_gl` сцена с `BOLD`/`ITALIC`/`INVERSE` |
| Все 4 канала атласа | ✅ | `Atlas::fill_channel` + `is_style_independent`; тест каналов |
| GL ≡ Software | ✅ | `stage4_gl`: 6 сцен, оба уровня |
| Переживает смену DPI и откат бэкенда | ✅ (DPI тестом) / по коду (откат) | `svg_glyphs_survive_size_change_in_place`; откат переносит растеризатор целиком (`Backend::into_rasterizer`, `app.rs:277`) |
| `AppOptions::font_size_px`, демо 26, `--bench` 16 | ✅ | `app.rs:117,127,449,487,702`; `common.rs` `DEMO_FONT_SIZE_PX`; `bench.rs` `load_family(FONT_SIZE_PX)` + `..AppOptions::default()`; запуск `demo`: `glyph_px 26` |
| Карта из SVG (вопрос владельца) | ✅ кадр / ⚠ окно | `docs/research/2026-10-08-svg-map.png`; в окне — у владельца |
| Размер бинарника замерен (NFR-1) | ✅ | 1 438 720 / 1 433 600 Б |
| Гейты | ✅ | `docs/done/2026-10-08-svg-glyphs.md`, EVIDENCE |

Проверочный проход (BP-09): перечитаны `svg.rs` (имя, загрузка, повтор, `rasterize`), `rasterizer.rs` (`has_glyph`, `rasterize`, `is_style_independent`), `atlas.rs` (`fill_channel`), `app.rs` (три места `font_size_px`), `common.rs` (`load_rasterizer`, `demo_options`), `bench.rs` — 14 мест в 7 файлах. Найдено и исправлено до отчёта: проверка повтора при загрузке использовала двоичный поиск по ещё не отсортированному набору (`contains`) — заменена линейной; лог «0 override:» — переписан; лишняя перестройка хинтеров 16 → 26 px при старте — растеризатор сразу строится на 26 px. Ложных подтверждений не выявлено.
