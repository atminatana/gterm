# Отчёт о выполнении: автохинтер со сглаженной целью и предпочтение файла с родными инструкциями

- **Задача:** `2026-10-27-autohint-smooth`. ТЗ — `docs/spec/2026-10-27-autohint-smooth.md`, план — `docs/plans/2026-10-27-autohint-smooth.md`.
- **Решение и его ПОЧЕМУ:** ADR-0025.
- **Статус:** закрыто владельцем 2026-10-27 («все тесты пройдены»); проверка глазами (К5) подтверждена владельцем 2026-10-27 («всё хорошо»).

## Что сделано

| Шаг плана | Результат |
|---|---|
| 1. Библиотека | `font.rs`: `has_glyph_instructions` (`maxp.maxSizeOfInstructions > 0`); `from_faces` — среди файлов одного начертания первым ищется файл с инструкциями, иначе первый по порядку. `rasterizer.rs`: `hinting_engine` через общий признак; `hinting_target` — `Interpreter` → `Mono`, `Auto` → `Smooth { Normal }`; `build_states` ставит цель по движку |
| 2. Тесты | `tests/stage2_font.rs`: `sources::face_with_glyph_instructions_is_preferred_regardless_of_order` (К1, копия Regular с правленым `maxp` через `with_instruction_budget`; `table_bounds` вынесен из `with_narrow_i`), `rasterization::uninstructed_font_is_autohinted_with_smooth_target` (К2, `к` и `С` при 16 px: растр = прямой рендер `Auto` + `Smooth/Normal`, ≠ `Mono`); golden `soft_golden_{16,32}px.bin` перезаписаны |
| 3. Пример, размеры | `widgets`: лог `font 'Cascadia Mono' — C:\Windows\Fonts\CascadiaMono-Regular.ttf` (было `.otf`); размеры release ниже |
| 4. Документы | ADR-0025 + индекс; ТЗ 1.20 (FR-8.4); `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md`; отчёты `test`/`verify`; временный крейт `target/scratch_hint` удалён |

Отклонений от плана нет. Дампы растра (временный крейт, тот же путь `skrifa` + `zeno`): `CascadiaMono-Regular.otf` 14 px, автохинтер `Mono` — стойки `к`/`н`/`т` по 2 px, `С` с изломом (совпадает со скриншотом владельца); `CascadiaMono-Regular.ttf` 14 px, интерпретатор — стойки ≈ 1,3 px, `Mono` ≡ `Smooth` бит в бит; RobotoMono 16 px: `Mono` — `С` 8 колонок из 10, `Smooth/Normal` — 9 из 10, стойка `к` 1 px.

## COMPLETENESS
- Предпочтение файла — код + тест К1; цель по движку — код + тест К2; путь интерпретатора не менялся (К3 — diff `build_states`, лог примера); golden перезаписаны, GL ↔ Software identical (К4).
- Глазами (К5) — за владельцем.

## CORRECTNESS
- Golden до перезаписи упали ровно на пикселях букв: 16 px — 2 639 из 35 000 пикселей (7,5 %), 32 px — 4 305 из 124 974 (3,4 %); после перезаписи `stage3_render golden` 3/3, `rendering_the_same_scene_twice_is_bit_identical` не менялся.
- `stage4_gl --nocapture`: `OpenGL 3.3: GL ↔ Software identical, 10 scenes`, `OpenGL ES 3.0: GL ↔ Software identical, 10 scenes`.
- Мутация (исходник восстановлен копией, прогон после восстановления зелёный): `hinting_target` возвращает `Mono` для автохинтера → `uninstructed_font_is_autohinted_with_smooth_target` падает на `к smooth` (`stage2_font.rs:479`).
- К1 проверяет оба порядка файлов; на исходном Regular `has_glyph_instructions` = `false`, на копии с бюджетом 1 — `true`.

## EVIDENCE
- Базовый прогон до изменений: `cargo test --workspace` — 27 наборов, 201 тест, 0 падений, код 0.
- После изменений: `cargo fmt --check` чисто, `cargo clippy --all-targets -- -D warnings` чисто, `cargo test --workspace` — 27 наборов, **203 теста** (201 + 2), 0 падений, код 0; `stage2_font` 51.
- Размер release 2026-10-27: `demo.exe` 1 543 168 Б (+512 к 1 542 656), `full_table.exe` 1 536 512 (+2 560), `overlay.exe` 1 531 904 (+2 560), `minimal.exe` 1 164 800 (+2 048), `pseudographica.exe` 1 170 944 (+2 560), `widgets.exe` 1 189 888 Б (+3 072).
- LOC: `font.rs` 392, `rasterizer.rs` 430, `tests/stage2_font.rs` 1 045 (порог не задан проектом; файл тестов рос и раньше — один набор этапа 2).
- Временный крейт `target/scratch_hint` и файлы сравнения удалены до финальных гейтов.
- Проверочный проход BP-09: заново открыты 7 мест — `grep -n` по `has_glyph_instructions`, `hinting_target`, `from_faces`; лог примера виджетов; вывод `stage4_gl`; сравнение golden по байтам; размеры release. Отброшенных находок нет.

## ⚠ WARNINGS
- Глазами — за владельцем: пример виджетов на `Cascadia Mono` (`.ttf`), демо на шрифте репозитория.
- Инструктированного шрифта в репозитории нет: ветка «интерпретатор + `Mono`» подтверждена неизменностью кода и логом примера, автотестом не покрыта (см. `TODO.md`).
- `Smooth/Normal` оставляет часть стоек на дробных позициях (серые края с двух сторон) — принятая цена сохранения пропорций; `Light` отклонён (ADR-0025).
- Ширина ячейки остаётся целой (`round(advance)`): при 14 px `Cascadia Mono` правый край `К` упирается в границу ячейки 8 px — не в объёме задачи.
- Коммит не делался.
