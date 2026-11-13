# План: разбиение `src/app.rs` на модули

Статус: **реализовано 2026-10-08** — см. `docs/done/2026-10-08-app-module-split.md` (вариант A; `apply_scale_factor` — проверка глазами у владельца). Решение — ADR-0011. Отклонение: шесть intra-doc ссылок после переноса получили явные пути (`super::…`, `crate::blink::…`) — текст документации не менялся.

- **Цель:** `src/app.rs` (1173 строки, порог QG-06 — 500) → каталог `src/app/` из восьми файлов по обязанностям; поведение и внешний API не меняются.
- **ТЗ:** §5.1 (структура проекта — строка `app.rs` в дереве файлов).
- **Рамки:** `src/app.rs` → `src/app/{mod,api,options,backend,chain,window,frame,events}.rs`; ТЗ §5.1; ADR-0011 + индекс; `PROJECT_MAP.md`, `AGENTS.md` (упоминания `app.rs`); отчёты `docs/done|test|verify`.
- **Не входит:** любые изменения логики, сигнатур, текстов сообщений; новые тесты (кроме проверки глазами `apply_scale_factor`); замечание про `on_tick` без перерисовки (отдельная задача).

## Раскладка

| Файл | Содержимое (строки `app.rs` до переноса) |
|---|---|
| `mod.rs` | документация модуля, `App` (346–388), `new`, `run`, `backend_name`, `backend_kind`, `glyph_px`, `grid` (409–481), реэкспорты |
| `api.rs` | `PointerEvent`, `Request`, `FrameStats`, `Ctx`, `AppHandler` (132–240) |
| `options.rs` | `AppError` (33–75), `Grid`, `GridMode`, `AppOptions` (76–131), `auto_grid`, `centered_origin` (390–406) |
| `backend.rs` | `SoftState` + `present`, `Backend` (241–345) |
| `chain.rs` | `gl_options`, `create_window_and_backend`, `build_gl_window`, `try_gl_level`, `software_on`, `fall_back`, `switch_backend`, `restore_backend`, `after_switch` |
| `window.rs` | `cell_px`, `window_attributes`, `fit_rasterizer_to_window`, `fit_rasterizer`, `apply_title`, `resize_window`, `apply_scale_factor` |
| `frame.rs` | `render_frame`, `take_injected_fault`, `redraw`, `request_redraw`, `stop_with`, `schedule_wakeup` |
| `events.rs` | `notify`, `apply_request`, `impl ApplicationHandler for App` |

## Шаги

| # | Шаг | Проверка |
|---|---|---|
| 1 | ADR-0011 + индекс; этот план | — |
| 2 | Базовый прогон: `fmt`, `clippy --all-targets`, `test` (131 тест + 4 оконных бинарника), `stage4_gl` | исходное состояние |
| 3 | Перенос скриптом по диапазонам строк; `pub(super)` на методах и типах, используемых соседями; `use` в каждом файле — по ошибкам и предупреждениям компилятора; реэкспорт в `mod.rs` | `git diff --stat`: удалённых ≈ добавленных; тела функций совпадают побайтно (сравнение скриптом) |
| 4 | Тот же прогон, что в шаге 2 | те же числа тестов, всё зелёное |
| 5 | ТЗ §5.1, `PROJECT_MAP.md`, `AGENTS.md`; отчёты | TL-05 |
| 6 | Владелец: глазами — перенос окна между мониторами с разным DPI (`apply_scale_factor`), запуск `demo`/`full_table`, переключение `1/2/3` | §11.3 |
