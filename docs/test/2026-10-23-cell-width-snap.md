# Отчёт о тестах: целая ширина ячейки подбором размера шрифта (опция)

- **Задача:** `2026-10-23-cell-width-snap` (ADR-0019)
- **Окружение:** Windows 10 Pro 19045, NVIDIA GTX 1060 3GB, Rust stable 1.98.1; тесты — debug, размеры — release; системный шрифт — `Cascadia Mono`

## Состав

| Файл | Тесты |
|---|---|
| `crates/gterm/tests/cell_snap.rs` (2) | **К2** `snapped_size_makes_advance_integer`: для 11/16/22 px advance `M` при `snapped_size` отличается от целого меньше чем на 0,01 px (квантование ppem 1/64 и масштаба 16.16 в `skrifa`), ширина ячейки равна округлению advance при номинале, `metrics_at_size` и `metrics_at` дают ту же ширину; шрифт репозитория — `1229/2048` em, подбор при 16 px ≈ 16,66 px. **К3** `rasterizer_snap_option_rebuilds_and_survives_set_size`: по умолчанию выключено, `effective_size_px == 16`; включение — номинал 16, эффективный ≈ 16,66, ячейка 10, глифы `A`/`M` Bold рисуются; `set_size(11)` сохраняет опцию (ячейка 7, ≈ 11,66 px); выключение возвращает метрики номинала |
| прежние (175) | без изменений эталонов |

## Результат прогона

- `cargo test`: **177 тестов** harness, 0 падений; оконные бинарники ok; golden без изменений.
- Мутация (исходник восстановлен): `rebuild` без подбора → падает К3.
- Гейты: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` — чисто.

## Запуски

| Команда | stderr |
|---|---|
| `cargo run -p gterm --example pseudographica -- --backend=soft --fail-frames=2` | `glyph_px 14 → effective 14.00 px, cell 8x16 (snap cell width: false)`; кадр нарисован, код 1 после инжекции (штатно) |
| то же с `--snap-cell` (debug и release) | `glyph_px 14 → effective 13.65 px, cell 8x16 (snap cell width: true)` |
| `… -- --help` | строка `--snap-cell      целая ширина ячейки подбором размера шрифта (ADR-0019)` |

Размер release: `demo.exe` 1 523 712 Б (+1 024), `full_table.exe` 1 517 056, `overlay.exe` 1 512 448, `minimal.exe` 1 145 856, `pseudographica.exe` 1 149 440 Б.

## Ограничения

- Эффект на стыках рамок и резкость букв при дробном ppem — глазами на машине владельца.
- Тесты опираются на шрифт репозитория (advance `1229/2048` em); на другом шрифте значения ячейки иные, но формула проверяется общей частью теста.
