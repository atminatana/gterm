# Проверка критериев приёмки: процедурные мозаики и фигуры «Symbols for Legacy Computing»

ТЗ задачи — `docs/spec/2026-10-23-legacy-blocks.md`, §4. Второй проход (BP-09): каждый критерий сверен с кодом, тестом или сохранённым выводом прогона 2026-10-23.

| № | Критерий | Статус | Подтверждение |
|---|---|---|---|
| К1 | Рамки и golden без изменений | ✅ | `box_drawing::draw` не менялся (только `Canvas` вынесен в `procedural.rs`); `cargo test` 183/0, golden без изменений, `stage4_gl` 10 сцен identical |
| К2 | Мозаики: узоры по именам Unicode, 0/255, ровная сетка | ✅ | `mosaics_tile_the_cell_exactly`, `centered_blocks_and_eighths_follow_the_grid`; мутация `edge` поймана |
| К3 | Брайль | — | не входит (О1) |
| К4 | Фигуры: симметрия ●, ◖+◗, четверти 2×2, ▲, ◢+◤, пары диагоналей | ✅ | `circle_parts_compose`, `triangles_and_diagonals_complement` |
| К5 | Диспетчер, `Combined` без глифа, `is_style_independent` | ✅ | `dispatcher_and_rasterizer_modes` |
| К6 | Пример: образцы и случайное заполнение из всех диапазонов; глазами | 🔶 | код `draw_set_samples`, `fill_random_procedural`; запуск с инжекцией рисует кадр; глазами — за владельцем |
| К7 | Гейты, размер, документы | ✅ | fmt, clippy, 183 теста; `demo.exe` 1 540 608 Б; ADR-0022 + индекс, ТЗ 1.18 (FR-8.4), `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md` |

**След прохода.** Открыто заново 7 мест: `wc -l` модулей; `grep -n "procedural::"` в `rasterizer.rs` (5 вызовов); первые/последние значения трёх таблиц против вывода генератора; ASCII-дамп; выводы `legacy_blocks` и `stage4_gl`; размеры release.

Отброшенных находок нет. Расхождений с ТЗ нет; К3 снят по О1.
