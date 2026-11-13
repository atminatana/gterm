# Выполнение: этап 3 — Software-рендерер по §7.3/§8, golden-тест, окно

- План: `docs/plans/2026-10-01-stage3-software-renderer.md`; ТЗ §3.3, §5.2–5.3, §7.3, §8, §11.2.
- Архитектурных решений нет; детали алгоритма (BOLD, курсор, центрирование, формат кадра) вписаны в ТЗ §7.3 как контракт бит-идентичности.
- Этап «ТЗ» (`docs/spec/`) не заводился — постановка в ТЗ проекта.

## WHAT

| Файл | Изменение |
|---|---|
| `src/cursor.rs` | новый: `CursorShape {Block, Underline, Bar}`, `CursorState` |
| `src/renderer/mod.rs` | `ViewParams` + `cursor`, `Clone/PartialEq` |
| `src/renderer/soft_backend.rs` | переписан: предвыделенный кадр `0x00RRGGBB`, центрирование, полная перерисовка при resize/смене `ViewParams`, иначе только dirty-строки; INVERSE/BOLD/BLINK/UNDERLINE/курсор по §7.3; фон пустых ячеек рисуется; без аллокаций в `render` |
| `src/app.rs` | новый: `App` на `winit 0.30` (`ApplicationHandler`), вывод через `softbuffer 0.4`; `Resized` → `resize`, `RedrawRequested` → `render` + `present`, `Esc`/крестик — выход, `ControlFlow::Wait`; все ошибки — `AppError` |
| `src/bin/demo.rs` | новый (вместо `src/main.rs`, §5.1): загрузка шрифта, статическая сцена (текст, флаги, цвета, псевдографика, глифы по слотам), запуск окна |
| `src/main.rs` | удалён |
| `src/lib.rs` | модули `cursor`, `app` |
| `Cargo.toml` | `winit 0.30` (`rwh_06,x11,wayland,wayland-dlopen`), `softbuffer 0.4` (`x11,wayland,wayland-dlopen`), оба без default-features |
| `tests/stage3_render.rs` | переписан: 13 тестов алгоритма/dirty/golden + 2 унаследованных GL (`#[ignore]`) |
| `tests/golden/soft_golden_1x.bin` | эталон 250×120 `u32` LE (120 000 байт) |
| `docs/TZ_terminal_emulator.md` | §7.3: BOLD, курсор, `u_origin`, формат кадра |

## COMPLETENESS

| Требование | Код | Тест |
|---|---|---|
| FR-13 целочисленный масштаб, без интерполяции | `fill_block` | `integer_scale_replicates_each_glyph_pixel_as_a_block` |
| FR-14 остаток окна — `border_color`; центрирование (§7.2) | `layout`, `frame.fill(border)` | `grid_is_centered_and_leftover_is_border` |
| FR-15 BLINK по фазе без изменения данных | `blink_hidden` | `blink_hides_ink_only_in_off_phase` |
| FR-16 UNDERLINE и курсор underline/bar по координатам в ячейке | `draw_cell` | `underline_fills_last_glyph_row_with_fg`, `cursor_block_inverts_…` |
| FR-3 INVERSE, BOLD | `draw_cell`, `lighten` | `inverse_swaps_colors`, `bold_lightens_fg_by_0x55_…` |
| §8 формат `0x00RRGGBB`; dirty-строки; полная перерисовка при resize | `render` | `frame_format_is_00rrggbb_everywhere`, `second_render_redraws_only_dirty_rows_…`, `view_change_forces_full_redraw` |
| §8 фон всегда (ошибка прежней реализации) | `draw_cell` без пропуска `ch == 0` | `glyph_pixels_follow_atlas_and_background_is_always_drawn` |
| §11.2 golden-дамп в репозитории | `tests/golden/` | `software_frame_matches_golden_dump`, `rendering_the_same_scene_twice_is_bit_identical` |
| NFR-7 нет паники: `scale = 0`, окно меньше сетки | `render`, `fill_block` | `zero_scale_is_an_error_not_a_panic`, `window_smaller_than_grid_clips_without_panic` |
| NFR-6 нет аллокаций в `render` | только индексация и `fill`; аллокации в `resize` | **по чтению кода**, автотеста нет (см. ⚠) |
| FR-22 окно по размеру сетки, FR-26 `Wait`, `Esc` | `app.rs` | ручной запуск `demo.exe` (см. EVIDENCE) |
| §8 отказ `softbuffer` → сообщение и код ≠ 0 | `AppError::Surface` → `ExitCode::FAILURE` | **не покрыто** автотестом |

## CORRECTNESS

- Пиксельные тесты сравнивают кадр с атласом и ожидаемыми цветами независимо от рендерера (ожидания вычислены вручную: `0x102030 + 0x55 → 0x657585`, насыщение `0xF000FF → 0xFF55FF`).
- Golden сгенерирован самим рендерером (эталон регрессии, не независимая истина) и проверен глазами как PNG: рамка, центрирование, кириллица, `─┼█`, INVERSE, UNDERLINE, скрытая строка BLINK в фазе off, инвертированный курсор на `>`.
- Dirty-тест ловит прежнюю ошибку (очистка кадра при частичной перерисовке).

## EVIDENCE

См. `docs/test/2026-10-01-stage3-software-renderer.md` и `docs/verify/2026-10-01-stage3-software-renderer.md`.

## ⚠ WARNINGS

- **Курсор-блок на пустой ячейке невидим**: `Cell::blank()` — fg = bg = чёрный (этап 1), инверсия ничего не меняет. Это семантика буфера; в реальном терминале пустые ячейки несут текущие атрибуты. Предлагается отдельно: `ScreenBuffer::new` с цветами по умолчанию (например, белый на чёрном).
- NFR-6 (нет аллокаций в `render`) — подтверждено чтением, не инструментом; `[data needed: тест со счётным аллокатором]`.
- Отказ `softbuffer`/`winit` → код ≠ 0 — без автотеста (нужно окружение без дисплея).
- `--fail-gl`, `--backend`, fallback — этап 5; `panic = "abort"`/`[profile.release]` — не добавлял.
- Golden зависит от шрифта и `FONT_SIZE_PX` — при смене регенерировать `GTERM_UPDATE_GOLDEN=1`.
- Release `demo.exe` **774 144 байт** без `[profile.release]` — запас до 1,5 МБ; `cargo bloat` не установлен.
- Демо-сцена `slot_to_codepoint` — линейный поиск по 65 536 кодам на ячейку (только демо, ~1000 ячеек, разово) — заменить на обратную таблицу в `Atlas` на этапе 7, если станет заметно.
- `AGENTS.md`: исправлены устаревшие утверждения («`[dependencies]` пусто», «`rust-version` не установлен», «`main.rs` — заглушка») — вне исходного scope, но они противоречат `Cargo.toml` и мешают следующим сессиям.
- GL-бэкенд не трогал: по-прежнему заглушка, тесты `#[ignore]` безусловные — этап 4.
