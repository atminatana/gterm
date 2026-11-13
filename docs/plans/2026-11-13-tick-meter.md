# План: периодический тик цикла событий и индикатор уровня `Meter`

Статус: **выполнен 2026-11-13** — см. `docs/done/2026-11-13-tick-meter.md`. Отклонений от плана нет.

- **ТЗ задачи:** `docs/spec/2026-11-13-tick-meter.md` (утверждено 2026-11-13; О1–О4 — варианты (a), О5 — половина ячейки, О6 — два ADR).
- **Решения:** ADR-0033 (тик; частично заменяет ADR-0012), ADR-0034 (`Meter`).
- **Опирается на:** ADR-0004, ADR-0006 (контракт приложения), ADR-0012 (что убрано и не возвращается), ADR-0023, ADR-0032 (виджеты-значения).

## Факты по коду (проверено 2026-11-13, коммит `c5b54c8`)

- `app/mod.rs:120` — `event_loop.set_control_flow(ControlFlow::Wait)`, единственное место; `app/events.rs:11-107` — `impl ApplicationHandler for App` с `resumed` и `window_event`; `notify`/`apply_request` — `events.rs:112-177`.
- `app/options.rs:79-91` — `AppOptions { grid_mode, resizable, vsync, font_size_px }`, `Copy`; все литералы в репозитории заканчиваются `..AppOptions::default()` (примеры `hotkeys`, `pseudographica`, `widgets`, `widget_edit`, `gterm-demo/src/common.rs`, `bin/demo/bench.rs`, `tests/events_common/mod.rs`).
- `app/api.rs:46-64` — `Request` (`Default`), `api.rs:135-164` — `AppHandler`.
- `tests/events_common/mod.rs` — `Probe`, `run_to_end` (шрифт репозитория, сторож 60 с); бинарники подключают модуль целиком, неиспользуемый в каком-либо бинарнике код дал бы предупреждение `dead_code` при `clippy --all-targets`.
- `tests/stage8_faults.rs` — образец «бинарник запускает сам себя по сценарию» (`--scenario=`).
- `widgets/gauge.rs` — `clamp_ratio`, `render_block` (приватные), `Gauge`/`LineGauge`; `box_drawing.rs:326-348` — блоки U+2580–U+259F рисуются процедурно, включая `▌` (U+258C) и `░` (U+2591).
- Базовый прогон: 307 тестов в 45 наборах и 4 оконных бинарника, 0 падений; `fmt` и `clippy --all-targets` чисты.

## Проектные решения

1. **Тик (ADR-0033).** Состояние — `app/tick.rs`: период и срок следующего тика. `ApplicationHandler::about_to_wait`: если окно создано, период задан и срок наступил — один `on_tick` через `notify` → `apply_request`, затем срок = «сейчас + период»; после этого режим ожидания — `WaitUntil(срок)` либо `Wait`. Проверка в `about_to_wait`, а не по причине пробуждения: тик приходит и тогда, когда цикл занят вводом или кадрами. Период < 1 мс → 1 мс. `Request::tick_period` применяется в `apply_request`: новый отсчёт от момента применения.
2. **Оконный тест** `tests/tick_events.rs` (`harness = false`), сценарии дочерними процессами. Общий запуск окна с произвольным обработчиком выносится в `tests/events_common/launch.rs` и используется и `run_to_end`, и новым бинарником (подключает только этот файл) — так ни в одном бинарнике не появляется неиспользуемого кода.
3. **`Meter` (ADR-0034).** `widgets/meter.rs`, виджет-значение; общие с `Gauge` помощники (`clamp_ratio`, `render_block`) становятся `pub(super)`.

## Шаги

1. **Тик.** `app/tick.rs`, `app/mod.rs`, `app/events.rs`, `app/options.rs`, `app/api.rs`; `tests/events_common/launch.rs`, `tests/events_common/mod.rs`, `tests/tick_events.rs`, `crates/gterm/Cargo.toml` (К1–К5). Проверка: `cargo test -p gterm`.
2. **`Meter`.** `widgets/meter.rs`, `widgets/mod.rs`, `widgets/gauge.rs` (видимость помощников), `tests/meter.rs` (К6–К8). Проверка: `cargo test -p gterm-widgets`.
3. **Пример.** Вкладка индикаторов в `examples/widgets.rs` (К9); запуск и снимок окна `tools/shot.ps1`.
4. **Документы и гейты.** ADR-0033, ADR-0034 + индекс; ТЗ проекта (FR-26, §13, версия); `AGENTS.md`, `PROJECT_MAP.md`, `README.md`, `README.ru.md`, `TODO.md`; отчёты `docs/done|test|verify/`; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, полный `cargo test`; размеры release.

## Открытые решения

Нет.

## Риски

- Оконный тест зависит от машины: интервалы проверяются только снизу (не короче периода — это гарантирует сама схема), общее время — с большим запасом под сторожем.
- Задержка обработчика в сценарии «отставание» имитируется `thread::sleep` внутри `on_tick` — это предмет проверки, а не синхронизация теста.
