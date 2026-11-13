# Верификация: динамический атлас, начертания, `assets/fonts/`

| Критерий (ТЗ 1.2 / ADR-0003) | Статус | Подтверждение |
|---|---|---|
| ADR п.1: атлас динамический, без файла | ✅ | `atlas.rs::ensure`; `font_gen.rs` и `.fnt` удалены; `grep -rn "fnt" src tests` → 0 |
| ADR п.2: каналы = начертания, один слот, ленивое заполнение | ✅ | `fill_channel` пишет только канал источника (процедурные — все); `styles_fill_channels_lazily_in_the_same_slot` |
| ADR п.3: подстановка, нет Regular → ошибка, выбор по `fsSelection` | ✅ | `FontFamily::from_faces`; тесты `family::*`; stderr demo печатает соответствие |
| ADR п.4: `ITALIC`, `BOLD` = глиф + осветление | ✅ | `cell.rs`, `draw_cell`; `bold_and_italic_flags_select_atlas_channels` (fg Bold = `0x657585`) |
| ADR п.5: страница `min(limit, 2048)`, не меньше 1024 | ✅ | `page_px_for_limit`; тест четырёх значений; demo: 2048 px / 19 788 слотов |
| ADR п.6: переполнение → сброс | ✅ | `clear_page` + `resets`; `overflow_resets_the_page_and_keeps_serving`, `atlas_reset_mid_frame…` |
| ADR п.7: горячий путь без аллокаций | ✅ (чтение) | `render`: `prepare_glyphs` (промахи) → `draw_row` (только `pixel`/`fill`); растеризация в `scratch`, контур в `PathPen` с `clear()` |
| ADR п.8–9: псевдографика процедурно; бит-идентичность не зависит от слотов | ✅ | `box_drawing_comes_from_procedural_generator_in_every_style`; `atlas_reset_mid_frame…` (разные слоты → тот же кадр) |
| FR-8.1: `find_folder`, `assets/fonts/` | ✅ | `find_assets_dir`; тест из cwd; demo из `target/debug` и `/tmp` |
| FR-8.5: ошибки без паники | ✅ | `NoRegularFace`, `Io`, `TtfParse`; `screen_larger_than_atlas_page_is_an_error_not_garbage` |
| NFR-1 | ✅ | 919 552 Б с профилем §10.2 |
| NFR-7, NFR-9 | ✅ | `Result` везде; `unsafe` только `gl_backend.rs` |
| §4.5 гейты | ✅ | fmt / clippy ×2 / 52 теста |
| Устаревшие упоминания (`.fnt`, `font_gen`, `FONT_FILE`, `pt-mono`) в коде/ТЗ/AGENTS/MAP | ✅ | `grep` → только история в ADR/research/done |
| Переоткрытые места | — | `font.rs`, `rasterizer.rs`, `atlas.rs`, `soft_backend.rs`, `cell.rs`, оба теста — целиком; golden — PNG; 0 ложных пунктов снято |

Расхождений с ТЗ 1.2 не найдено.
