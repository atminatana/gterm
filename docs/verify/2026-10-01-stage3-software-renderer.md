# Верификация: этап 3 — Software-рендерер

| Критерий ТЗ | Статус | Подтверждение |
|---|---|---|
| §7.3 алгоритм: INVERSE → BOLD → курсор-блок; `a` из атласа; BLINK; UNDERLINE; курсор underline/bar; `out = a ? fg : bg` | ✅ | `soft_backend.rs::draw_cell` — порядок операций как в псевдокоде; тесты `algorithm::*` |
| §7.2 `u_origin` — центрирование | ✅ | `layout()`: `saturating_sub / 2`; `grid_is_centered_and_leftover_is_border` |
| FR-12 один draw call | n/a | относится к GL (этап 4) |
| FR-13 `NEAREST`, целочисленный масштаб | ✅ | `fill_block` блоками `scale × scale`; `integer_scale_…` |
| FR-14 рамка | ✅ | `frame.fill(border)` при полной перерисовке; тест |
| FR-15 мигание без обновления данных | ✅ | фаза — в `ViewParams`, буфер не трогается; `blink_hides_…` |
| FR-16 подчёркивание/курсор по координатам в ячейке | ✅ | `gy == glyph_h − 1`, `gy + 2 ≥ glyph_h`, `gx == 0`; тест |
| §8 кадр `u32` `0x00RRGGBB`, совместим с `softbuffer` | ✅ | `encode_color`, `frame_format_…`; `app.rs` копирует кадр в буфер `softbuffer` без преобразования |
| §8 dirty-строки; `resize` → полная перерисовка | ✅ | `render`: `full = needs_full_redraw || last_view != view`; тесты `dirty_rows::*` |
| §8 собственный кадр `Vec<u32>` размера окна | ✅ | `frame` в `SoftBackend`; `resize` |
| §8 отказ `softbuffer` → сообщение, код ≠ 0 | ✅ (код) | `AppError::Surface` → `demo.rs` `ExitCode::FAILURE`; без автотеста |
| §5.2 `ViewParams { scale, border_color, blink_phase, cursor }` | ✅ | `renderer/mod.rs` |
| §5.3 п.4: `Resized` → `resize`, `RedrawRequested` → `render` | ✅ | `app.rs::window_event` |
| FR-22 окно по числу ячеек; FR-26 `ControlFlow::Wait` | ✅ | `create_window` (`grid_size_px`), `set_control_flow(Wait)`; запуск `demo.exe` |
| §11.2 golden-дамп в репозитории | ✅ | `tests/golden/soft_golden_1x.bin`, 120 000 байт |
| NFR-6 нет аллокаций в горячем пути | ✅ (чтение) | `render` → `fill`/индексация; `clone_from` для `ViewParams` без кучи (`Color`, `CursorState` — Copy-типы) |
| NFR-7 без паники | ✅ | `Result` везде; `scale = 0`, малое окно — тесты |
| NFR-9 `unsafe` только в `gl_backend` | ✅ | `grep -rn unsafe src/` → только `gl_backend.rs` |
| §5.1 `src/bin/demo.rs`, lib | ✅ | `main.rs` удалён, `cargo run` запускает `demo` |
| §4.5 fmt / clippy / тесты | ✅ | обе конфигурации чисто; 39 passed |
| Переоткрытые места при проходе | — | `soft_backend.rs`, `app.rs`, `demo.rs`, `stage3_render.rs` — целиком; golden — как PNG; пиксель курсора в golden проверен независимым скриптом (`0xFFFF` = инвертированный cyan) |

Расхождений с ТЗ не найдено. Открытые пункты — в ⚠ WARNINGS отчёта `docs/done/`.
