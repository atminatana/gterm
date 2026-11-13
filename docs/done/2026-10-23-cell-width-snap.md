# Отчёт о выполнении: целая ширина ячейки подбором размера шрифта (опция, вариант А)

- **Задача:** `2026-10-23-cell-width-snap`. ТЗ — `docs/spec/2026-10-23-cell-width-snap.md`, план — `docs/plans/2026-10-23-cell-width-snap.md`.
- **Решение и его ПОЧЕМУ:** ADR-0019.
- **Итог 2026-10-23:** владелец проверил глазами и отклонил — улучшения нет. Код (`font.rs`, `rasterizer.rs`, пример, `tests/cell_snap.rs`, ТЗ 1.16, `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md`) откачен до состояния коммита 7f3d6b2; отчёты ниже описывают откаченное состояние и хранятся как запись эксперимента. ADR-0019 — `Rejected`.

## Что сделано

| Шаг плана | Результат |
|---|---|
| 1. Библиотека | `font.rs`: `advance_m_per_px` (`:357`, из единиц шрифта и `unitsPerEm`), `FontFamily::metrics_at_size` (`:277`), `snapped_size` (`:284`), `cell_metrics_at(ttf, f32)` (`:381`; целый `cell_metrics` — обёртка); `rasterizer.rs`: поля `effective_px`, `snap_cell_width`, `set_snap_cell_width` (`:194`), `snap_cell_width()`, `effective_size_px` (`:206`), общий `rebuild` (`:211`) для `set_size` и опции, `build_states(f32)`; `size_px()` — номинальный, `App`/атлас/бэкенды не тронуты |
| 2. Тесты | `tests/cell_snap.rs` (2): математика подбора при 11/16/22 px и растеризатор с опцией |
| 3. Пример | `pseudographica`: флаг `--snap-cell` (снимается до `Config::parse`), опция включается до `App::new`, stderr `glyph_px 14 → effective 13.65 px, cell 8x16 (snap cell width: true)`, статус дополнен «ячейка: целая/дробная», строка в `--help` |
| 4. Документы | ADR-0019 + индекс; ТЗ 1.16 (FR-8, FR-24); `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md`; отчёты `test`/`verify` |

Отклонения от плана:
- `a` (advance `M` на пиксель размера) считается из единиц шрифта и `unitsPerEm`, а не из масштаба при `Size::new(1.0)`: `skrifa` квантует ppem до 1/64 px и хранит масштаб в 16.16, при 1 px ошибка доходила до 0,8 %. Остаток после подбора — порядка 0,005 px (тот же квантователь), ширина ячейки совпадает.
- Размер шрифта примера — 14 px: владелец изменил константу на диске до начала задачи (в ТЗ 11 px); оставлено как есть, у `Cascadia Mono` 14 px → 13,65 px, ячейка 8 px.

## COMPLETENESS
- Опция — код `font.rs`/`rasterizer.rs`; тесты `cell_snap` (К2, К3); умолчание — `snap_cell_width() == false` в тесте, golden нетронуты (К1).
- Пример — флаг, лог, статус; запуск с флагом и без на Software с инжекцией ошибки кадра (К4, автоматизируемая часть).

## CORRECTNESS
- `snapped_size`: advance при эффективном размере отличается от целого меньше чем на 0,01 px при 11/16/22 px; `metrics_at_size(size).glyph_w` равно округлению advance при номинале; шрифт репозитория — `1229/2048` em, подбор при 16 px даёт `10 / a ≈ 16,66` px.
- Растеризатор: с опцией глифы `A` Regular и `M` Bold рисуются при дробном размере хинтера; `set_size(11)` сохраняет опцию (7 px ячейка, ≈ 11,66 px); выключение возвращает метрики номинала бит в бит.
- Мутация (исходник восстановлен, прогон после восстановления зелёный): `rebuild` без подбора (`effective = nominal`) → падает `rasterizer_snap_option_rebuilds_and_survives_set_size`.
- `cargo test` после изменений: golden, `stage4_gl` (10 сцен identical), все прежние тесты без правок эталонов.

## EVIDENCE
- Гейты после последнего изменения кода: `cargo fmt --check` чисто, `cargo clippy --all-targets -- -D warnings` чисто, `cargo test` — **177 тестов** harness (175 + 2), 0 падений, 4 оконных бинарника ok; `git status crates/gterm/tests/golden` пуст.
- Размер release: `demo.exe` 1 523 712 Б (+1 024), `full_table.exe` 1 517 056 (+1 536), `overlay.exe` 1 512 448 (+1 024), `minimal.exe` 1 145 856 (+1 024), `pseudographica.exe` 1 149 440 Б (+2 048).
- Запуски примера (debug и release, `--backend=soft --fail-frames=2`): без флага `glyph_px 14 → effective 14.00 px, cell 8x16`; с `--snap-cell` — `effective 13.65 px, cell 8x16`.
- LOC: `font.rs` 404, `rasterizer.rs` 328, `examples/pseudographica.rs` 378, `tests/cell_snap.rs` 104.
- Проверочный проход BP-09: заново открыты 6 мест — `grep -n` по новым функциям, `wc -l`, вывод `cell_snap`, сохранённый вывод `cargo test`, `ls -l target/release`, разбор `hhea`/`hmtx`/`head` шрифта репозитория (`unitsPerEm` 2048, advance 1229). Отброшенных находок нет.

## ⚠ WARNINGS
- Глазами (К4) — за владельцем: два запуска примера в режиме `w`, стыки `─` и резкость букв при дробном ppem. Вердикт определяет судьбу опции (переключение на лету, `AppOptions`) и ТЗ варианта Б.
- Эффективный размер квантуется `skrifa` до 1/64 px: advance целый с остатком ≈ 0,005 px — последняя колонка глифа покрыта на ≈ 99,5 %, визуально неотличимо.
- Высота ячейки и вертикали `│` опцией не затронуты (вариант Б).
- Коммит не делался.
