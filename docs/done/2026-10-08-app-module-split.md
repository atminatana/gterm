# Выполнение: разбиение `src/app.rs` на модули

- План: `docs/plans/2026-10-08-app-module-split.md`; ТЗ 1.9 (§5.1); ПОЧЕМУ — `docs/adr/0011-app-module-split.md`.

## WHAT

| Файл | Содержимое | Строк |
|---|---|---|
| `src/app/mod.rs` | документация модуля, объявления модулей, реэкспорт 11 публичных имён, `App`, `new`, `run`, геттеры | 151 |
| `src/app/api.rs` | `PointerEvent`, `Request`, `FrameStats`, `Ctx`, `AppHandler` | 120 |
| `src/app/options.rs` | `AppError` (+ `Display`, `Error`, `From`), `Grid`, `GridMode`, `AppOptions`, `auto_grid`, `centered_origin` | 128 |
| `src/app/backend.rs` | `SoftState` + `present`, `Backend` | 117 |
| `src/app/chain.rs` | `gl_options`, `create_window_and_backend`, `build_gl_window`, `try_gl_level`, `software_on`, `fall_back`, `switch_backend`, `restore_backend`, `after_switch` | 337 |
| `src/app/window.rs` | `cell_px`, `window_attributes`, `fit_rasterizer_to_window`, `fit_rasterizer`, `apply_title`, `resize_window`, `apply_scale_factor` | 167 |
| `src/app/frame.rs` | `render_frame`, `take_injected_fault`, `redraw`, `request_redraw`, `stop_with`, `schedule_wakeup` | 110 |
| `src/app/events.rs` | `notify`, `apply_request`, `impl ApplicationHandler for App` | 153 |
| `src/app.rs` | удалён | −1173 |
| ТЗ 1.9 §5.1 и §14, ADR-0011 + индекс, план, `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md` | обновлены | — |

Механика: скрипт резал `app.rs` по границам элементов (с doc-комментариями и атрибутами) и раскладывал по файлам; методы `App` — в блоки `impl App` дочерних модулей; `pub(super)` — на методах, перенесённых из `impl App`, и на `Backend`, `SoftState`, их полях и методах; `use` — по ошибкам и предупреждениям компилятора; шесть intra-doc ссылок — с явными путями.

## COMPLETENESS

| Требование (ADR-0011 / план) | Подтверждение |
|---|---|
| Восемь модулей по обязанностям, каждый ниже порога QG-06 | таблица выше; самый крупный `chain.rs` — 337 |
| Внешний API `gterm::app::…` не изменился | реэкспорт в `mod.rs`; ни один файл вне `src/app/` не менялся (`git status`); бинарники и тесты собираются без правок |
| Только перемещение кода | сравнение строк до/после (без `use`, `pub(super)`, обвязки модулей): 906 строк кода совпали, отличаются только 4 сигнатуры, которые `rustfmt` перенёс на несколько строк из-за длины `pub(super)` |
| Поведение не изменилось | те же тесты, что до переноса: 131 + 4 оконных бинарника; GL ↔ Software — 6 сцен на обоих уровнях |

## CORRECTNESS

Тесты не менялись и не добавлялись — проверяется ровно то же, что до переноса. Цепочку откатов исполняют `stage6_fallback` и `stage8_faults` (5 сценариев), события, сетку и `Request` — `stage6_events`/`stage6_events_auto`, геометрию — `stage6_app`. Не исполняется автотестами `apply_scale_factor` (смена DPI окна) — проверка глазами у владельца.

## EVIDENCE

- До переноса (HEAD `2d11e64`): `cargo test` — 131 тест + 4 оконных бинарника, гейты чистые.
- После: `cargo fmt --check` ✓; `cargo build` — 0 предупреждений; `cargo clippy --all-targets -- -D warnings` ✓; `cargo test` — 131 тест, 9 строк `: ok` оконных сценариев, 0 `FAILED`; `stage4_gl`: `GL ↔ Software identical, 6 scenes` на OpenGL 3.3 и OpenGL ES 3.0.
- `cargo doc --no-deps`: неразрешённых ссылок в `src/app/` нет; остаются 4 в `src/cell.rs:147-150` (`[0]`…`[3]`, были до задачи).
- Временные скрипты и копия `app.rs` — в `target/tmp/` (не в репозитории).

## ⚠ WARNINGS

- `apply_scale_factor` (перенос окна между мониторами с разным DPI) после переноса не исполнялся — пункт в `TODO.md` для владельца.
- Вне задачи, не менялось: на тике мигания `App` перерисовывает кадр только по запросу обработчика (`app/events.rs`, `about_to_wait`); 4 неразрешённые doc-ссылки в `src/cell.rs` — оба пункта в `TODO.md`.
- Изменения не закоммичены — коммит по запросу владельца; по CQ-02.1 перенос должен идти отдельным коммитом без иных изменений.
