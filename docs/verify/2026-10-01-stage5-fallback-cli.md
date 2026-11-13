# Верификация: этап 5 — цепочка fallback, потеря контекста, CLI/ENV

| Критерий ТЗ | Статус | Подтверждение |
|---|---|---|
| FR-17 GL 3.3 → GLES 3.0 → Software | ✅ | `BackendKind::Auto.chain()`; `--fail-gl` проходит все три уровня в этом порядке (stderr) |
| FR-18 откат при ошибке конфигурации / контекста / шейдера / текстуры / `glGetError` первого кадра | ✅ | каждый шаг `try_gl_level` → `Err` → следующий уровень; `--fail-gl` имитирует отказ контекста; остальные ветки — по коду (`Result` на каждом шаге) |
| FR-19 `--backend`, `TERM_BACKEND`, `auto` по умолчанию | ✅ | `Config::parse`; 7 тестов; ручные запуски всех значений |
| FR-20 `Renderer::name()`, лог причины, заголовок в `--debug` | ✅ | `RendererKind::name`, `eprintln!` на каждом уровне, `apply_title` |
| FR-21 потеря контекста → Software без завершения | ✅ (код) | `GL_CONTEXT_LOST` после кадра и ошибка `swap_buffers` → `ContextLost` → `fall_back`: атлас из `GlBackend::into_atlas`, `software_on(window)`, `resize`, redraw; воспроизведение — ⚠ |
| §5.3 п.5 повтор кадра один раз, затем откат; на Software — выход | ✅ (код) | `redraw`: `frame_retried`; `fall_back` на `Backend::Soft` → `stop_with` |
| §9 п.6 `GDI Generic` отказ; `llvmpipe`/`softpipe` по `--allow-sw-gl` | ✅ (код) | `query_info`; прогон на `llvmpipe` — ⚠ |
| §11.2 fallback-тест (`--backend=gl` без GL → soft) | ✅ (эквивалент) | `--fail-gl` на машине с GL даёт тот же путь; ТЗ дополнено этим способом проверки |
| §11.2 бит-идентичность на обоих GL-уровнях | ✅ | `stage4_gl`: OpenGL 3.3 и OpenGL ES 3.0 — 0 отличий |
| §11.4 п.4 `--fail-gl` | ✅ | ручной запуск, окно работает на Software |
| §5.1 `config.rs`, `RendererKind` в `renderer/mod.rs` | ✅ | файлы на месте; дублирующие `RendererKind` в бэкендах удалены ранее |
| NFR-7 | ✅ | все отказы — `Result`/лог; единственный `expect` в цепочке — на инвариант «окно есть, если есть GL-конфиг» |
| NFR-9 | ✅ | `unsafe` только в `gl_backend.rs` |
| §4.5 гейты | ✅ | fmt / clippy / 60 тестов |
| Переоткрытые места | — | `config.rs`, `app.rs`, `gl_backend.rs` (изменённые части), оба теста — целиком; stderr всех ручных сценариев прочитан |

Расхождений с ТЗ не найдено. Непроверенные сценарии (реальная потеря контекста, `llvmpipe`, повтор кадра) — в ⚠ WARNINGS `docs/done/`.
