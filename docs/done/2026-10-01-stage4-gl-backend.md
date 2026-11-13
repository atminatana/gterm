# Выполнение: этап 4 — GL-бэкенд

- План: `docs/plans/2026-10-01-stage4-gl-backend.md`; ТЗ §6, §7, §9, §11.2; ADR-0002/0003 применены к GPU-пути. Новых ADR нет: решения (`RGBA8UI` с каналами, слот + флаги в data-текстуре, FBO для чтения) — прямое следствие принятых.
- Этап «ТЗ» (`docs/spec/`) не заводился — постановка в ТЗ проекта.

## WHAT

| Файл | Изменение |
|---|---|
| `Cargo.toml` | `glow` обязателен; + `glutin 0.32`, `glutin-winit 0.5`, `raw-window-handle 0.6` (без default-features); feature `gl` и `bytemuck` удалены (§10.3) |
| `src/renderer/shaders/cell.vert` | полноэкранный треугольник по `gl_VertexID` |
| `src/renderer/shaders/cell.frag` | алгоритм §7.3 дословно: data-текстура → слот/флаги/цвета, INVERSE → BOLD → курсор-блок, канал по `u_style_channel`, BLINK/UNDERLINE/курсор, целочисленное смешивание |
| `src/renderer/gl_backend.rs` | переписан: `GlContext` (конфиг 8/8/8, без depth/stencil/MSAA; контекст 3.3 core → GLES 3.0; поверхность; current; VSync; `glow`; `GL_VENDOR/RENDERER/VERSION/MAX_TEXTURE_SIZE`; отказ на `GDI Generic` и лимите < 1024), `GlBackend` (программа с подставляемым `#version`, пустой VAO, атлас `RGBA8UI`, данные `RGBA32UI`, uniform'ы §7.2, загрузка dirty-строк и dirty-слотов, один `draw_arrays(TRIANGLES, 0, 3)`, `glGetError` после первого кадра, `render_to_frame` через FBO) — весь `unsafe` здесь |
| `src/renderer/mod.rs` | `gl_backend` без `cfg(feature)` |
| `src/app.rs` | `Backend::{Gl, Soft}`; окно через `DisplayBuilder` (на WGL — до выбора конфига); GL → при любой ошибке Software на том же окне с логом причины; страница атласа по `GL_MAX_TEXTURE_SIZE` |
| `src/atlas.rs` | `Box<Rasterizer>`, `into_rasterizer` |
| `src/bin/demo.rs` | `App::new(…, rasterizer, …)` |
| `tests/common/mod.rs` | общие сцена/вид/шрифт |
| `tests/stage4_gl.rs` | цикл событий `with_any_thread`, скрытое окно, GL-контекст, 3 сцены × 2 кадра против Software (допуск 0); GL недоступен → причина и успех |
| `tests/stage3_render.rs` | GL-заглушки удалены |
| ТЗ (§6.1, §6.3, §7.1, §7.2, §9, §11.2, §12), `AGENTS.md`, `PROJECT_MAP.md` | обновлены |

## COMPLETENESS

| Требование | Код | Тест |
|---|---|---|
| FR-12 один draw call, fullscreen-треугольник | `draw`: `draw_arrays(TRIANGLES, 0, 3)`, `cell.vert` | бит-идентичность (кадр верен только если треугольник покрывает окно) |
| FR-13 `NEAREST`, целочисленный масштаб | `set_nearest_clamp`, `u_scale` | сцена `scale 2` в `stage4_gl` |
| FR-14 рамка, центрирование | `u_border`, `u_origin` | сцена «окно 2× больше» |
| FR-15/16 BLINK, UNDERLINE, курсор в шейдере | `cell.frag` | сцены с `blink_phase` 0/1 и курсором Block/Underline |
| §6.1 data-текстура `RGBA32UI`, §6.2 dirty-строки, пересоздание при смене сетки | `upload_data`, `recreate_data_texture` | второй кадр в тесте идёт по dirty-пути |
| §6.3 атлас `RGBA8UI`, загрузка dirty-слотов | `upload_atlas_page`, `upload_dirty_slots` | тест (новые глифы грузятся между сценами) |
| §9 п.1–7 | `config_template`, `pick_config`, `GlContext::new`, `query_info` | запуск: stderr с vendor/renderer/version |
| FR-17/18 (ядро): GL → Software при ошибке | `App::try_gl` | **ручная проверка отказа не выполнена** (см. ⚠) |
| FR-20 имя бэкенда и причина отката в stderr | `create_window_and_backend` | запуск |
| §11.2 сравнение с допуском 0 | `render_to_frame` (FBO + `glReadPixels`, переворот строк) | `gl_frames_match_software_frames_bit_for_bit` |
| NFR-7 | все пути `Result`; `glGetError` после init и первого кадра | — |
| NFR-9 `unsafe` только в `gl_backend.rs` | `grep -rn unsafe src/` → только `gl_backend.rs` | — |
| NFR-1 | release 1 036 288 Б (профиль §10.2) | — |

## CORRECTNESS

- Тест бит-идентичности на этой машине (NVIDIA GTX 1060, OpenGL 3.3.0 581.57): **3 сцены × 2 кадра — 0 отличий** с первого запуска. Сцены покрывают: все флаги, 4 начертания, курсор Block и Underline, фазу мигания 0 и 1, масштаб 1 и 2, обрезку сетки окном и центрирование с рамкой, dirty-путь (второй кадр без изменений).
- Демо запускается на GL-бэкенде; Esc закрывает.
- Шейдер и `soft_backend.rs` — одна и та же последовательность операций; обе реализации целочисленные.

## EVIDENCE

См. `docs/test/2026-10-01-stage4-gl-backend.md`, `docs/verify/2026-10-01-stage4-gl-backend.md`.

## ⚠ WARNINGS

- **Откат GL → Software не проверен на реальном отказе**: на этой машине GL всегда доступен. Код пути написан и компилируется; проверка требует ВМ без 3D или `--fail-gl` (этап 5, §11.4 п.4). `[data needed: запуск в окружении без GL]`.
- GLES 3.0 путь (ANGLE/Mesa) не проверен — на машине с NVIDIA первая попытка (3.3 core) всегда успешна. `[data needed: запуск с `LIBGL_ALWAYS_SOFTWARE`/Mesa или на GLES-драйвере]`.
- Потеря контекста (FR-21) не обрабатывается — этап 5; `RenderError::ContextLost` пока не возникает.
- `u_border` передаётся как `vec4` через деление на 255 — точно обратимо в UNORM8; при `GL_FRAMEBUFFER_SRGB` (мы его не включаем) бит-идентичность пропала бы. Зафиксировано в коде (`disable(BLEND)`, без sRGB).
- Тест стартует цикл событий в потоке `cargo test` (`with_any_thread`) — на macOS это не работает (платформа вне ТЗ).
- `GL_MAX_TEXTURE_SIZE` 32768 на NVIDIA → страница 2048² (кап ADR-0003 п.5); на слабых GPU страница 1024² — `page_px_for_limit` покрыт тестом.
- Размер: 1 036 КБ, +117 КБ за GL-стек; `[profile.release]` в `Cargo.toml` по-прежнему не задан (этап 7).
