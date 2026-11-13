# Тесты: этап 4 — GL-бэкенд

## Состав

`tests/stage4_gl.rs` — 1 тест `gl_frames_match_software_frames_bit_for_bit`:

| Шаг | Что делает |
|---|---|
| Цикл событий | `EventLoop::builder().with_any_thread(true)` (Windows / X11); `run_app` с обработчиком, который всё делает в `resumed` и выходит |
| Контекст | `DisplayBuilder` + скрытое окно 250×140 → `GlContext::new` (3.3 core, при отказе GLES 3.0) → `GlBackend::new` с атласом по `GL_MAX_TEXTURE_SIZE` |
| Сцена 1 | golden (24×6, все флаги, 4 начертания, курсор Block, blink off), окно 250×140, scale 1 |
| Сцена 2 | та же сетка, scale 2 — сетка больше окна, обрезка; курсор Underline, blink on, другая рамка |
| Сцена 3 | окно 507×285, scale 2 — центрирование, рамка |
| По 2 кадра на сцену | второй кадр без изменений буфера — dirty-путь (ничего не грузится) обязан дать тот же кадр |
| Сравнение | `GlBackend::render_to_frame` (FBO + `glReadPixels`) против `SoftBackend::frame`, допуск 0; при расхождении — номер пикселя и оба значения |
| Пропуск | конфиг/контекст не создался → `GL test skipped: …` и успех; ошибка после создания контекста → падение |

`tests/common/mod.rs` — общие константы, шрифт, golden-сцена и виды (используются stage3 и stage4).

## Прогон

```
cargo test --test stage4_gl -- --nocapture
  GL: NVIDIA Corporation / NVIDIA GeForce GTX 1060 3GB/PCIe/SSE2 / 3.3.0 NVIDIA 581.57 (max texture 32768)
  GL ↔ Software identical on OpenGL 3.3, 3 scenes
  test result: ok. 1 passed (0.78 s)
cargo test                 → 53 passed (8 + 24 + 20 + 1)
cargo clippy --all-targets -- -D warnings → чисто
cargo fmt --check          → чисто
demo.exe                   → "backend OpenGL 3.3 — NVIDIA … (GL_MAX_TEXTURE_SIZE 32768)", окно открылось
release (--config профиль §10.2) → 1 036 288 байт
cargo bloat --release --crates (opt-level z): skrifa 297 KiB, gterm 236, std 144, winit 105, read_fonts 67, glow 34, glutin 32, softbuffer 31
```

## Ограничения

- Прогон только на NVIDIA/OpenGL 3.3; GLES-путь и откат на Software на реальном отказе не проверены.
- Тест зависит от дисплея: на CI без GPU/дисплея он печатает причину и проходит — регрессию GL такой прогон не ловит.
- Чтение кадра из FBO, не из экрана: экранный путь (`swap_buffers`) проверяется только запуском демо.
