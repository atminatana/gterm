# План: автохинтер со сглаженной целью и предпочтение файла с родными инструкциями

Статус: **реализовано и закрыто владельцем 2026-10-27** — см. `docs/done/2026-10-27-autohint-smooth.md`; отклонений от плана нет.

- **ТЗ задачи:** `docs/spec/2026-10-27-autohint-smooth.md`.
- **Решения:** ADR-0025 (`docs/adr/0025-autohint-smooth-native-first.md`).
- **Опирается на:** ADR-0002 (цель `Mono` для интерпретатора), ADR-0003 (цепочка подстановки), ADR-0017 (источники семейства).

## Факты по коду (проверено 2026-10-27)

- `font/system.rs::from_system_database`: пути семейства `sort()` + `dedup()` → `from_files`; `from_files` сохраняет порядок, `from_faces` берёт `faces.iter().position(|f| f.style == style)` — первый файл стиля.
- `rasterizer.rs::hinting_engine`: `maxp.maxSizeOfInstructions > 0 && prefer_interpreter()` → `Interpreter`, иначе `Auto(None)`; `build_states` ставит `Target::Mono` для любого движка.
- Автохинтер skrifa 0.47 (`autohint/metrics/mod.rs:363-378`): `Mono` → `HORIZONTAL_SNAP | VERTICAL_SNAP | STEM_ADJUST | MONO`; `Smooth/Normal` → только `STEM_ADJUST`; `Light` → `NO_HORIZONTAL`, без `STEM_ADJUST`.
- Пробы (временный крейт `target/scratch_hint`, тот же путь `skrifa` + `zeno`): `CascadiaMono-Regular.ttf` 14 px — `Mono` ≡ `Smooth` бит в бит (интерпретатор); `CascadiaMono-Regular.otf` 14 px — `Mono`: стойки `к` 2 px, `С` с изломом; `Smooth/Normal`: стойки 1 px; RobotoMono 16 px — `Mono`: `С` 8 колонок из 10, `Smooth/Normal`: 9 из 10, стойка `к` 1 px; `Light`: стойка `к` в две серые колонки.
- Базовый прогон до изменений: `cargo test --workspace` — 27 наборов, 0 падений, код 0.
- Тесты уже правят таблицы TTF в памяти (`with_narrow_i` в `stage2_font.rs`): каталог таблиц, `skrifa` контрольные суммы не проверяет. `maxp` версии 1.0: `maxSizeOfInstructions` — u16 по смещению 26.

## Проектные решения (зафиксированы в ADR-0025)

1. Признак «глифы несут родные инструкции» — одна функция `has_glyph_instructions(&FontRef)` в `font.rs`; ею пользуются `from_faces` (предпочтение файла) и `hinting_engine` (выбор движка).
2. Предпочтение действует внутри одного стиля; порядок среди равных прежний (сортировка путей / порядок списка).
3. Цель хинтинга по движку: `Interpreter` → `Mono`, `Auto` → `Smooth { Normal }`.

## Шаги

1. `font.rs`: `has_glyph_instructions`; `from_faces` — выбор индекса стиля с предпочтением. `rasterizer.rs`: `hinting_engine` через общую функцию; `build_states` — цель по движку.
2. Тесты К1 и К2 в `tests/stage2_font.rs`; `GTERM_UPDATE_GOLDEN=1 cargo test --test stage3_render golden`; `cargo test --workspace`.
3. Пример виджетов (строка лога — `.ttf`), дамп растра; размеры release.
4. ADR-0025 + индекс, ТЗ 1.20, `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md`, отчёты; временный крейт удалить.

## Открытые решения

Нет.

## Риски

- Все golden меняются (шрифт репозитория без инструкций); перезапись осознанная.
- Стойки на дробных позициях в `Smooth/Normal` — серые края.
