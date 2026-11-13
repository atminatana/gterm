# Выполнение: этап 6 — мигание, размер шрифта и DPI, режимы сетки, полный экран, API событий

- План: `docs/plans/2026-10-02-stage6-blink-dpi-events.md`; ТЗ FR-7, FR-13, FR-16, FR-22…FR-28, §1.3, §5.3, §7.2, §7.3, §8, §12 этап 6, §11.4 п.5/п.6. Новые ADR: `0004-app-event-handler-trait.md`, `0005-font-size-drives-geometry.md`; индекс ADR и ТЗ (FR-23 дополнена уточнением про `Request::grid`) обновлены.

## WHAT

| Файл | Изменение |
|---|---|
| `src/blink.rs` | новый: `Blinker { period, since, visible }`, `DEFAULT_BLINK_PERIOD = 500 мс`; `visible_at`, `next_toggle`, `catch_up`, `set_period`; время инъектируется, поэтому вся логика проверяется без окна |
| `src/cursor.rs` | **удалён** — курсор рисует приложение (ADR-0004, §13) |
| `src/font.rs` | `FONT_SIZE_PX = 16`, `FONT_SCALE = 1.0`, `GLYPH_PX_MIN/MAX = 8/48`, `glyph_px_for(base, scale, factor)`, `glyph_px(factor)`, `FontFamily::metrics_at(size_px)`; не-конечный, нулевой и отрицательный DPI → базовый размер (NFR-7) |
| `src/rasterizer.rs` | `new_at(family, size_px)`, `size_px()`, `set_size()` + `build_states` — пересборка хинтеров на месте; `into_family()` для перехода между бэкендами |
| `src/atlas.rs` | `set_size(size_px)` с ранним выходом на том же размере, `set_rasterizer`, `apply_rasterizer`, `regrid`, `size_px`; страница переиспользуется, `resets += 1` (ADR-0003) |
| `src/renderer/mod.rs` | `ViewParams` без `scale` и `cursor`: только `border_color` и `blink_phase` |
| `src/renderer/cell.frag` | удалены `u_scale`, `u_cursor`, `fill_block`; ячейка = `glyph_w × glyph_h` 1:1 (FR-13) |
| `src/renderer/soft_backend.rs` | эталон §7.3 без масштаба: `layout()` (центрирование, обрезка), `set_size()`, `grid_size_px`, `atlas()`/`atlas_mut()`/`into_atlas()` |
| `src/renderer/gl_backend.rs` | те же uniform'ы, `set_size`, `after_rasterizer_change`, `set_rasterizer`; весь `unsafe` остался здесь (NFR-9) |
| `src/buffer.rs` | `blink_count`, `has_blinking()`, `recount()` — условие для таймера (FR-26) |
| `src/app.rs` | переписан: трейт `AppHandler` + `Ctx`, `Grid`/`GridMode`/`AppOptions`, чистые `auto_grid`/`centered_origin`, `notify`/`apply_request`, цепочка бэкендов, `switch_backend`/`restore_backend`, `fit_rasterizer` (старт) и `apply_scale_factor` (`ScaleFactorChanged` + `InnerSizeWriter`), `about_to_wait` с `WaitUntil` только при мигании, полный экран, `set_ime_allowed(true)`, `Esc` не перехватывается |
| `src/bin/demo.rs` | `Demo: AppHandler`: сцена FR-27, курсор через `INVERSE`, `1/2/3` — бэкенд, `F11`, `Esc`, `Backspace`, строки ввода; `--debug` печатает логическую ячейку и `glyph_px`; ключ демо `--no-blink` (замер §11.4 п.6) |
| `tests/blink`-содержимое | `tests/stage6_blink.rs` (6), `tests/stage6_app.rs` (9), `tests/stage6_events.rs`, `tests/stage6_events_auto.rs`, `tests/stage6_fallback.rs` (три бинарника с `harness = false`), `tests/events_common/mod.rs` |
| `tests/golden/` | `soft_golden_1x.bin` → `soft_golden_16px.bin`, новый `soft_golden_32px.bin` |
| `tests/stage4_gl.rs` | 4-я сцена `dpi 16px → 32px` — GL и Software совпадают после смены размера на месте |
| `Cargo.toml` | три `[[test]]` с `harness = false` (цикл событий `winit` — только на главном потоке и только один на процесс) |

## COMPLETENESS

| Требование | Код | Тест / замер |
|---|---|---|
| FR-7 мигание, период настраивается, `0` — выкл | `Blinker`, `Request::blink_period`, `demo --no-blink` | 6 тестов `stage6_blink` |
| FR-13 1:1 пиксель глифа и кадра, масштаба нет | ячейка из метрик, `u_scale`/`fill_block` удалены | golden 16/32 px, `cell_size_is_the_rasterization_metric…` |
| FR-16/FR-24 DPI как множитель размера растеризации | `glyph_px`, `fit_rasterizer`, `apply_scale_factor` | 4 теста `dpi_change`, 4-я сцена `stage4_gl` |
| FR-22 размер окна по сетке, `resizable`, заголовок, `F11` | `AppOptions`, `Request::fullscreen` | код; визуально ⚠ |
| FR-23 `AutoGrid`/`FixedGrid`, шрифт не подбирается | `auto_grid`, `centered_origin`, `Request::grid` только в `FixedGrid` | 3 + 3 теста, `stage6_events`, `stage6_events_auto` |
| FR-25 трейт, типы `winit` не переводятся, `Esc` не перехватывается | `AppHandler`, `Ctx`, `PointerEvent` | три интеграционных теста на настоящем окне |
| FR-26 перерисовка по запросу и по таймеру, простой без CPU | `about_to_wait` + `has_blinking` | замер 0,00 % без мигания на GL и Software |
| FR-28 `1/2/3` — переключение бэкенда на лету | `Request::backend` → `switch_backend` | `stage6_fallback`: `["OpenGL 3.3", "Software"]` |
| §11.2 бит-идентичность GL ↔ Software | без изменений в алгоритме, кроме удаления масштаба | 4 сцены × 2 уровня, 0 отличий |
| §11.4 п.6 CPU в простое < 1 % | таймер только при `BLINK` | 0,00 % (GL и Software), с миганием 0,20 % / 2,60 % |
| §11.4 п.5 нет утечек | — | RSS +4 КБ за 2 минуты |
| NFR-6 нет аллокаций в кадре | кадр и `scratch` переиспользуются | без изменений против этапа 3 |
| NFR-7 не паниковать | `Result` на каждом шаге, `glyph_px` чинит негодный DPI | `expect` только в `gl_backend.rs` (4 шт., все на инвариантах) |
| NFR-9 `unsafe` только в GL | — | `grep -rn unsafe src/` → только `gl_backend.rs` |

## CORRECTNESS

- Геометрия выводится из одного места: `glyph_px` → метрики растеризации → ячейка → сетка → окно. Целочисленного масштаба не осталось ни в шейдере, ни в рендерерах, ни в `ViewParams`.
- Смена DPI идёт **на месте**: страница атласа (16 МБ) переиспользуется, перестраиваются только хинтеры и сетка слотов, `resets` растёт → рендерер перезаливает страницу целиком. Кадр после смены DPI побитово равен эталону, построенному растеризатором с нуля, и совпадает с GL.
- `Atlas::set_size` на том же размере не делает ничего: без раннего выхода счётчик сбросов рос бы на каждом вызове и страница затиралась бы впустую.
- `Request::grid` действует только в `FixedGrid`. В `AutoGrid` он игнорируется: сетка всегда выводится из окна, иначе следующая же перерисовка вернула бы заданное значение обратно (поймано интеграционным тестом, уточнение внесено в FR-23).
- Откат на лету проверен на настоящем окне: приложение попросило `Software` на втором тике мигания — уведомления `["OpenGL 3.3", "Software"]`, буфер и таймер пережили смену.
- Окно создаётся скрытым и показывается после первого кадра: белой вспышки до первого кадра нет (FR-22).

## EVIDENCE

См. `docs/test/2026-10-02-stage6-blink-dpi-events.md`, `docs/verify/2026-10-02-stage6-blink-dpi-events.md`.

## ⚠ WARNINGS

- **NFR-1 на Linux превышен: 2 323 352 Б при лимите 2 МБ.** Это не регрессия этапа — до него было 2 298 120 Б (+25 232 Б, +1,1 %), и новых зависимостей этап не добавил. `opt-level = "s"` хуже (2 499 144 Б), `"z"` оставлен. Основные вклады по `nm --print-size`: `winit` 228 КБ, `skrifa` 192 КБ, `core` 160 КБ, `wayland_client` 142 КБ, `zeno` 115 КБ, `read_fonts` 110 КБ, `std` 102 КБ, `gterm` 90 КБ, `x11_dl` 77 КБ. Дальнейшее уменьшение возможно только отказом от Wayland или урезанием `winit`/`glutin` — это решение этапа 7, где профиль release финализируется. `[data needed: cargo bloat --release --crates для точной картины]`. На Windows лимит 1,5 МБ ранее измерялся как 1 051 136 Б; перемер на Windows-машине не делался.
- **Видимость окна в этом окружении не проверяется**: `xdotool` не видит ни одного клиентского окна (проверено на постороннем `xmessage`), `ffmpeg -f x11grab` даёт чёрный root. Окно создаётся, GL-контекст работает, кадры совпадают — но «сцена видна глазами» подтвердить нечем. Владельцу: смена мониторов 100 %↔200 %, `F11`, ввод, реакция на `1/2/3` (§11.3).
- **Смена DPI вживую не воспроизводилась** (один монитор). Покрыто тестом пути `set_size` и равенством GL ↔ Software после него; обработчик `ScaleFactorChanged` с `InnerSizeWriter` проверен по коду и по API `winit 0.30.13`, но не вживую.
- **Время перерисовки при смене DPI не измерено** (нужен второй монитор). Риск плана: пересборка хинтеров + ленивая растеризация видимых глифов на 28–48 px даёт заметную паузу при переносе окна; бюджет растеризации сознательно не введён (ADR-0005 п.8).
- **Software в простое с миганием — 2,6 % CPU**, потому что смена фазы меняет `view.blink_phase` и приводит к полной перерисовке 2,2 Мпикс (≈ 13 мс на кадр). Требование §11.4 п.6 сформулировано «без мигания» и выполнено; если 2,6 % неприемлем, нужен частичный redraw по строкам с `BLINK` в Software-рендерере — это отдельная задача, не этап 6.
- **Память проверена на 2 минутах, а не на 10** (RSS +4 КБ при мигании). Десятиминутный прогон — ⚠ §11.4 п.5.
- **Повтор кадра по §5.3 п.5** по-прежнему без автотеста (перенесено с этапа 5): ошибку рендера нечем инжектировать без тестового бэкенда.
- **Клавиатурный ввод и полный экран проверены только по коду.** Интеграционные тесты не посылают события ввода: `winit` не имеет синтетического ввода без ОС-уровня, а этап 6 ограничен ручной проверкой.
- **Отсутствие белого окна до первого кадра и «прыжок» окна при переносе мониторов** — визуальные свойства, здесь не проверяются (см. первый ⚠).
