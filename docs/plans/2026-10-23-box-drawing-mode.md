# План: режим псевдографики и пример `pseudographica`

Статус: **реализовано 2026-10-23** — см. `docs/done/2026-10-23-box-drawing-mode.md`; отклонений от плана нет (О1 — отдельный путь на атласе и бэкендах, О2 — системный шрифт 11 px).

- **ТЗ задачи:** `docs/spec/2026-10-23-box-drawing-mode.md`.
- **Решения:** ADR-0018 (`docs/adr/0018-box-drawing-mode.md`).
- **Опирается на:** ADR-0002 (процедурная псевдографика), ADR-0003 (каналы атласа по начертаниям), ADR-0004 (`Request`), ADR-0005 (путь `set_size` на месте), ADR-0017 (пример без `assets/`).

## Факты по коду (проверено 2026-10-23)

- `Rasterizer::rasterize` (`rasterizer.rs:187`): SVG → `box_drawing::draw` (безусловно для U+2500–U+259F; `draw` сам проверяет диапазон и обнуляет буфер) → контур шрифта начертания-источника. `is_style_independent` (`:104`) и `has_font_glyph` (`:109`) считают псевдографику «всегда есть, одинакова во всех каналах».
- `Atlas::fill_channel` (`atlas.rs:273`) заполняет четыре канала, если `is_style_independent`; `Atlas::ensure` (`:190`) заводит слот только при `has_glyph`. `Atlas::set_size` → `clear_page` (`resets += 1`).
- `GlBackend::set_size` (`gl_backend.rs:425`) → `after_rasterizer_change` (`needs_full_upload`); `SoftBackend::set_size` (`soft_backend.rs:175`) → `atlas_resets_seen`, `needs_full_redraw`, `is_overlay_valid = false`. `Backend::set_size` (`app/backend.rs:111`) — диспетчер по бэкенду; `App::apply_request` (`app/events.rs:102`) применяет `grid`, `fullscreen`, `backend`, `redraw`, `exit`.
- `Rasterizer` живёт в `App::rasterizer` до первого бэкенда и переезжает между бэкендами через `into_rasterizer` — состояние растеризатора переживает смену уровня и DPI.
- `tests/stage4_gl.rs`: таблица `Scene` с полями `size_px`, `via_dpi`, `svg`, `overlay`; Software создаётся на сцену заново, GL получает новый растеризатор только при смене размера. `tests/events_common::Probe` шлёт `request.grid` и `request.backend` из `on_backend_changed`/`on_frame`.
- Шрифт репозитория: наличие глифов U+2500–U+259F в `cmap` не проверялось — тесты находят присутствующую и отсутствующую кодовые точки перебором.

## Проектные решения (зафиксированы в ADR-0018)

1. `rasterizer::BoxDrawingMode { Procedural (Default), Font, Combined }`; поле `box_drawing` в `Rasterizer`, `set_box_drawing_mode`/`box_drawing_mode`. `rasterize`: `Procedural` — прежний порядок; `Font` — путь шрифта; `Combined` — шрифт, при `false` — формула. Путь шрифта выделен в `rasterize_font_glyph`.
2. `is_style_independent` для псевдографики — только в `Procedural`; `has_font_glyph` для псевдографики — `true` кроме режима `Font` (там — по `cmap`).
3. `Atlas::set_box_drawing_mode` — режим растеризатора + `clear_page` при смене; `GlBackend`/`SoftBackend::set_box_drawing_mode` — как после `set_size`; `Backend::set_box_drawing_mode` — диспетчер; `Request::box_drawing: Option<BoxDrawingMode>` → `apply_request` (бэкенд или `App::rasterizer` до бэкенда) + `request_redraw`.
4. Пример `examples/pseudographica.rs`: `FixedGrid` 88×13, системный шрифт, `font_size_px = 11`, три таблицы 22×5 с общей функцией отрисовки по набору линий (`Lines`), строка дополнений (стыки одиночных с двойными, штриховые, дуги, тени и блоки), статус в строке 0, подсказка в последней; клавиши `1/2/3`, `q/w/e` (и `й/ц/у` в русской раскладке — те же физические клавиши), `Esc`.

## Шаги

1. **Библиотека.** `rasterizer.rs`, `atlas.rs`, `gl_backend.rs`, `soft_backend.rs`, `app/backend.rs`, `app/api.rs`, `app/events.rs`, `lib.rs`. Проверка: `cargo test` без изменений эталонов (К1).
2. **Тесты.** `tests/box_drawing_mode.rs`: режимы на шрифте репозитория (К2), каналы атласа по режиму (К3), кадр Software после смены режима (К4); `stage4_gl` — две сцены `box font 16px` и `box combined 16px` (поле `box_mode` у `Scene`); `events_common::Probe` — `Request::box_drawing` на втором кадре.
3. **Пример и манифест.** `examples/pseudographica.rs`, `[[example]]`; запуск `--backend=soft --fail-frames=2` (кадр нарисован).
4. **Гейты, размер, документы.** fmt, clippy, test; размеры release; ADR-0018 — последствия; ТЗ 1.15 (FR-8.4, FR-25, §5.1, §13), `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md`; отчёты `done`/`test`/`verify`.

## Открытые решения

Нет.

## Риски

- У шрифта репозитория может не быть глифов рамок: тесты К2 тогда проверяют только ветку «нет глифа» (`Font` → пусто, `Combined` = `Procedural`) и печатают, что ветка «есть глиф» не покрыта; сцены `stage4_gl` при этом всё равно сравнивают GL ↔ Software.
- Смена режима сбрасывает всю страницу атласа — первый кадр после смены растеризует все видимые глифы заново (как при смене DPI).
