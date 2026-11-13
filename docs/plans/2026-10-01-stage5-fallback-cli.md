# План: этап 5 — цепочка fallback, потеря контекста, CLI/ENV

- **ТЗ:** FR-17…FR-21, §5.1 (`config.rs`, `RendererKind`), §5.3 п.3–5, §9 п.6, §11.2 (fallback-тест), §11.4 п.4 (`--fail-gl`).
- **Рамки:** `config.rs` (разбор аргументов и окружения), `renderer/mod.rs` (`RendererKind`), `gl_backend.rs` (один API на попытку, `--fail-gl`, `--allow-sw-gl`, обнаружение потери контекста, ресурсы GL без `Drop` на всём бэкенде — чтобы атлас можно было забрать при откате), `app.rs` (цепочка уровней, откат во время работы, повтор кадра), демо (конфигурация, `--help`), тесты разбора конфигурации, ручная проверка `--fail-gl` и `--backend=soft`.
- **Не входит:** FPS и клавиши демо (этап 7), DPI/масштаб/полный экран (этап 6).

## Открытые решения

Нет. Уточнения, принятые как локальные (BP-02.3), вписываются в ТЗ:
- Принудительный `--backend=X` — **стартовый уровень цепочки**, не единственный: §11.2 требует, чтобы `--backend=gl` без GL завершался переходом на `soft`, а не падением. `gl` → GL 3.3 → Software; `gles` → GLES 3.0 → Software; `soft` → только Software; `auto` → GL 3.3 → GLES 3.0 → Software.
- Приоритет источников: аргументы командной строки > `TERM_BACKEND` > `auto`. Неизвестный аргумент или значение → сообщение, подсказка `--help`, код ≠ 0.
- `--fail-gl` — отказ при создании **любого** GL-контекста (имитация ошибки инициализации); `--allow-sw-gl=false` — `llvmpipe`/`softpipe` считаются отказом; `GDI Generic` — отказ всегда.
- Потеря контекста: `glGetError() == GL_CONTEXT_LOST` (0x0507) после кадра либо ошибка `swap_buffers` → `RenderError::ContextLost` → Software на том же окне без перезапуска. Иная ошибка `render` → один повтор, затем откат на следующий уровень (§5.3 п.5).
- `--debug` — имя бэкенда в заголовке окна (FR-20); feature `debug-title` из §10.3 заменяется флагом времени выполнения (не влияет на размер: строка форматирования).

## Шаги

| # | Шаг | Проверка |
|---|---|---|
| 1 | `src/config.rs`: `BackendKind`, `Config { backend, fail_gl, allow_sw_gl, debug }`, `Config::parse(args, env)`, `usage()`; `renderer/mod.rs`: `RendererKind { OpenGl33, Gles30, Software }`, `BackendKind::chain()` | `tests/stage5_config.rs`: умолчания, CLI, ENV, приоритет, ошибки |
| 2 | `gl_backend.rs`: `GlContext::new(config, window, w, h, api, options)`; `GlResources` с `Drop`; `GlBackend::into_atlas`; `CONTEXT_LOST`; проверка `llvmpipe`/`softpipe` | сборка; GL-тест этапа 4 по-прежнему зелёный |
| 3 | `app.rs`: цепочка по `Config`, `try_level`, откат во время работы (`fall_back`), повтор кадра, `--debug` в заголовке | `demo --fail-gl` → stderr «falling back to Software», окно работает; `--backend=soft`; `TERM_BACKEND=soft`; `--backend=x` → код ≠ 0 |
| 4 | `demo.rs`: `Config::parse`, `--help`, вывод в stderr | запуск |
| 5 | ТЗ FR-19/FR-21/§9 п.6/§10.3 уточнения; `AGENTS.md` (ловушки про `--fail-gl` закрыть), `PROJECT_MAP.md` | grep |
| 6 | Гейты; отчёты | `docs/done|test|verify` |
