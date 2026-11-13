# План: целая ширина ячейки подбором размера шрифта (опция)

Статус: **реализовано и откачено 2026-10-23** (отклонено владельцем после проверки глазами, ADR-0019 Rejected) — см. `docs/done/2026-10-23-cell-width-snap.md`. Отклонение: `a` считается из единиц шрифта (`unitsPerEm`), а не из масштаба при 1 px — `skrifa` квантует ppem до 1/64 и масштаб в 16.16; размер шрифта примера — 14 px (правка владельца на диске до задачи).

- **ТЗ задачи:** `docs/spec/2026-10-23-cell-width-snap.md`.
- **Решения:** ADR-0019 (`docs/adr/0019-cell-width-snap.md`).
- **Опирается на:** ADR-0005 (целый номинальный размер — контракт `App`), ADR-0018 (режимы псевдографики, пример `pseudographica`).

## Факты по коду (проверено 2026-10-23)

- `font.rs`: `cell_metrics(ttf, size_px: u16)` — `Size::new(size_px as f32)`, `glyph_w = advance_M.round()`, `glyph_h = (ascent − descent).round()`, проверка 1…255; `FontFamily::metrics_at(u16)` — обёртка по файлу Regular.
- `rasterizer.rs`: `build_states(family, size_px: u16)` — `HintingInstance::new(&outlines, Size::new(size_px as f32), …)`; `set_size(u16)` — метрики, затем хинтеры, поля меняются после успеха; `size_px() -> u16` читает `App` (`window.rs: fit_rasterizer`, `apply_scale_factor`).
- `skrifa::instance::Size::new(f32)` принимает дробный размер и для метрик, и для хинтера.
- Пример `pseudographica`: аргументы идут в `Config::parse` напрямую — свой флаг снимается заранее, как `take_flag` в демо.

## Проектные решения (зафиксированы в ADR-0019)

1. Эффективный размер — внутреннее состояние `Rasterizer` (`effective_px: f32`, `snap_cell_width: bool`); публичный `size_px()` остаётся номинальным целым — `App`, атлас, бэкенды не меняются.
2. `FontFamily::snapped_size(nominal) -> f32`: `a = advance_M` при `Size::new(1.0)`, `w = max(1, round(nominal · a))`, `size = w / a`; `FontFamily::metrics_at_size(f32)`; `cell_metrics` получает дробный вариант `cell_metrics_at(ttf, f32)`, целый остаётся обёрткой.
3. `Rasterizer::set_snap_cell_width(bool)` перестраивает метрики и хинтеры под эффективный размер (тот же порядок, что `set_size`); `set_size` считает эффективный размер по текущему флагу. `effective_size_px()` — для лога.
4. Пример: флаг `--snap-cell` снимается до `Config::parse`; опция включается у растеризатора до `App::new`; статус дополняется «ячейка: целая/дробная», stderr — номинал, эффективный размер, ширина ячейки.

## Шаги

1. `font.rs`, `rasterizer.rs` (опция, дробный размер внутри). Проверка: `cargo test` — golden и `stage4_gl` без изменений (К1).
2. `tests/cell_snap.rs`: математика подбора при 11/16/22 px (К2); растеризатор с опцией — хинтеры при дробном размере, `set_size` сохраняет опцию, глиф рисуется, выключение возвращает номинал (К3).
3. Пример: флаг, лог, статус; запуск `--snap-cell --backend=soft --fail-frames=2` (К4).
4. Гейты, размеры, ADR-0019 + индекс, ТЗ 1.16 (FR-8, FR-24), `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md`, отчёты (К5).

## Открытые решения

Нет.

## Риски

- Качество хинтинга при дробном ppem — проверка глазами; опция по умолчанию выключена.
- Переключение опции на лету не делается: смена ширины ячейки требует перестройки окна (путь DPI).
