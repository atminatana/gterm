# Проверка критериев приёмки: автохинтер со сглаженной целью и предпочтение файла с родными инструкциями

ТЗ задачи — `docs/spec/2026-10-27-autohint-smooth.md`, §4. Второй проход (BP-09): каждый критерий сверен с кодом, тестом или сохранённым выводом прогона 2026-10-27.

| № | Критерий | Статус | Подтверждение |
|---|---|---|---|
| К1 | Среди двух Regular побеждает файл с инструкциями при любом порядке | ✅ | `face_with_glyph_instructions_is_preferred_regardless_of_order`: оба порядка, все четыре начертания указывают на копию с инструкциями; `from_faces` (`font.rs`) — `find(has_instructions)`, затем первый |
| К2 | Автохинтер в `Smooth/Normal`: растр = прямой рендер skrifa, ≠ `Mono` | ✅ | `uninstructed_font_is_autohinted_with_smooth_target` на `к`, `С`; мутация на `Mono` поймана |
| К3 | Путь интерпретатора не изменён | ✅ | `hinting_target` (`rasterizer.rs`): `Interpreter` → `Target::Mono`; `hinting_engine` — то же условие через `has_glyph_instructions`; лог примера виджетов — `CascadiaMono-Regular.ttf` |
| К4 | GL ↔ Software identical после перезаписи golden | ✅ | `stage3_render golden` 3/3; `stage4_gl` 10 сцен identical на GL 3.3 и GLES 3.0 |
| К5 | Пример виджетов: стойки 1 px, пропорции букв как в файле | ✅ | дампы 14 px: `.otf`+`Mono` — стойки 2 px (как на скриншоте), `.ttf`+интерпретатор — ≈ 1,3 px; глазами — подтверждено владельцем 2026-10-27 («всё хорошо») |
| К6 | Гейты, размер, документы | ✅ | fmt, clippy, 203 теста; `demo.exe` 1 543 168 Б; ADR-0025 + индекс, ТЗ 1.20 (FR-8.4), `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md` |

**След прохода.** Открыто заново 7 мест: `grep -n` по `has_glyph_instructions`, `hinting_target`, `hinting_engine`, `from_faces`; лог примера виджетов до/после; вывод `stage4_gl --nocapture`; побайтовое сравнение golden `HEAD` ↔ рабочая копия; размеры release.

Отброшенных находок нет. Расхождений с ТЗ нет; критерий К3 «прогон примера берёт `.ttf`» подтверждён логом, автотеста на инструктированный шрифт нет (`TODO.md`).
