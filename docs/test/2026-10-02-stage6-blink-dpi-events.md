# Тесты: этап 6 — мигание, размер шрифта и DPI, режимы сетки, полный экран, API событий

## Состав

### Новые бинарники

| Файл | Проверяет |
|---|---|
| `tests/stage6_blink.rs` | 6 тестов таймера: период по умолчанию 500 мс, смена фазы по времени, дедлайн и `catch_up`, стабильность фазы внутри периода, `period = 0` — всегда видимая фаза, смена периода на лету без потери начала отсчёта |
| `tests/stage6_app.rs` | 9 тестов: 3 геометрии сетки (`auto_grid`, `centered_origin`, вырожденная ячейка), 4 пути смены DPI (`set_size` совпадает с растеризатором с нуля, тот же размер — ничего не меняет, сетка пересчитывается, кадр после смены DPI равен эталону), 1 на отсутствие курсора в библиотеке, 1 «крупнее шрифт → меньше ячеек в том же окне» |
| `tests/stage6_events.rs` | `FixedGrid` на настоящем окне: уведомление о бэкенде приходит один раз до первого кадра, запрошенная сетка применена, `on_grid_resized` пришёл со старым размером, содержимое пережило смену сетки, `on_tick` будит цикл, `ctx.exit()` останавливает его; плюс границы `glyph_px` при DPI 0.1…100 и при `NaN`/`0`/`-2` |
| `tests/stage6_events_auto.rs` | `AutoGrid` на настоящем окне: сетка выведена из окна, запрос приложения не действует (FR-23) |
| `tests/stage6_fallback.rs` | Откат на лету: `Request::backend = Software` на втором тике → уведомления `["OpenGL 3.3", "Software"]`, сетка приложения пережила смену, таймер мигания продолжал тикать |
| `tests/events_common/mod.rs` | Общая часть трёх бинарников: `Probe: AppHandler`, журнал наблюдений, один запуск окна с заданным режимом сетки |

Три последних бинарника объявлены в `Cargo.toml` как `[[test]] harness = false`: цикл событий `winit` обязан быть создан на главном потоке, а обычный harness запускает каждый тест в отдельном потоке; кроме того, `winit` разрешает только один цикл событий на процесс, поэтому каждый сценарий — отдельный бинарник.

### Изменённые тесты

| Файл | Изменение |
|---|---|
| `tests/golden/soft_golden_1x.bin` → `soft_golden_16px.bin` | переименован: сцена называется размером растеризации, а не масштабом (ADR-0005) |
| `tests/golden/soft_golden_32px.bin` | новый эталон: вторая сцена шрифта 32 px вместо «scale 2» |
| `tests/stage3_render.rs` | 20 тестов: `cell_size_is_the_rasterization_metric_and_nothing_is_scaled`, `view_change_forces_full_redraw` переименован в «смена вида», добавлен `big_font_frame_matches_golden_dump` |
| `tests/stage4_gl.rs` | 4-я сцена `dpi 16px → 32px` идёт сразу за сценой 16 px, чтобы оба бэкенда сменили размер **на месте** (`set_size`) — тот путь, который идёт при переносе окна между мониторами |
| `tests/stage2_font.rs` | 33 теста: 7 новых на формулу размера, `new_at`, `set_rasterizer`/`set_size` (переиспользование страницы, `resets`) |
| `tests/stage1_buffer.rs` | `blink_counter::counter_follows_writes_scrolls_resize_and_clear` — счётчик мигающих ячеек для таймера |

## Прогон

```
cargo fmt --check                              → чисто
cargo clippy --all-targets -- -D warnings      → чисто
cargo test                                     → 85 тестов harness'а:
    stage1_buffer 9, stage2_font 33, stage3_render 20, stage4_gl 1,
    stage5_config 7, stage6_app 9, stage6_blink 6
  + stage6_events, stage6_events_auto, stage6_fallback (harness = false)

cargo test --test stage4_gl -- --nocapture
  OpenGL 3.3:    GL ↔ Software identical, 4 scenes
  OpenGL ES 3.0: GL ↔ Software identical, 4 scenes
  (контекст: AMD Radeon RX 580 2048SP, radeonsi; OpenGL 4.6 core / OpenGL ES 3.2, max texture 16384)

cargo test --test stage6_events
  размер глифа следует DPI и остаётся в границах: ok
  gterm: dpi 1.75 → glyph_px 28 (было 16)
  gterm: backend OpenGL 3.3 — AMD / … / GL_MAX_TEXTURE_SIZE 16384, glyph_px 28
  FixedGrid: бэкенд OpenGL 3.3, 3 тиков мигания, сетка Grid { cols: 40, rows: 12 }
  FixedGrid: запрошенная сетка применена: ok

cargo test --test stage6_events_auto
  AutoGrid: сетка из окна Grid { cols: 24, rows: 5 }, бэкенд OpenGL 3.3

cargo test --test stage6_fallback
  откат на лету: ["OpenGL 3.3", "Software"], 4 тиков мигания
  откат на Software на лету без перезапуска: ok
```

## Ручные сценарии демо

```
demo --debug --backend=gl     → dpi 1.75 → glyph_px 28; backend OpenGL 3.3 — AMD …; окно создано скрытым
demo --backend=gles           → backend OpenGL ES 3.0 — … OpenGL ES 3.2 Mesa 26.2.3
demo --backend=soft           → backend Software
TERM_BACKEND=soft demo        → backend Software
demo --fail-gl                → "OpenGL 3.3 unavailable: … forced failure (--fail-gl)",
                                "OpenGL ES 3.0 unavailable: …", "backend Software"
demo --backend=gl --fail-gl   → цепочка от GL 3.3, "backend Software"
demo --backend=vulkan         → "unknown backend 'vulkan' (expected auto|gl|gles|soft)" + usage, код 1
demo --frob                   → "unknown argument '--frob'" + usage, код 1
demo --help                   → usage библиотеки + "--no-blink выключить мигание BLINK (замер простоя, §11.4 п.6)"
demo --no-blink               → работает, мигания нет
```

## Замеры

Окно 80×25 при `glyph_px = 28` (ячейка 17×36) на экране 2194×1234, период мигания 500 мс, окно измерения 10 с после прогрева 4 с (`/proc/<pid>/stat`, `utime+stime`).

| Сценарий | CPU |
|---|---|
| GL 3.3, мигание выключено (`--no-blink`) | **0,00 %** |
| Software, мигание выключено | **0,00 %** |
| Software через `--fail-gl`, мигание выключено | **0,00 %** |
| GL 3.3, мигание включено (2 полных кадра в секунду) | 0,20 % |
| Software, мигание включено | 2,60 % |
| Software через `--fail-gl`, мигание включено | 2,10 % |

Software дороже не из-за таймера, а из-за полной перерисовки кадра при смене фазы: `view.blink_phase` входит в ключ вида, поэтому любой её смены достаточно для полного прохода по 2,2 Мпикс (≈ 13 мс на кадр, ≈ 6 нс/пиксель). Требование §11.4 п.6 сформулировано «в простое (без мигания)» — оно выполнено с запасом.

Память, GL 3.3, мигание включено, 12 замеров по 10 с:

```
RSS kB: 142588 142592 142592 142592 142592 142592 142592 142592 142592 142592 142592 142592
рост за 2 минуты: 4 kB
```

Размер release-бинарника (профиль §10.2 через `--config`, `RUSTFLAGS=-C target-cpu=x86-64`, Linux x86-64):

| Сборка | Байт |
|---|---|
| этап 6, `opt-level = "z"` | 2 323 352 (2,21 МиБ) |
| тот же HEAD (до этапа 6) | 2 298 120 (2,19 МиБ) |
| этап 6, `opt-level = "s"` | 2 499 144 (2,38 МиБ) |

`cargo bloat` не установлен; распределение по крейтам снято через `nm --print-size` по нестрипнутой сборке: `winit` 228 КБ, `skrifa` 192 КБ, `core` 160 КБ, `wayland_client` 142 КБ, `zeno` 115 КБ, `read_fonts` 110 КБ, `std` 102 КБ, `gterm` 90 КБ, `x11_dl` 77 КБ, `gimli` 34 КБ, `glutin` 27 КБ, `glow` 17 КБ. Превышение лимита NFR-1 для Linux существовало до этапа 6 — см. ⚠ WARNINGS.

## Ограничения

- Видимость окна и его содержимое в этом окружении проверить нельзя: `xdotool` не видит ни одного клиентского окна (проверено на постороннем `xmessage`), а `ffmpeg -f x11grab` даёт полностью чёрный root. Поэтому визуальная проверка сцены, смены DPI между мониторами, полного экрана и ввода остаётся владельцу (§11.3); программно проверены только stderr, код возврата и содержимое буферов/кадров.
- Смена `ScaleFactorChanged` вживую не воспроизводилась (один монитор): покрыта тестом пути `set_size` (смена DPI на месте) и проверкой, что GL и Software совпадают после неё.
- Время перерисовки при смене DPI не измерено: нужен второй монитор либо инструмент, задающий `scale_factor`.
