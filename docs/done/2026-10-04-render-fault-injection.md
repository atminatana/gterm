# Выполнение: инжекция ошибок рендера и автотест политики §5.3 п.5 (этап 8)

- План: `docs/plans/2026-10-04-render-fault-injection.md`; ТЗ §5.3 п.5, FR-19, FR-20, FR-21, §11.2; ПОЧЕМУ — `docs/adr/0007-render-fault-injection-flags.md`.

## WHAT

| Файл | Изменение |
|---|---|
| `src/config.rs` | поля `Config::fail_frames: u32`, `Config::lose_context: bool`; разбор `--fail-frames=N` / `--fail-frames N` и `--lose-context[=bool]`; `ConfigError::BadNumber`; справка; общий помощник `take_value` (им же пользуется `--backend`) |
| `src/app.rs` | поля `frames_to_fail`, `is_context_loss_pending`; `take_injected_fault` вызывается в `render_frame` после рендера и вывода кадра, до обновления `FrameStats`; `redraw` и `fall_back` не менялись |
| `tests/stage5_config.rs` | +1 тест `fault_injection_flags`; умолчания и справка дополнены |
| `tests/events_common/mod.rs` | `run_to_end(config, …) -> Option<Finished>` (журнал + результат `App::run`); `run_once` — поверх него, принимает `Config` |
| `tests/stage6_events.rs`, `stage6_events_auto.rs`, `stage6_fallback.rs` | вызов `run_once(Config::default(), …)` — место вызова изменённой сигнатуры |
| `tests/stage8_faults.rs`, `Cargo.toml` | новый оконный тест-бинарник (`harness = false`), 5 сценариев, перезапуск самого себя по разу на сценарий |
| ТЗ 1.5 (FR-19, §11.2), ADR-0007 + индекс, `AGENTS.md`, `TODO.md`, `PROJECT_MAP.md` | обновлены |

## COMPLETENESS

| Требование | Код | Тест |
|---|---|---|
| §5.3 п.5: ошибка кадра → один повтор | `App::redraw` (не менялся) + инжекция | сценарий `retry`: бэкенд один, stderr `retrying once` ровно 1 раз, без `switching to Software` |
| §5.3 п.5: ошибка и после повтора → откат | то же | сценарий `fallback`: `[OpenGL 3.3, Software]`, результат `Ok`, сетка сохранена, тики и кадры идут |
| §5.3 п.5 / FR-21: `ContextLost` → откат сразу | то же | сценарий `context-lost`: откат без `retrying once` |
| §5.3 п.5: ошибка на Software → завершение с ошибкой | `fall_back` → `stop_with` | сценарии `software-stops`, `chain-end`: `App::run` → `AppError::Render`, 0 кадров в статистике; код возврата демо — ручной запуск, см. `docs/test/` |
| FR-20: причина в stderr | `eprintln!` в `redraw` / `fall_back` | родительский процесс сверяет подстроки stderr каждого сценария |
| FR-19: флаги, ошибки разбора | `Config::parse` | `fault_injection_flags`, `usage_mentions_every_option`, `defaults_are_auto_with_gl_allowed` |
| NFR-6: нет аллокаций в горячем пути | `take_injected_fault`: два сравнения; строка — только при инжекции | по чтению кода |

## CORRECTNESS

- Инжектированная ошибка идёт тем же путём `redraw`, что и настоящая: отдельной ветки нет, политика не менялась.
- Тест зелёный по существу: без инжекции в stderr не было бы `retrying once` / `injected by --fail-frames`, а сценарии остановки получили бы `Ok`.
- Три оконных теста этапа 6 проходят без изменения проверок.

## EVIDENCE

См. `docs/test/2026-10-04-render-fault-injection.md`, `docs/verify/2026-10-04-render-fault-injection.md`.

## ⚠ WARNINGS

- **Настоящая потеря контекста не воспроизведена** — инжекция имитирует ошибку после успешного кадра; RDP / смена драйвера остаются ручной проверкой этапа 8.
- **Сценарии «на GL» проверены только на этой машине** (Windows 10, NVIDIA GTX 1060, GL 3.3). Откат с GLES 3.0 флагами не прогонялся; на машине без GL сценарии `fallback`, `context-lost`, `chain-end` пропускаются с причиной.
- **Содержимое кадра после повтора и отката не читается** оконным тестом — проверяются бэкенды, счётчики, результат `run`, stderr.
- Сообщение демо при остановке дублирует префикс: `gterm: render failed: render failed: injected by --fail-frames` (`AppError::Render` и `RenderError::Other` оба добавляют его). Унаследовано, вне рамок — предлагается отдельной правкой.
- `src/app.rs` — 1106 строк (до задачи 1081): превышение порога QG-06 унаследовано, файл не делился; предлагается отдельная задача.
- Release (Windows x86-64, профиль §10.2): `demo.exe` 1 080 832 Б (+2 560 к этапу 7), `full_table.exe` 1 070 592 Б (+2 048); в прирост входят и правки псевдографики от 2026-10-04.
