# План: этап 3 — Software-рендерер по §7.3/§8, golden-тест, окно `winit` + `softbuffer`

- **ТЗ:** `docs/TZ_terminal_emulator.md` §3.3 (FR-12…FR-16), §5.2–5.3, §7.3, §8, §11.2, §12 этап 3.
- **Рамки:** `soft_backend.rs` переписывается как эталонная реализация алгоритма §7.3; `ViewParams` получает `cursor` (§5.2); `src/cursor.rs` (данные курсора, FR-7 без таймера); golden-тест; минимальное окно (`src/app.rs`, `src/bin/demo.rs`): создание по размеру сетки, `Resized` → `resize`, `RedrawRequested` → `render` + `softbuffer`, `Esc` — выход, `ControlFlow::Wait`.
- **Не входит:** GL-бэкенд (этап 4), цепочка fallback и `--backend`/`--fail-gl` (этап 5), мигание по таймеру, DPI, полноэкранный режим, `+`/`-` (этап 6), демо-сцена FR-27 целиком (этап 7).

## Открытые решения

Нет архитектурных. Детали алгоритма, которые ТЗ оставляло «при реализации», фиксируются в самом ТЗ §7.3 (они — контракт бит-идентичности GL↔Soft):
- BOLD = осветление цвета: `c' = min(c + 0x55, 255)` по каналам (целочисленно, одинаково в шейдере и CPU).
- Курсор: `Block` — swap(fg, bg) после INVERSE; `Underline` — `a = 1` в двух нижних строках глифа; `Bar` — `a = 1` в столбце `in_cell.x == 0`.
- Сетка центрируется: `origin = ((win − grid) / 2)` в пикселях окна (§7.2 `u_origin`).
- Формат кадра `0x00RRGGBB` (старший байт 0), как у `softbuffer`.

## Шаги

| # | Шаг | Проверка |
|---|---|---|
| 1 | `src/cursor.rs`: `CursorShape`, `CursorState`; `ViewParams { scale, border_color, blink_phase, cursor }` + `Clone/PartialEq` | компиляция обеих конфигураций |
| 2 | `soft_backend.rs` заново: предвыделенный кадр, `origin`, полная перерисовка при resize/смене `ViewParams`, иначе только dirty-строки; алгоритм §7.3 по пикселям; без аллокаций в `render` | пиксельные тесты: глиф, INVERSE, UNDERLINE, BLINK, BOLD, курсор ×3, рамка, центрирование, scale=2, dirty-строки не стирают остальное |
| 3 | Golden: `tests/golden/soft_demo_1x.bin` (raw `u32` LE) + тест сравнения; обновление через `GTERM_UPDATE_GOLDEN=1` | `cargo test --test stage3_render` |
| 4 | `Cargo.toml`: `winit 0.30`, `softbuffer 0.4` без default-features; `src/app.rs`: окно + цикл; `src/bin/demo.rs` вместо `src/main.rs` (§5.1) | `cargo run` показывает сцену, Esc закрывает |
| 5 | ТЗ §7.3 — вписать зафиксированные детали; `[profile.release]` §10.2 — **не трогаю** (отдельно, этап 7) | `grep` |
| 6 | Гейты fmt/clippy×2/test; размер release-бинарника | отчёты `docs/done|test|verify` |
