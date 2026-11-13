# Выполнение: этап 7 — демо FR-27, замеры `--bench`, профиль release

- План: `docs/plans/2026-10-02-stage7-demo-bench-release.md`; ТЗ 1.4: NFR-1 (лимит снят), FR-27, FR-27.1 (новый), §5.1, §5.2, §10.2, §11.4 п.2, §12 этап 7. Новый ADR: `0006-frame-stats-blink-phase-in-ctx.md` (ПОЧЕМУ — там); индекс ADR обновлён. Расширение рамок по BP-07.6 (согласовано владельцем 2026-10-02): `src/blink.rs`, `tests/stage6_blink.rs` — дефект этапа 6.

## WHAT

| Файл | Изменение |
|---|---|
| `src/app.rs` | `FrameStats { frames, cpu, total }`; `Ctx` + `blink_phase`, `stats`; `AppHandler::on_frame` (тело по умолчанию) — вызывается после успешного кадра из `redraw`; замер в `render_frame` (GL: `cpu` = `last_cpu_time()`, Software: `cpu` = `render` без `present`); `AppOptions::vsync` → `GlOptions::vsync`; две унаследованные правки clippy (`then` → `then_some`, строки 417/918) |
| `src/renderer/gl_backend.rs` | `GlOptions::vsync` (по умолчанию `true`) → `SwapInterval::Wait(1)`/`DontWait`; поле `last_cpu_time` и метод `last_cpu_time()`; `render` замеряет `render_into` до `swap_buffers` |
| `src/blink.rs` | **исправление дефекта этапа 6**: поле `odd` — отсчёт сдвинут на нечётное число фаз; `visible`/`visible_at` учитывают его; `catch_up` переключает `odd` при нечётном числе пропущенных фаз. До исправления после каждого дедлайна фаза снова была «видимой» — символы `BLINK` в приложении никогда не гасли |
| `src/bin/demo.rs` → `src/bin/demo/{main,common,scene,bench}.rs` | разбиение: `main.rs` — `Demo: AppHandler`, аргументы, курсор **в противофазе** (`cursor_visible = no_blink \|\| !ctx.blink_phase`), строка 0 с FPS/`cpu`/кадр по тику; `common.rs` — шрифт и лог, `take_flag`, `parse_config`, `demo_options`/`demo_view`, `handle_common_key` (`Esc`/`F11`/`1/2/3`), `write_status`, `FpsMeter`; `scene.rs` — перенос статической сцены без изменения содержимого (кроме подписи строки `BLINK`); `bench.rs` — `--bench`: `Bench: AppHandler`, сетка 200×60 (GL/GLES) или 120×40 (Software), 30 кадров прогрева + 300 замера, полная перезапись сетки каждый кадр, VSync выключен, отчёт `cpu`/`total`/FPS (mean/p50/p95/max) в stderr, код выхода 0 |
| `src/bin/full_table/main.rs` | новый бинарник (FR-27.1): перечень кодовых точек BMP с глифом (`Rasterizer::has_glyph`, 3369 у RobotoMono Nerd Font), страницы по 24×80 = 1920 глифов (`PgUp`/`PgDn`/`Home`/`End`, колесо), блок 16 фонов × 16 цветов символа (палитра ANSI), строка статуса, мигающая метка в заголовке (чтобы тикал таймер); `common.rs` через `#[path = "../demo/common.rs"]` |
| `Cargo.toml` | `[profile.release]` целиком по §10.2: `opt-level = "z"`, `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = true`, `debug = false` |
| `tests/events_common/mod.rs` | `Log` + `phases`, `frames`, `frame_events`; `Probe::on_frame`; на тике пишется `ctx.blink_phase`; выход — когда тиков достаточно **и** обе фазы видели (запас 20 тиков); `assert_frame_stats`; `vsync: true` в `AppOptions` |
| `tests/stage6_events.rs`, `stage6_events_auto.rs`, `stage6_fallback.rs` | вызов `assert_frame_stats` (обе фазы, `frame_events == frames`, `frames ≥ 1`) во всех трёх сценариях |
| `tests/stage6_blink.rs` | новый тест `catch_up_on_an_odd_number_of_phases_keeps_the_parity`: шесть дедлайнов подряд и пропуск трёх фаз разом |
| `docs/TZ_terminal_emulator.md` | 1.4: NFR-1, FR-27 (без прокрутки; FPS/`cpu`; противофаза; `--bench`), FR-27.1, §5.1 (структура `src/bin/`), §5.2 (`FrameStats`, `Ctx`, `on_frame`), §10.2 (зафиксирован), §11.4 п.2 |
| `AGENTS.md`, `TODO.md`, `PROJECT_MAP.md` | состояние этапов 1–7, команды `full_table`/`--bench`, абзац NFR-1 без лимита, список оставшегося |

Устаревший пункт TODO «`slot_to_codepoint` в демо → обратная таблица в `Atlas`» снят без кода: функции в демо нет с этапа 6, обратная таблица `codepoint_of_slot` в `Atlas` уже есть.

### Дополнение 2026-10-03: замена шрифта на вариант Nerd Font Mono

Визуальная проверка `full_table` владельцем: иконки обрезаны справа и сливаются с соседними. Разбор (DBG-07): в сборке «RobotoMono Nerd Font» (без суффикса Mono) иконки нарочно шире своего advance (9,6 px) — контур типично 0…12 px при ячейке 10 px, максимум на две ячейки; рендерер по FR-9/FR-13 рисует глиф строго в своей ячейке (`rasterizer.rs:193-202`, zeno обрезает всё вне маски), поэтому правые 20–50 % иконок отрезались. Временная диагностика по всем глифам: 2000 из 2321 иконок PUA выходили за ячейку по x, по y — ни одна. Решение владельца — вариант **Mono** (иконки заранее ужаты в один advance).

| Файл | Изменение |
|---|---|
| `assets/fonts/RobotoMonoNerdFontMono-{Regular,Bold,Italic,BoldItalic}.ttf` | заменяют `RobotoMonoNerdFont-*.ttf`; релиз Nerd Fonts v3.5.1 (`RobotoMono.zip` с GitHub), 2,57–2,58 МБ каждый |
| `src/font.rs` | `FONT_FAMILY = "RobotoMono Nerd Font Mono"` |
| `tests/stage2_font.rs` | четыре константы имён файлов |
| `docs/TZ_terminal_emulator.md` | FR-8 (константа и почему Mono), §5.1 (имена файлов) |

После замены: глифов в BMP 5019 (PUA 3715, страниц в `full_table` — 3), за ячейку по x выходят 2 иконки на 0,6 px (остальные 19 — типографские выступы вроде ď, как и раньше). Golden-эталоны 16/32 px **не изменились**: буквы и псевдографика у Mono-варианта и у v3.5.1 те же.

## COMPLETENESS

| Требование | Код | Тест / замер |
|---|---|---|
| FR-27 счётчик FPS и имя бэкенда в строке статуса | `common::write_status`, `FpsMeter`, `Demo::on_tick` | оконные тесты: `stats.frames` растёт, `on_frame` по разу на кадр; глазами — ⚠ |
| FR-27 курсор в противофазе с `BLINK` | `Demo::sync_cursor_phase` ← `ctx.blink_phase` | оконные тесты: обе фазы наблюдаются (`[true, false, true]`); `stage6_blink` ×7; глазами — ⚠ |
| FR-27 прокрутка | снята из ТЗ решением владельца | — |
| FR-27 `--bench` NFR-4 / NFR-5 | `bench.rs`, `AppOptions::vsync`, `GlBackend::last_cpu_time` | GL 3.3 200×60: `cpu` 0,578 мс mean / 0,629 p95 (≤ 1 мс ✅); GLES 3.0: 0,568 / 0,623 ✅; Software 120×40: 97,9 FPS (≥ 60 ✅) |
| FR-27.1 `full_table`: все глифы постранично, 16×16 цветов | `src/bin/full_table/main.rs` | smoke-запуск release: 3369 глифов, 2 страницы, бэкенд OpenGL 3.3, окно живёт до таймаута; листание — глазами ⚠ |
| FR-28 клавиши в обоих бинарниках | `common::handle_common_key` | `demo --fail-gl` smoke: откат к Software в stderr |
| §10.2 профиль release целиком | `Cargo.toml` | `cargo build --release` чисто; `demo.exe` 1 078 272 Б, `full_table.exe` 1 068 544 Б |
| NFR-1 размер замерен и записан | — | таблица в `docs/test/…stage7`; лимита нет (ТЗ 1.4) |
| §5.2 `on_frame`, `Ctx.blink_phase`, `Ctx.stats` | `app.rs` | три оконных теста |
| FR-7 (дефект этапа 6) `BLINK` гаснет и зажигается | `blink.rs` `odd` | `catch_up_on_an_odd_number_of_phases_keeps_the_parity`; оконные тесты видят обе фазы |
| §11.4 п.1 без ошибок и предупреждений | две правки clippy в `app.rs` | `fmt --check`, `clippy --all-targets -D warnings` чисто на stable 1.98.1 |

## CORRECTNESS

- Тест фаз падал до исправления `blink.rs` (23 тика, все `true`) и проходит после (`[true, false, true]`) — он проверяет поведение, а не подогнан под него. Репро с инъекцией времени (`catch_up(+501)` → `visible_at(+502)` было `true`) удалён после подтверждения; та же проверка закреплена в `stage6_blink`.
- `frame_events == frames` — `on_frame` вызывается ровно по разу на успешный кадр; обе величины берутся из одного события.
- Бит-идентичность GL ↔ Software не затронута: `stage4_gl` — 4 сцены на обоих уровнях, 0 отличий (алгоритм не менялся, добавлен только замер времени вокруг `render_into`).
- Первый замер `--bench` с VSync дал `cpu` = 16,1 мс на GL — равен периоду монитора; разобрано (ожидание в первом GL-вызове следующего кадра), добавлен `vsync = false` для замера, повтор дал 0,58 мс. Цифра NFR-4 — из второго прогона, с объяснением в ADR-0006.
- Golden-эталоны не менялись: сцена Software не затронута.

## EVIDENCE

```
cargo fmt --check                              → чисто
cargo clippy --all-targets -- -D warnings      → чисто (до этапа падал: 2 × unnecessary_lazy_evaluations в app.rs)
cargo test                                     → 86 тестов harness'а (9+33+20+1+7+9+7) + 3 бинарника harness=false, всё ok
  stage6_events:   FixedGrid: OpenGL 3.3, 3 тика, 3 кадра, фазы [true, false, true]
  stage6_fallback: ["OpenGL 3.3", "Software"], 4 тика, 4 кадра
cargo build --release                          → demo.exe 1 078 272 Б, full_table.exe 1 068 544 Б
demo --bench (GL 3.3, 200×60)                  → cpu mean 0.578 ms · p50 0.554 · p95 0.629 · max 3.679; fps 1098.7
demo --bench --backend=gles (200×60)           → cpu mean 0.568 ms · p50 0.560 · p95 0.623 · max 0.759; fps 1096.5
demo --bench --backend=soft (120×40)           → cpu mean 9.232 ms · p95 10.779; total mean 9.969 ms; fps 97.9
full_table (release, 8 с)                      → «глифов в BMP: 3369, страниц по 1920: 2», backend OpenGL 3.3
demo --fail-gl (release, 5 с)                  → оба GL-уровня отвергнуты, backend Software
```

Дополнение 2026-10-03 (шрифт Mono):
```
диагностика до замены   → PUA: за ячейку по x 2000 из 2321, макс. выход 9,5 px; по y 0
диагностика после       → глифов 5019, PUA 3715; за ячейку по x 21 (PUA 2, макс. 0,6 px)
cargo fmt --check / clippy --all-targets -- -D warnings → чисто
cargo test              → 86 + 3 оконных, всё ok; golden 16/32 px без изменений
cargo build --release   → пересобрано; full_table запущен владельцу для повторного осмотра
```
Временные тесты `tests/tmp_glyph_widths.rs` (две редакции) удалены, контрольный прогон после удаления — зелёный.

Проверочный проход (BP-09): переоткрыты все изменённые файлы (`git status` — 14 изменённых + 4 новых пути); маркеров `TODO/FIXME/HACK/WORKAROUND` в затронутых файлах нет; фраз-отсрочек ADR в `docs/` нет (grep); `unsafe` только в `gl_backend.rs` (в `renderer/mod.rs` — слово в doc-комментарии, код без `unsafe`). Временный репро-тест `tests/tmp_blink_repro.rs` удалён (подтверждено `ls tests`). Лог первого (красного) прогона сохранён отдельно от второго.

## ⚠ WARNINGS

- **Визуально ничего не проверено** (окно без человека): противофаза курсора, обновление строки статуса, листание `full_table`, читаемость блока 16×16 — владельцу (§11.3, TODO этапа 7).
- **Замеры — одна конфигурация:** Windows 10, NVIDIA GTX 1060 (драйвер 581.57), `glyph_px = 16`, 100 % DPI. На Linux/Mesa и при высоком DPI (сетка 200×60 → 3400×2160 px при `glyph_px = 28`, окно обрежется) цифры другие; замер Software при обрезанной сетке завышен — `--bench` печатает логический размер окна для контроля.
- **`cpu` на GL — без выполнения на GPU** (draw call асинхронен); это соответствует формулировке NFR-4 («сборка данных + `glTexSubImage2D`»), но не время кадра на GPU. `max` 3,68 мс на GL 3.3 — единичный выброс (p95 0,63).
- **`total`/FPS на GL без VSync** (≈ 1100 FPS) — верхняя оценка; в обычном режиме VSync включён.
- **Размер release на Linux с полным профилем не перемерен** (нет Linux-машины здесь); последняя цифра — 2 323 352 Б через `--config` на этапе 6. Лимита нет, но §15 п.5 требует таблицу — этап 8.
- **`app.rs` 1081 строк, `gl_backend.rs` 947** — порог QG-06 превышен до этапа (1027 и 926), этап добавил 54 и 21 строку по теме файлов; разбиение — отдельная задача, если понадобится.
- **`full_table` строит перечень глифов 65 тыс. вызовами `has_glyph`**, каждый заново разбирает `FontRef`; на этой машине старт незаметен, замер не делался. Пункт в TODO.
- **Пропуск фаз в оконном тесте:** проверка «обе фазы видели» ждёт до 20 запасных тиков, если таймер ОС пропускает переключения; на этой машине фазы чередуются без пропусков.
- **Строки с `windows_subsystem = "windows"`**: в release на Windows stderr `--bench` виден только при перенаправлении (`2> bench.txt`) — записано в `AGENTS.md` и в комментарии `main.rs`.
