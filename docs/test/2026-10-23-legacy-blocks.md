# Отчёт о тестах: процедурные мозаики и фигуры «Symbols for Legacy Computing»

- **Задача:** `2026-10-23-legacy-blocks` (ADR-0022)
- **Окружение:** Windows 10 Pro 19045, NVIDIA GTX 1060 3GB (драйвер 581.57), Rust stable 1.98.1; тесты — debug, размеры — release; таблицы — Unicode 16.0 (`unicodedata`, Python 3.14)

## Состав

| Файл | Тесты |
|---|---|
| `crates/gterm/tests/legacy_blocks.rs` (5) | **К2** `mosaics_tile_the_cell_exactly`: 6 одноклеточных секстантов и 8 одноклеточных октантов (включая U+1CEA8/AB/A3/A0) дают ровно 255 в каждом пикселе ячейки 10×21; U+1FB00 + U+1FB3B = █, U+1CEA8 + U+1CDE5 = █; октанты 1+3 = квадрант ▘; 8 знаков — только 0/255. **К2** `centered_blocks_and_eighths_follow_the_grid`: ■ и U+1FBE4 по сетке 4×4, U+1FB86 — 18 строк из 21, U+1FB8B — без первой колонки. **К4** `circle_parts_compose`: ● симметричен по осям, ◖ + ◗ = ● попиксельно, четверти U+1FBEC–EF — площадь π·w·h ±2 %, половины U+1FBE9+EB — половина того же. **К4** `triangles_and_diagonals_complement`: ▲ симметричен, ◢+◤, ◣+◥, четверть+три четверти, четыре четверти, U+1FB9A+9B — 255 с допуском 16–32, пять пар диагоналей U+1FB3C–40 + U+1FB52–56 = 255 ±16, площадь U+1FB3C = (w/2)(h/3)/2 ±1. **К5** `dispatcher_and_rasterizer_modes`: диапазоны (13 внутри, 7 снаружи), 543 процедурных знака, `Combined` на U+1CD00 = `Procedural`, `Font` без глифа, независимость от начертания в `Procedural` |
| прежние (178) | рамки, golden, режимы — без изменений эталонов |

## Результат прогона

- `cargo test`: **183 теста** harness, 0 падений; оконные бинарники ok; golden без изменений.
- `stage4_gl`: «OpenGL 3.3: GL ↔ Software identical, 10 scenes», «OpenGL ES 3.0: … 10 scenes».
- ASCII-дамп 30 знаков (зонд, удалён): секстанты/октанты по сетке, «Centered» по 4×4, круги и четверти ориентированы по именам, ▲ ◆ ◢ симметричны, диагонали отрезают нужный угол.
- Мутация (исходник восстановлен): `edge` усечением → 2 теста падают.
- Гейты: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` — чисто.

## Запуски

| Команда | stderr |
|---|---|
| `cargo run -p gterm --example pseudographica -- --backend=soft --fail-frames=2` | `font 'Cascadia Mono'`, `random fill seed …`, кадр нарисован, код 1 после инжекции (штатно) |

Размер release: `demo.exe` 1 540 608 Б (+9 728), `full_table.exe` 1 533 952, `overlay.exe` 1 529 344, `minimal.exe` 1 162 752, `pseudographica.exe` 1 168 384 Б.

## Ограничения

- Вид наборов глазами — на машине владельца.
- Таблицы сверены с именами Unicode выборочно (одноклеточные узоры, дополнения, пять пар диагоналей); полный перебор 334 узоров против имён не делался — генератор детерминирован.
