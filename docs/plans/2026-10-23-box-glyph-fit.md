# План: растяжение глифов псевдографики под ячейку (вариант Б)

Статус: **реализовано 2026-10-23** — см. `docs/done/2026-10-23-box-glyph-fit.md`. Отклонение: вписываемый контур — без хинтинга (п.3 ADR-0020), иначе `│` не доходит до низа ячейки.

- **ТЗ задачи:** `docs/spec/2026-10-23-box-glyph-fit.md`.
- **Решения:** ADR-0020 (`docs/adr/0020-box-glyph-fit.md`).
- **Опирается на:** ADR-0018 (режимы, `rasterize_font_glyph`), ADR-0002 (процедурный путь не трогается), ADR-0019 (вариант А отклонён).

## Факты по коду (проверено 2026-10-23)

- `rasterize_font_glyph` (`rasterizer.rs:239`): контур начертания-источника через `DrawSettings::hinted` в `PathPen(Vec<zeno::Command>)`, затем `zeno::Mask::new(..).size(w, h).origin(BottomLeft).offset((0, h − baseline))` — координаты пера: пиксели при размере растеризации, y вверх, базовая линия в 0; ячейка — `x ∈ [0, w)`, `y ∈ [baseline − h, baseline)`.
- `zeno::Command` — `MoveTo(Vector)`, `LineTo(Vector)`, `QuadTo(Vector, Vector)`, `CurveTo(Vector, Vector, Vector)`, `Close`; `Vector { x: f32, y: f32 }` — поля публичные.
- Метрики: `font.glyph_metrics(size, loc).advance_width(gid)`, `font.metrics(size, loc).ascent / descent` (descent < 0) при `Size::new(size_px as f32)`.
- `rasterize` уже знает `is_box` и режим: путь шрифта вызывается для рамок только в `Font`/`Combined`.

## Проектные решения (зафиксированы в ADR-0020)

1. Преобразование — в `rasterize_font_glyph` по флагу `fit_to_cell` (истина для `is_box` в `Font`/`Combined`); реализовано свободной функцией над командами пера `fit_commands_to_cell(commands, sx, y_from, y_to, sy)`.
2. Масштаб по метрикам строки, не по габаритам глифа: `sx = w / advance`, `sy = h / (ascent − descent)`, `y' = (baseline − h) + (y − descent) · sy`.
3. Хинтинг сохраняется (вариант В — отдельно).

## Шаги

1. `rasterizer.rs`: параметр `fit_to_cell`, функция преобразования, вызов из `rasterize`. Проверка: `cargo test` (К1, К3, К4).
2. Тесты К2 в `tests/box_drawing_mode.rs`: покрытие краёв `─`, `│`, `┼` в `Font`; сначала замер фактического покрытия на шрифте репозитория (`--nocapture`), затем утверждения.
3. Запуск примера на Software с инжекцией (кадр рисуется); размеры release.
4. Документы: ADR-0020 + индекс, ТЗ 1.16, `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md`, отчёты.

## Открытые решения

Нет (допущения п.2–3 помечены в ТЗ как обратимые).

## Риски

- Серые тонкие линии после растяжения хинтованного контура — глазами; при плохом результате — вариант В.
