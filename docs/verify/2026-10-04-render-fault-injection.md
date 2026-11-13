# Верификация: инжекция ошибок рендера и политика §5.3 п.5 (этап 8)

| Критерий (план / ТЗ) | Статус | Подтверждение |
|---|---|---|
| `--fail-frames=N`: первые N попыток кадра → `RenderError::Other`, счётчик общий на запуск | ✅ | `App::take_injected_fault` (`src/app.rs`); сценарий `chain-end`: 4 ошибки распределились GL ×2 → Software ×2 |
| `--lose-context`: первая попытка кадра → `ContextLost`, один раз | ✅ | `take_injected_fault`; сценарий `context-lost`: один откат, дальше кадры идут |
| Инжекция после рендера и вывода, до `FrameStats`; `on_frame` за ошибочный кадр не приходит | ✅ | вызов между `match backend` и `self.stats = …` в `render_frame`; сценарии остановки: `frames == 0`, `frame_events == 0` |
| §5.3 п.5: иная ошибка → повтор один раз | ✅ | сценарий `retry`; `retrying once` ровно 1 раз |
| §5.3 п.5: затем откат | ✅ | сценарий `fallback`: `[OpenGL 3.3, Software]`, `Ok` |
| §5.3 п.5 / FR-21: `ContextLost` → откат без повтора, без перезапуска | ✅ (инжекция) | сценарий `context-lost`; настоящая потеря контекста — ⚠ |
| §5.3 п.5: ошибка на Software → завершение с сообщением и кодом ≠ 0 | ✅ | сценарии `software-stops`, `chain-end` (`AppError::Render`); `demo --backend=soft --fail-frames=2` → код 1 |
| FR-19: флаги в перечне, ошибки разбора типизированы | ✅ | ТЗ FR-19; `Config::parse`, `ConfigError::BadNumber`; тест `fault_injection_flags` |
| FR-20: причина в stderr | ✅ | родитель `stage8_faults` сверяет подстроки |
| §11.2: тест ошибок кадра описан | ✅ | ТЗ §11.2, новый пункт |
| Политика `redraw` / `fall_back` не менялась | ✅ | `git diff src/app.rs`: только поля, инициализация, вызов и `take_injected_fault` |
| NFR-6 | ✅ (чтение) | два сравнения на кадр; `String` создаётся только при инжекции |
| NFR-7 | ✅ | инжекция возвращает `Result`; новых `unwrap`/`expect` в `src/` нет |
| NFR-9 | ✅ | `gl_backend.rs` не менялся |
| NFR-1: размер записан | ✅ | `demo.exe` 1 080 832 Б, `full_table.exe` 1 070 592 Б |
| §4.5 гейты | ✅ | fmt / clippy `--all-targets` / 96 тестов + 4 оконных бинарника |
| TL-05 | ✅ | `2026-10-04-render-fault-injection.md` в `docs/plans`, `done`, `test`, `verify`; ADR-0007 в индексе |
| Переоткрытые места | — | `render_frame` и `take_injected_fault` перечитаны; вывод `cargo test`, `demo` и stderr всех пяти сценариев прочитан |

Расхождений с ТЗ и планом не найдено. Дефектов политики §5.3 п.5 тест не выявил. Непроверенное — в ⚠ WARNINGS `docs/done/`.
