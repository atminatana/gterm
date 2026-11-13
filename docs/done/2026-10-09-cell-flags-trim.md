# Отчёт: флаги ячейки — `BOLD`, `ITALIC`, `UNDERLINE`; мигание, `INVERSE`, `CURSOR` удалены

- **Дата:** 2026-10-09
- **Решение:** ADR-0012 (почему так — там)
- **Этапы цикла:** ТЗ и план пропущены по указанию владельца (сокращённый цикл); test/verify-отчёты не создавались, проверки — ниже.

## Что сделано

| Область | Изменение |
|---|---|
| `src/cell.rs` | `CellFlags`: `BOLD = 1<<0`, `ITALIC = 1<<1`, `UNDERLINE = 1<<2`; `INVERSE`, `BLINK`, `CURSOR` удалены |
| `src/renderer/shaders/cell.frag` | `F_BOLD = 1u`, `F_ITALIC = 2u`, `F_UNDERLINE = 4u`; удалены `u_blink`, обмен цветов, гашение глифа |
| `src/renderer/soft_backend.rs` | удалены `INVERSE` и `BLINK` в `draw_cell`, `row_has_blinking` и ветка «сменилась только фаза»; частичный кадр — только dirty-строки |
| `src/renderer/gl_backend.rs`, `src/renderer/mod.rs` | uniform `u_blink` и `ViewParams::blink_phase` удалены |
| `src/blink.rs` | удалён |
| `src/buffer.rs` | `blink_count`, `has_blinking`, `put`, `recount` удалены — запись ячейки напрямую |
| `src/app/*` | `Blinker`, `about_to_wait`, `schedule_wakeup`, `AppOptions::blink_period`, `Request::blink_period`, `Ctx::blink_phase`, `AppHandler::on_tick` удалены; цикл всегда `Wait` |
| `src/bin/demo/*` | курсор — обмен fg/bg своей ячейки, не мигает; `--no-blink` удалён; строки сцены `INVERSE` и `BLINK` удалены; статус обновляется по вводу |
| `src/bin/full_table/main.rs` | мигающая метка `●` удалена; статус обновляется при смене страницы и полной отрисовке |
| `tests/` | `stage6_blink.rs` удалён; `stage1` — тест порядка битов против `cell.frag` вместо счётчика мигания; `stage3` — тесты `INVERSE`/`BLINK` удалены; `soft_partial_redraw` — сценарии на dirty-строках; `stage4_gl` — третий кадр после записи в строку; golden-сцена: `Bold`, `UNDERLINE Italic`, `BOLD UNDERLINE`; оконные тесты ведёт `on_frame` + поток-сторож 60 с |
| `tests/golden/*.bin` | 16 и 32 px перезаписаны (сцена изменилась; до перезаписи оба теста падали на строке 2 — ожидаемо) |
| Документы | ADR-0012, статусы ADR-0004/0006/0010, индекс ADR, `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md` |

## Проверки

- `cargo fmt --check` — чисто; `cargo clippy --all-targets -- -D warnings` — чисто.
- `cargo test` — 122 теста harness'а пройдены, 4 оконных бинарника `ok` на настоящем окне (OpenGL 3.3, NVIDIA GTX 1060): `FixedGrid` 5 кадров, `AutoGrid`, откат GL → Software на лету, 5 сценариев `stage8_faults`.
- `stage4_gl`: «OpenGL 3.3: GL ↔ Software identical, 6 scenes», «OpenGL ES 3.0: … identical, 6 scenes».
- Размер release (Windows x86-64): `demo.exe` 1 437 184 Б (было 1 438 720), `full_table.exe` 1 432 064 Б (было 1 433 600).

## После приёмки

- Глазами на машине владельца (§11.3) — подтверждено 2026-10-09.
- ТЗ обновлено до версии 1.10 под ADR-0012 — 2026-10-09.
