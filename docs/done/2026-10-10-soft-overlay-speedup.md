# Отчёт о выполнении: ускорение наложения в Software-рендерере

- **Задача:** `2026-10-10-soft-overlay-speedup`. ТЗ — `docs/spec/2026-10-10-soft-overlay-speedup.md`, план — `docs/plans/2026-10-10-soft-overlay-speedup.md`.
- **Решение и его ПОЧЕМУ:** ADR-0015.

## Что сделано

| Шаг плана | Результат |
|---|---|
| 0. Замер разбивки | основной слой 8,8 мс, сборка текстуры 21,0 мс, наложение на кадр 10,6 мс (таймеры в копии крейта вне репозитория) |
| 1. Сравнение записей | `src/overlay.rs`: `Overlay` и `Entry` — `PartialEq, Eq`; ручной `Clone` с `clone_from`, переиспользующим память |
| 2. Кэш текстуры | `src/renderer/soft_backend.rs`: поля `overlay_drawn`, `is_overlay_valid`; `draw_overlay` → `is_overlay_current` / `build_overlay` / наложение; `set_size` сбрасывает кэш, после сброса текстура очищается целиком |
| 3. Замер | `--overlay` 23,7 → 48,2 FPS; без наложения 103,3 |
| 4. Пары каналов | `RB`, `ROUND_PAIR`, `div255_pair`, `mul_pair`; `blend`, `over`, `composite` считают каналы парами |
| 5. Замер, размер | `--overlay` 65,4–70,7 FPS, без наложения 103,6–114,9; размеры release те же, что до задачи |
| 6. Документы | ADR-0015 и индекс, статус ADR-0014, `PROJECT_MAP.md`, `AGENTS.md`, `TODO.md`, отчёты `done`/`test`/`verify` |

Тесты: `tests/overlay.rs` +8 (2 — `overlay_type`, 6 — `software`), новый `tests/soft_pixel_math.rs` (3). Подробности — `docs/test/2026-10-10-soft-overlay-speedup.md`.

## COMPLETENESS
- Кэш текстуры (ТЗ §3 п.1): код — `soft_backend.rs:227–272`, `overlay.rs:13–80`; тесты — 6 сценариев инвалидации и 2 теста `Overlay`.
- Пары каналов (§3 п.2): код — `soft_backend.rs:534–617`; тест — `tests/soft_pixel_math.rs`.
- Замеры и ADR (§3 п.3): этот отчёт, `docs/test/…`, ADR-0015.

## CORRECTNESS
- Кадр бит в бит прежний: golden 16/32 px не перезаписывались (`git status tests/golden` пуст), `stage4_gl` — «identical, 8 scenes» на GL 3.3 и GLES 3.0.
- Тесты ловят ошибки (мутации, исходник восстановлен после каждой):
  - кэш без сравнения записей → падают 2 теста;
  - без сброса кэша в `set_size` → падает `overlay_after_font_size_change_with_the_same_grid_px_matches_a_fresh_frame`;
  - `ROUND_PAIR = 0x007F_007E` → падают все 3 теста `soft_pixel_math`.

## EVIDENCE
- Базовый прогон до правок: 151 тест harness, 0 падений.
- После: `cargo fmt --check` — чисто; `cargo clippy --all-targets -- -D warnings` — чисто; `cargo test` — 162 теста harness (151 + 11 новых), 0 падений, оконные бинарники `harness = false` — ok.
- `--bench --backend=soft` (release, 120×40, 300 кадров) — таблица в `docs/test/…`.
- Размер release: `demo.exe` 1 454 080 Б, `full_table.exe` 1 447 424 Б, `overlay.exe` 1 442 816 Б.

## ⚠ WARNINGS
- Наложение, которое меняется каждый кадр, кэш не ускоряет; такой сценарий не замерялся (в `--bench --overlay` наложение неподвижно).
- `soft_backend.rs` — 625 строк (до задачи 575; ориентир QG-06 — 500). Превышение унаследовано; файл связный: один алгоритм §7.3 и его попиксельная арифметика. Вынести попиксельные функции (`div255*`, `blend`, `fade`, `mul*`, `over`, `composite`) в отдельный модуль — предложение отдельной задачей.
- ~~Визуальная проверка `cargo run --bin overlay` на Software — за владельцем (§11.3).~~ Проверено владельцем 2026-10-10: ускорение видно.
