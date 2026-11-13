# Верификация: этап 6 — мигание, размер шрифта и DPI, режимы сетки, полный экран, API событий

| Критерий ТЗ | Статус | Подтверждение |
|---|---|---|
| FR-3 флаги, курсор не флаг ячейки | ✅ | `CellFlags` без курсора; `golden_scene_has_no_cursor_in_the_library` |
| FR-7 мигание `BLINK`, период задаётся и меняется, `0` — выкл | ✅ | `Blinker::set_period`, `Blinker::visible_at`; 6 тестов `stage6_blink`; `demo --no-blink` |
| FR-8 `glyph_px = clamp(round(FONT_SIZE_PX · FONT_SCALE · scale_factor), 8, 48)`, `FONT_SCALE = 1.0` | ✅ | `font::glyph_px_for`/`glyph_px`; 7 тестов размера, включая `NaN`/`inf`/`0`/`-1`; `user_scale_is_a_constant_one_in_this_version` |
| FR-10 размер окна по числу ячеек, окно создаётся скрытым | ✅ | `App::window_attributes` (логический размер, `with_visible(false)`), `set_visible(true)` после первого кадра |
| FR-13 один пиксель глифа = один пиксель кадра, масштаба нет | ✅ | `u_scale`/`fill_block`/`ViewParams::scale` удалены; `cell_size_is_the_rasterization_metric_and_nothing_is_scaled`; golden 16/32 px |
| FR-15 мигание через uniform, без обновления текстуры | ✅ | `u_blink_phase` в `cell.frag` и обоих бэкендах; `blink_hides_ink_only_in_off_phase` |
| FR-16 дискретный размер шрифта — из DPI | ✅ | `apply_scale_factor` → `Backend::set_size`; `frame_after_a_dpi_change_equals_the_reference_dump` |
| FR-22 размер по сетке, `resizable`, заголовок, полный экран `F11` | ✅ (код) | `AppOptions::resizable`, `Request::fullscreen` → `Fullscreen::Borderless(None)`, `F11` в демо; визуально — ⚠ |
| FR-23 `AutoGrid` / `FixedGrid`, размер шрифта не подбирается | ✅ | `auto_grid`, `centered_origin`; `FixedGrid` — `Request::grid` (тест), `AutoGrid` — сетка из окна (тест); центрирование и обрезка в `soft_backend::layout` |
| FR-24 DPI как множитель размера растеризации, единственный источник геометрии | ✅ | `fit_rasterizer` при старте, `apply_scale_factor` при `ScaleFactorChanged` + `InnerSizeWriter::request_inner_size`; 4 теста `dpi_change`, 4-я сцена `stage4_gl` |
| FR-25 события через трейт `AppHandler` + `Ctx`, типы `winit` не переводятся, `Esc` не перехватывается | ✅ | `AppHandler` (6 методов), `Ctx { screen, request }`, `PointerEvent`; три интеграционных теста на настоящем окне; `Esc` обрабатывает демо |
| FR-26 перерисовка по запросу и по таймеру, в простое CPU не потребляется | ✅ | `about_to_wait`: `WaitUntil` только при `screen.has_blinking()`, иначе `Wait`; замеры 0,00 % без мигания на обоих бэкендах |
| FR-27 демо показывает сцену | ◐ (этап 7) | есть: строки с флагами, псевдографика, палитра 16 цветов, таблица глифов (латиница/греческий/кириллица — проверка ёмкости атласа), строка статуса с именем бэкенда, подсказка по клавишам, приглашение с курсором приложения. Нет: счётчика FPS и прокрутки — план этапа 6 (`docs/plans/2026-10-02-stage6-blink-dpi-events.md`) отнёс их к этапу 7 |
| FR-28 `1/2/3` — переключение бэкенда на лету, `F11`, `Esc` | ✅ | `Request::backend` → `switch_backend` с ограничением уровней; `stage6_fallback`: `["OpenGL 3.3", "Software"]` на одном окне |
| §1.3 единственный источник геометрии — метрики растеризации | ✅ | ADR-0005; `demo --debug`: «logical 16px → cell 10x21», «glyph_px at dpi 1.0 = 16» |
| §5.1 `blink.rs`, `RendererKind`, `config.rs` | ✅ | файлы на месте, `RendererKind` единственный в `renderer/mod.rs` |
| §5.3 п.5 повтор кадра, затем откат | ✅ | `App::redraw` с `frame_retried`; откат на лету проверен тестом, повтор кадра — по коду (⚠ этапа 5) |
| §7.2 `u_origin` — центрирование сетки | ✅ | `centered_origin` + 3 теста + сцены обрезки в `stage4_gl` |
| §7.3 Software — эталон, GL ≡ Software (допуск 0) | ✅ | `stage4_gl`: 4 сцены × 2 уровня GL, 0 отличий; в том числе после смены DPI на месте |
| §8 бит-идентичность рендереров | ✅ | тот же тест; golden совпадает после смены размера |
| §11.2 сравнение GL ↔ Software | ✅ | 4 сцены на обоих уровнях; без GL — печать причины и успех |
| §11.4 п.1 сборка без ошибок и предупреждений | ✅ | `cargo fmt --check` и `cargo clippy --all-targets -- -D warnings` чисто |
| §11.4 п.2 размер бинарника в пределах NFR-1 | ❌ (Linux) | 2 323 352 Б при лимите 2 МБ для Linux; было 2 298 120 Б до этапа 6 — см. ⚠ WARNINGS |
| §11.4 п.3 демо на GL и Soft, кадры совпадают | ✅ (программно) | stderr обоих режимов; совпадение кадров — `stage4_gl`; видимость окна — ⚠ |
| §11.4 п.4 `--fail-gl` | ✅ | stderr: оба GL-уровня отвергнуты, `backend Software` |
| §11.4 п.5 нет утечек за 10 минут | ✅ (2 мин) | RSS 142 588 → 142 592 КБ за 2 минуты с миганием; 10-минутный прогон — ⚠ |
| §11.4 п.6 CPU в простое (без мигания) < 1 % | ✅ | 0,00 % на GL 3.3 и на Software (окно измерения 10 с) |
| NFR-6 нет выделений в горячем пути кадра | ✅ | `soft_backend` переиспользует кадр; GL отдаёт указатели на атлас; `take_dirty_slots` без аллокаций в кадре |
| NFR-7 не паниковать при ошибках графики | ✅ | все шаги возвращают `Result`; `NaN`/`0`/`-2` в `glyph_px` дают базовый размер; `expect` остались только на инварианты (окно есть, шрифт в репозитории) |
| NFR-9 `unsafe` только в `gl_backend.rs` | ✅ | `grep unsafe src/` → только `src/renderer/gl_backend.rs` |
| §4.5 гейты | ✅ | fmt / clippy / 85 тестов + 3 интеграционных бинарника |
| §12 этап 6 | ✅ | все восемь пунктов содержания закрыты; `cargo run` работает |
| Переоткрытые места | — | `app.rs`, `font.rs`, `rasterizer.rs`, `atlas.rs`, оба бэкенда, `cell.frag`, `buffer.rs`, `demo.rs`, `Cargo.toml`, все шесть тестовых файлов — целиком; stderr всех ручных сценариев прочитан |

Расхождений с ТЗ не найдено, кроме NFR-1 на Linux (предел 2 МБ) — это существующий долг, а не следствие этапа: до него бинарник был 2 298 120 Б. Непроверенные в этом окружении сценарии (видимость окна, перенос между мониторами, полный экран, ввод, 10-минутная утечка) — в ⚠ WARNINGS `docs/done/`.
