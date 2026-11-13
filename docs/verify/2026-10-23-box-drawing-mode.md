# Проверка критериев приёмки: режим псевдографики и пример `pseudographica`

ТЗ задачи — `docs/spec/2026-10-23-box-drawing-mode.md`, §4. Второй проход (BP-09): каждый критерий сверен с кодом, тестом или сохранённым выводом прогона 2026-10-23.

| № | Критерий | Статус | Подтверждение |
|---|---|---|---|
| К1 | Умолчание не меняет вывод | ✅ | `BoxDrawingMode::default() == Procedural` (`rasterizer.rs:24`, проверяется в `modes_pick_their_source`); `cargo test` 175/0, golden без изменений, прежние 8 сцен `stage4_gl` identical |
| К2 | `Font` берёт глиф шрифта и отличается от формулы; `Combined` при наличии глифа = шрифт; без глифа — `Font` пусто, `Combined` = формула | 🔶 | `modes_pick_their_source`: 138 из 160 знаков отличаются, `Combined` = `Font` на всех 160; ветка «без глифа» не покрыта — шрифт репозитория содержит весь диапазон (оговорка ТЗ К2 сработала, отмечено в отчёте и `TODO.md`); логика ветки — `rasterize` (`rasterizer.rs`, `!drawn && is_box && Combined`) |
| К3 | `is_style_independent` по режиму; атлас в `Font`/`Combined` заполняет каналы по начертаниям | ✅ | `style_independence_follows_mode`; мутация без учёта режима поймана |
| К4 | Смена режима на лету сбрасывает атлас и перерисовывает кадр на обоих бэкендах | ✅ | `switching_mode_redraws_the_whole_frame` (Software, `resets` 0 → 1, кадр = с нуля и обратно); `stage4_gl` — сцены `box font`/`box combined` identical на GL 3.3 и GLES 3.0 после `set_box_drawing_mode` на том же атласе; `events_common::Probe` — `Request::box_drawing` на втором кадре, оконные бинарники ok; мутация без сброса поймана |
| К5 | Пример: три таблицы, статус сверху, подсказка снизу, клавиши `1/2/3`, `q/w/e`, `Esc` | 🔶 | код `examples/pseudographica.rs` (`TABLES`, `draw_table`, `draw_status`, `on_key`); запуск `--backend=soft --fail-frames=2` — шрифт `Cascadia Mono`, кадр нарисован; окно глазами — за владельцем |
| К6 | Гейты, размер, документы | ✅ | fmt, clippy `--all-targets -D warnings`, 175 тестов; `demo.exe` 1 522 688 Б (без изменений), `pseudographica.exe` 1 147 392 Б; ADR-0018 + индекс, ТЗ 1.15 (FR-8.4, FR-25, §5.1), `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md` |

**След прохода.** Открыто заново 7 мест: `grep -n` по `BoxDrawingMode`, `set_box_drawing_mode`, `rasterize_font_glyph`, `is_style_independent`, `Request::box_drawing`; `wc -l` затронутых файлов; сохранённые выводы `stage4_gl` и `box_drawing_mode -- --nocapture`; `git status` golden; `ls -l target/release`; индекс ADR.

Отброшенных находок нет. Расхождений с ТЗ по контрактам §3 нет; К2 — частичное покрытие по оговорке самого ТЗ.
