# Отчёт о тестах: альфа цветов ячейки и фон буфера

- **Задача:** `2026-10-09-color-alpha` (ADR-0013)
- **Окружение:** Windows 10 Pro 19045, NVIDIA GTX 1060 3GB (драйвер 581.57), Rust stable 1.98.1, debug для тестов, release (`opt-level = "z"`) для `--bench`

## Состав (новое и изменённое)

| Файл | Тесты |
|---|---|
| `tests/stage1_buffer.rs` | `color_encodes_alpha_in_the_top_byte`, `buffer_background_defaults_to_black_and_can_be_changed`; в `cell_pack_and_unpack_are_round_trip_exact` — альфа после упаковки |
| `tests/stage3_render.rs`, модуль `alpha` | `fade`; `bg.a` = 0 и 128; `fg.a` = 0 и 128; `BOLD` с альфой; `UNDERLINE` с альфой; обе альфы над фоном буфера — 8 тестов, значения посчитаны вручную |
| `tests/soft_partial_redraw.rs` | `background_change_redraws_the_whole_frame` (бывший тест смены рамки), `background_change_shows_through_transparent_cells` |
| `tests/stage4_gl.rs` | сцена `alpha 16px`: строки с `fg.a` и `bg.a` 0…255, `BOLD`/`UNDERLINE` с альфой, фон 0x206030; третий кадр — запись в строку и фон 0xC04010 |
| `tests/common/mod.rs` | помощник `RenderWith` (фон и отрисовка одним вызовом) |

Golden-эталоны `tests/golden/soft_golden_{16,32}px.bin` не менялись.

## Результат прогона

- `cargo test`: 133 теста harness, 0 падений.
  - `stage1` — 13;
  - `stage3` — 26;
  - `soft_partial_redraw` — 9;
  - `stage4_gl` — 1: GL 3.3 и GLES 3.0, по 7 сцен, допуск 0;
  - оконные `stage6_events`, `stage6_events_auto`, `stage6_fallback`, `stage8_faults` (5 сценариев) — `ok`.
- Проверка мутацией: без строки `fade` в `cell.frag` `stage4_gl` падает на `alpha 16px pass 0` (gl 0x856C60 против soft 0x3010A0); после восстановления — identical.

## `--bench` (release, VSync выключен, 300 кадров после 30 прогрева)

| Сборка | Software 120×40, FPS | GL 3.3 200×60, FPS |
|---|---|---|
| до правок (замер шага 1) | 88,0; 90,2 | 925,8 |
| после правок (шаг 7) | 77,1; 76,9 | 1008,5 |
| попеременно: исходный код (`HEAD`, worktree) | 92,2; 88,6; 90,0 | — |
| попеременно: текущий код | 77,4; 77,7; 78,4 | — |
| вариант с отдельным циклом для непрозрачного fg (отклонён) | 67,8; 67,4 | — |
| вариант `#[inline(always)]` у `fade` (отклонён), попеременно с исходным 92,3 / 93,9 / 93,5 | 75,9; 76,1; 77,9 | — |

NFR-5 (Software ≥ 60 FPS) выполнен. Регрессия около 14 % — в ⚠ отчёта о выполнении.

## Ограничения

- Демо проверено только запуском на 6 с (GL 3.3 и Software, stderr без ошибок); как выглядит альфа — за владельцем (§11.3).
- Замеры только на одной машине.
