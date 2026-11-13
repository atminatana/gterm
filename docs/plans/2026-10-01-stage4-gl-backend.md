# План: этап 4 — GL-бэкенд (OpenGL 3.3 core / GLES 3.0), один draw call, golden GL ↔ Software

- **ТЗ:** §3.3 (FR-12…FR-16), §6 (данные на GPU), §7 (шейдеры), §9 (инициализация GL), §11.2 (сравнение с допуском 0), NFR-7, NFR-9; ADR-0002 (целочисленное смешивание, `R8UI`→`RGBA8UI`), ADR-0003 (слоты, каналы-начертания, `take_dirty_slots`).
- **Рамки:** `gl_backend.rs` заново (контекст через `glutin` — весь `unsafe` здесь, NFR-9; текстуры; шейдеры из `shaders/`; FBO для чтения кадра в тестах), `app.rs` — выбор бэкенда GL → Software при любой ошибке (ядро FR-18; `--backend`/`TERM_BACKEND`/`--fail-gl` — этап 5), тест бит-идентичности. Feature `gl` **упраздняется** — §10.3 её не предусматривает, GL — часть ядра с откатом в рантайме.
- **Не входит:** потеря контекста во время работы (FR-21, этап 5), CLI/ENV, принудительный отказ GL, отладочный заголовок.

## Открытые решения

Нет. Зафиксированные детали реализации (в духе §6–7):
- Атлас в GL — `RGBA8UI`, канал по начертанию: `uniform ivec4 u_style_channel` (подстановка FR-8.3 — на CPU, в шейдер уходят итоговые номера каналов). Загрузка: при сбросе страницы — целиком, иначе — только слоты из `Atlas::take_dirty_slots` через `glTexSubImage2D`.
- Текстура данных `RGBA32UI` `cols × rows`: R = слот | флаги « 16, G = fg, B = bg, A = 0 (§6.1); начертание шейдер берёт из флагов `BOLD`/`ITALIC`. Загружаются только dirty-строки; при смене сетки — пересоздание.
- Размер страницы атласа — `page_px_for_limit(GL_MAX_TEXTURE_SIZE)`; Software в приложении использует тот же размер, чтобы оба бэкенда вели себя одинаково.
- Проверки §9: версия, `GL_MAX_TEXTURE_SIZE`, компиляция/линковка, `glGetError` после создания текстур; `GL_RENDERER` с `GDI Generic` → отказ. `llvmpipe`/`softpipe` допускаются (по умолчанию `--allow-sw-gl`).
- Чтение кадра (тесты, §11.2): рендер в FBO с текстурой `RGBA8` размера окна, `glReadPixels`, переворот строк → `0x00RRGGBB`.

## Шаги

| # | Шаг | Проверка |
|---|---|---|
| 1 | `Cargo.toml`: `glow` обязателен, `glutin 0.32`, `glutin-winit 0.5`, `raw-window-handle 0.6` без default-features; убрать feature `gl` и `bytemuck`; `AGENTS.md` — гейты без `--features gl` | сборка |
| 2 | `shaders/cell.vert`, `shaders/cell.frag` (без `#version` — заголовок подставляется: `330 core` / `300 es` + precision); фрагментный — дословно §7.3 | компиляция в контексте (тест) |
| 3 | `gl_backend.rs`: `GlContext` (glutin: конфиг → контекст 3.3 core, при отказе GLES 3.0 → поверхность → current → `glow`; проверки §9), `GlBackend` (программа, VAO, текстуры, uniform'ы, `render` = подготовка глифов → загрузка dirty → один `draw_arrays(TRIANGLES, 0, 3)` → swap), `render_to_frame` через FBO | `cargo test --test stage4_gl` |
| 4 | `app.rs`: `Backend::{Gl, Soft}`; окно через `DisplayBuilder` (на WGL окно создаётся до выбора конфига), при любой ошибке GL — лог в stderr и Software на том же окне (риск §14 п.1 проверяется запуском) | `cargo run`: в stderr `GL_VENDOR/RENDERER/VERSION`, бэкенд `OpenGL`; Esc |
| 5 | `tests/stage4_gl.rs`: цикл событий `with_any_thread`, в `resumed` — GL-контекст на скрытом окне, кадр golden-сцены через FBO против `SoftBackend` (допуск 0); GL недоступен → тест печатает причину и завершается успешно (§11.2 «пропускается») | прогон на этой машине |
| 6 | ТЗ: §6.1 (A-канал, начертание из флагов), §6.3 (`RGBA8UI`, `u_style_channel`), §7.2 (+`u_style_channel`, `u_atlas_cols`, `u_viewport_h`), §9 п.6; `PROJECT_MAP`, `AGENTS` | grep |
| 7 | Гейты; размер release с профилем §10.2; `cargo bloat --crates` | отчёты `docs/done|test|verify` |
