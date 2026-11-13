# Проверка критериев приёмки: альфа цветов ячейки и фон буфера

ТЗ задачи — `docs/spec/2026-10-09-color-alpha.md`, §5. Второй проход (BP-09): каждый критерий сверен с кодом, тестом или выводом прогона 2026-10-09.

| № | Критерий | Статус | Подтверждение |
|---|---|---|---|
| 1 | При непрозрачных цветах кадр равен существующим golden 16 и 32 px, эталоны не перезаписаны | ✅ | `git status tests/golden` пуст; `golden::software_frame_matches_golden_dump`, `golden::big_font_frame_matches_golden_dump` — ok |
| 2 | Пиксельные тесты формулы §3.4 (bg.a 0/128, fg.a 0/128, BOLD, UNDERLINE с альфой), значения посчитаны вручную | ✅ | `tests/stage3_render.rs`, модуль `alpha`, 8 тестов; расчёт сверен отдельно (0x59728B, 0x586878, 0x8292A2, 0x112A43) |
| 3 | Смена фона: следующий кадр Software равен кадру с нуля и перерисован целиком вместе с полем | ✅ | `background_change_redraws_the_whole_frame` (полоса на весь кадр), `background_change_shows_through_transparent_cells` |
| 4 | `stage4_gl`: сцена с полупрозрачными fg/bg и фоном не по умолчанию, допуск 0 на GL 3.3 и GLES 3.0 | ✅ | «identical, 7 scenes» на обоих уровнях; мутация шейдера ловится этой сценой |
| 5 | Упаковка сохраняет альфу; `from_rgb` → A = 255; `from_rgba` хранит A | ✅ | `cell_pack_and_unpack_are_round_trip_exact`, `color_encodes_alpha_in_the_top_byte` |
| 6 | `--bench` Software ≥ 60 FPS на сетке 120×40, замер записан рядом с прежним | ✅ с замечанием | 75,9–78,4 FPS; до правок 88,0–93,9 (`docs/test/…`). Регрессия ≈ 14 % — ⚠ |
| 7 | Гейты чистые; размер release замерен | ✅ | fmt, clippy `--all-targets`, `cargo test` — чисто; 1 438 208 / 1 432 576 Б |
| 8 | Основное ТЗ — 1.11; есть ADR | ✅ | `docs/TZ_terminal_emulator.md` 1.11 (FR-2, FR-2.1, FR-14, §5.2, §6.1, §7.2–7.3, §8); `docs/adr/0013-…`, индекс |
| 9 | Глазами: строки демо с альфой на GL 3.3, GLES 3.0 и Software | ⏳ | за владельцем; `cargo run`, `cargo run -- --backend=gles`, `cargo run -- --backend=soft` |

**След прохода.** Открыто заново 9 мест:
- 3 тестовых файла;
- `git status` по golden;
- вывод `stage4_gl`;
- 2 замера;
- ТЗ;
- ADR.

Отброшенных находок нет. Расхождение одно — критерий 6 (регрессия скорости при выполненном пороге). Открыт один пункт — критерий 9.
