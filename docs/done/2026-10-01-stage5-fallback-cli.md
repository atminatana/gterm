# Выполнение: этап 5 — цепочка fallback, потеря контекста, CLI/ENV

- План: `docs/plans/2026-10-01-stage5-fallback-cli.md`; ТЗ FR-17…FR-21, §5.1, §5.3 п.5, §9 п.6, §11.2, §11.4 п.4. Новых ADR нет; уточнения (принудительный уровень = стартовый, приоритет источников, признаки потери контекста, `--debug` вместо feature) вписаны в ТЗ как уточнения существующих требований.

## WHAT

| Файл | Изменение |
|---|---|
| `src/config.rs` | новый: `BackendKind {Auto, Gl, Gles, Soft}` + `chain()`, `Config {backend, fail_gl, allow_sw_gl, debug, help}`, `Config::parse(args, env)` / `from_process()`, типизированный `ConfigError`, `usage()` |
| `src/renderer/mod.rs` | `RendererKind {OpenGl33, Gles30, Software}` с `name()`/`is_gl()` (§5.1) |
| `src/renderer/gl_backend.rs` | `GlContext::new(…, kind, &GlOptions)` — один уровень на попытку; `--fail-gl`, `--allow-sw-gl` (`llvmpipe`/`softpipe`), `GDI Generic` всегда отказ; `GlResources` с `Drop` отдельно от бэкенда → `GlBackend::into_atlas` для отката; `GL_CONTEXT_LOST` после кадра и ошибка `swap_buffers` → `RenderError::ContextLost` |
| `src/app.rs` | цепочка уровней из `Config`: окно+конфиг GL один раз, `try_gl_level` на каждый GL-уровень, Software на существующем или новом окне; `redraw` с §5.3 п.5 (повтор один раз → откат); `fall_back` — Software на том же окне без перезапуска (FR-21); ошибка на Software → выход с кодом ≠ 0; `--debug` → `[имя бэкенда]` в заголовке |
| `src/bin/demo.rs` | `Config::from_process()`, `--help`, ошибки разбора → справка и `FAILURE` |
| `tests/stage5_config.rs` | 7 тестов разбора/приоритетов/ошибок/цепочек/справки |
| `tests/stage4_gl.rs` | проверяет **оба** GL-уровня в одном цикле событий |
| ТЗ (FR-19, FR-21, §5.3, §10.3, §11.2, §12), `AGENTS.md`, `PROJECT_MAP.md` | обновлены |

## COMPLETENESS

| Требование | Код | Тест |
|---|---|---|
| FR-17 порядок уровней | `BackendKind::chain` | `chains_start_at_requested_level_and_end_in_software` |
| FR-18 откат при любой ошибке инициализации | `create_window_and_backend` перебирает уровни, логирует причину | `demo --fail-gl`: оба GL-уровня отвергнуты, `backend Software` |
| FR-19 `--backend`, `TERM_BACKEND`, приоритет, ошибки | `Config::parse` | 5 тестов; ручные: `--backend=soft`, `TERM_BACKEND=soft`, `--backend=gles` → GLES 3.0, `--backend=vulkan` → справка, `--frob` → код 1 |
| FR-20 имя и причина в stderr; `--debug` в заголовке | `eprintln!` на каждом уровне; `apply_title` | ручной запуск `--debug` |
| FR-21 потеря контекста → Software без выхода | `draw`/`swap_buffers` → `ContextLost`; `fall_back` | **не воспроизведена** (см. ⚠); путь кода компилируется и переиспользует `software_on` из основного пути |
| §5.3 п.5 повтор кадра, затем откат | `redraw` | **без автотеста** (нужна инъекция ошибки рендера) |
| §9 п.6 `--allow-sw-gl`, `GDI Generic` | `query_info` | ручной прогон с `llvmpipe` недоступен на этой машине |
| §11.4 п.4 `--fail-gl` | `GlContext::new` | ручной запуск, см. EVIDENCE |
| §11.2 бит-идентичность на GLES | `stage4_gl` | **OpenGL ES 3.0: identical, 3 scenes** |

## CORRECTNESS

- Цепочка проверена на всех трёх выходах: GL 3.3 (auto), GLES 3.0 (`--backend=gles`), Software (`--backend=soft`, `TERM_BACKEND=soft`, `--fail-gl`).
- GLES-путь теперь не только создаётся, но и даёт тот же кадр, что Software (0 отличий) — закрыт `[data needed]` этапа 4.
- Разбор аргументов: типизированные ошибки с источником; неизвестное значение из окружения называет `TERM_BACKEND`.

## EVIDENCE

См. `docs/test/2026-10-01-stage5-fallback-cli.md`, `docs/verify/2026-10-01-stage5-fallback-cli.md`.

## ⚠ WARNINGS

- **Реальная потеря контекста не воспроизведена** (нет RDP-сессии/смены драйвера под рукой); обработчик написан по признакам `GL_CONTEXT_LOST`/ошибки swap. `[data needed: RDP-сессия или ВМ без 3D — §11.3]`.
- **Повтор кадра по §5.3 п.5 без автотеста** — ошибку рендера нечем инжектировать без тестового бэкенда; предлагается на этапе 8 добавить `Renderer`-заглушку с программируемыми ошибками.
- Обнаружение потери контекста через `glGetError` каждый кадр — один вызов на кадр, дёшево; `GL_CONTEXT_LOST` возвращается только драйверами с `KHR_robustness`/GL 4.5 — на старых драйверах потеря проявится ошибкой `swap_buffers`, которая тоже ведёт к откату.
- Окно, созданное под GL-конфиг, при откате используется `softbuffer` (риск §14 п.1): на Windows проверено `--fail-gl` (кадр виден); Linux/X11 — не проверено.
- `--allow-sw-gl=false` не проверен на `llvmpipe` (нет Mesa на машине).
- `cargo bloat` после этапа: `find_folder`/`config` незаметны; release **1 051 136 Б** (профиль §10.2 через `--config`).
