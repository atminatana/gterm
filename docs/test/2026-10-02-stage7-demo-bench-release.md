# Тесты: этап 7 — демо FR-27, замеры `--bench`, профиль release

## Состав

### Изменённые тесты

| Файл | Изменение |
|---|---|
| `tests/stage6_blink.rs` | 7 тестов (+1): `catch_up_on_an_odd_number_of_phases_keeps_the_parity` — шесть дедлайнов подряд (`catch_up` → `visible_at`/`visible` → `next_toggle`), затем пропуск трёх фаз разом. Проверяет исправление дефекта этапа 6 |
| `tests/events_common/mod.rs` | `Probe` пишет `ctx.blink_phase` на каждом тике, считает `on_frame` и запоминает `ctx.stats.frames`; выход — когда тиков ≥ 3 **и** обе фазы видели (запас 20 тиков на пропуски таймера ОС); `assert_frame_stats`: обе фазы, `frames ≥ 1`, `frame_events == frames` |
| `tests/stage6_events.rs`, `stage6_events_auto.rs`, `stage6_fallback.rs` | все три сценария вызывают `assert_frame_stats`; откат на лету подтверждает, что счётчик кадров переживает смену бэкенда |

Новых тестовых бинарников нет: статистика кадра, фаза в `Ctx` и `on_frame` проверяются на настоящем окне существующими бинарниками (`harness = false`, один цикл `winit` на процесс).

## Прогон (Windows 10, stable 1.98.1, NVIDIA GTX 1060 / 581.57)

```
cargo fmt --check                              → чисто
cargo clippy --all-targets -- -D warnings      → чисто
cargo test                                     → 86 тестов harness'а:
    stage1_buffer 9, stage2_font 33, stage3_render 20, stage4_gl 1,
    stage5_config 7, stage6_app 9, stage6_blink 7
  + stage6_events, stage6_events_auto, stage6_fallback (harness = false)

cargo test --test stage6_events
  размер глифа следует DPI и остаётся в границах: ok
  gterm: backend OpenGL 3.3 — NVIDIA Corporation / NVIDIA GeForce GTX 1060 3GB … (GL_MAX_TEXTURE_SIZE 32768, glyph_px 16)
  FixedGrid: бэкенд OpenGL 3.3, 3 тиков мигания, 3 кадров, фазы [true, false, true], сетка Grid { cols: 40, rows: 12 }
  FixedGrid: запрошенная сетка применена: ok

cargo test --test stage6_events_auto
  AutoGrid: сетка из окна Grid { cols: 24, rows: 6 }, бэкенд OpenGL 3.3
  AutoGrid: сетка выведена из окна, запрос проигнорирован: ok

cargo test --test stage6_fallback
  откат на лету: ["OpenGL 3.3", "Software"], 4 тиков мигания, 4 кадров
  откат на Software на лету без перезапуска: ok
```

Первый прогон после добавления проверки фаз был **красным**: `за 23 тиков обе фазы не встретились: [true, true, …]` — это и вскрыло дефект `Blinker::catch_up` (лог сохранён отдельно). Временный репро `tests/tmp_blink_repro.rs` подтвердил причину (`catch_up(+501)` → `visible_at(+502) = true`) и удалён; та же проверка закреплена в `stage6_blink`.

## Ручные сценарии (release, stderr перенаправлен в файл)

```
demo --bench                  → см. замеры; код 0
demo --bench --backend=gles   → см. замеры; код 0
demo --bench --backend=soft   → см. замеры; код 0
full_table (8 с до таймаута)  → "глифов в BMP: 3369, страниц по 1920: 2", backend OpenGL 3.3
demo (5 с до таймаута)        → backend OpenGL 3.3, окно живёт
demo --fail-gl (5 с)          → "OpenGL 3.3 unavailable: … forced failure (--fail-gl)", "OpenGL ES 3.0 unavailable: …", "backend Software"
```

## Замеры

### `--bench` (NFR-4, NFR-5)

Условия: Windows 10, NVIDIA GeForce GTX 1060 3GB, драйвер 581.57, 100 % DPI (`glyph_px = 16`, ячейка 10×21), VSync выключен (`AppOptions::vsync = false`), 30 кадров прогрева + 300 измеряемых, каждый кадр перезаписывает все ячейки (все строки dirty).

| Бэкенд | Сетка | `cpu` mean | p50 | p95 | max | `total` mean | FPS |
|---|---|---|---|---|---|---|---|
| OpenGL 3.3 | 200×60 (12 000) | **0,578 мс** | 0,554 | 0,629 | 3,679 | 0,615 мс | 1098,7 |
| OpenGL ES 3.0 | 200×60 (12 000) | **0,568 мс** | 0,560 | 0,623 | 0,759 | 0,606 мс | 1096,5 |
| Software | 120×40 (4 800), окно 1200×840 px | 9,232 мс | 9,049 | 10,779 | 12,353 | 9,969 мс | **97,9** |

- NFR-4 (GL 200×60 ≤ 1 мс CPU): выполнено на обоих GL-уровнях — mean 0,57–0,58 мс, p95 0,62–0,63 мс.
- NFR-5 (Software 120×40 ≥ 60 FPS при полной перерисовке): выполнено — 97,9 FPS (≈ 9,9 мс на кадр, ≈ 9 нс на пиксель включая копирование в окно).
- Первый прогон **с VSync** дал на GL `cpu` mean 16,148 мс / `total` 16,198 мс / 60,0 FPS: драйвер возвращает из `swap_buffers` сразу и ждёт кадр монитора в первом GL-вызове следующего кадра (см. ADR-0006). Поэтому `--bench` работает без VSync.

### Размер release (NFR-1 — только фиксация, лимита нет)

Профиль §10.2 целиком в `Cargo.toml`, `cargo build --release`, Windows x86-64 MSVC:

| Бинарник | Байт |
|---|---|
| `demo.exe` | 1 078 272 (1,03 МиБ) |
| `full_table.exe` | 1 068 544 (1,02 МиБ) |

Для сравнения: этап 5 (Windows, `--config`) — 1 051 136 Б; этап 6 (Linux, `--config`) — 2 323 352 Б. На Linux с профилем из манифеста не перемерялось.

## Дополнение 2026-10-03: шрифт Nerd Font Mono

Владелец при осмотре `full_table` увидел обрезанные и «слипшиеся» иконки. Временный тест (удалён) по всем глифам Regular через `skrifa::GlyphMetrics` на `glyph_px = 16` (ячейка 10×21):

| Шрифт | Глифов | PUA | PUA за ячейку по x | Макс. выход |
|---|---|---|---|---|
| RobotoMono Nerd Font (было) | 3369 | 2321 | **2000** | 9,5 px |
| RobotoMono Nerd Font Mono v3.5.1 (стало) | 5019 | 3715 | 2 | 0,6 px |

Advance у всех глифов обоих вариантов одинаковый (9,6 px) — шрифт моноширинный, двойной ширины нет; по вертикали за ячейку не выходит ничего. После замены: `cargo fmt --check`, `clippy --all-targets -- -D warnings` чисто; `cargo test` — 86 + 3 оконных, golden 16/32 px без изменений (упали и были исправлены только четыре константы имён файлов в `stage2_font`). `full_table` после замены: 5019 глифов, 3 страницы.

## Ограничения

- Визуальная часть (противофаза курсора, обновление строки статуса, листание `full_table`, блок 16×16) здесь не проверялась — нет человека у окна; подтверждены только stderr, коды возврата и содержимое буферов.
- Замеры — одна конфигурация; на Linux/Mesa и при высоком DPI цифры будут другими. При `glyph_px = 28` сетка 200×60 требует окно 3400×2160 px — на меньшем экране `FixedGrid` обрежет её, и замер Software окажется завышенным (`--bench` печатает логический размер окна).
- Оконный тест фаз зависит от таймера ОС: при пропусках переключений ждёт до 20 запасных тиков; на этой машине пропусков не было (`[true, false, true]`).
