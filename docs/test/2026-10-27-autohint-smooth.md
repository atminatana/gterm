# Отчёт о тестах: автохинтер со сглаженной целью и предпочтение файла с родными инструкциями

- **Задача:** `2026-10-27-autohint-smooth` (ADR-0025)
- **Окружение:** Windows 10 Pro 19045, NVIDIA GTX 1060 3GB (драйвер 581.57), Rust stable 1.98.1; тесты — debug, размеры — release; системный шрифт — `Cascadia Mono` (в системе и `.ttf`, и `.otf`)

## Состав

| Файл | Тесты |
|---|---|
| `crates/gterm/tests/stage2_font.rs::sources::face_with_glyph_instructions_is_preferred_regardless_of_order` (1, **К1**) | два Regular — исходный файл репозитория (инструкций нет) и его копия с `maxp.maxSizeOfInstructions = 1`; в обоих порядках `face(style).path` для всех четырёх начертаний — копия |
| `rasterization::uninstructed_font_is_autohinted_with_smooth_target` (1, **К2**) | `к`, `С` при 16 px: растр `Rasterizer` = прямой рендер `skrifa` (`Engine::Auto`, `Smooth/Normal`) + `zeno` тем же путём; ≠ рендер с `Target::Mono` |
| `stage3_render::golden` (**К4**) | эталоны перезаписаны `GTERM_UPDATE_GOLDEN=1`: до перезаписи 16 px — 2 639 из 35 000 пикселей (7,5 %), 32 px — 4 305 из 124 974 (3,4 %) |
| `stage4_gl` (**К4**) | GL 3.3 и GLES 3.0 — identical, 10 сцен |
| прежние (201) | без изменений, кроме golden |

## Результат прогона

- Базовый прогон до изменений: `cargo test --workspace` — 27 наборов, 201 тест, 0 падений, код 0.
- После изменений: `cargo test --workspace` — 27 наборов, **203 теста**, 0 падений, код 0; `stage2_font` 51; оконные бинарники ok.
- Мутация (исходник восстановлен копией): `hinting_target` → `Mono` для автохинтера — К2 падает (`к smooth`).
- Гейты: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` — чисто.

## Запуски

| Команда | Вывод |
|---|---|
| `cargo run -p gterm-widgets --example widgets` (до) | `widgets: font 'Cascadia Mono' — C:\Windows\Fonts\CascadiaMono-Regular.otf` |
| `cargo run -p gterm-widgets --example widgets` (после) | `widgets: font 'Cascadia Mono' — C:\Windows\Fonts\CascadiaMono-Regular.ttf`, `backend OpenGL 3.3 … glyph_px 14` |
| `cargo test -p gterm --test stage4_gl -- --nocapture` | `OpenGL 3.3: GL ↔ Software identical, 10 scenes`; `OpenGL ES 3.0: GL ↔ Software identical, 10 scenes` |

Дампы растра (временный крейт `target/scratch_hint`, удалён): `CascadiaMono-Regular.otf` 14 px `Mono` — стойки `к` 2 px (`@@`), совпадает со скриншотом владельца; `Smooth/Normal` — 1 px; `CascadiaMono-Regular.ttf` 14 px — `Mono` ≡ `Smooth`; RobotoMono 16 px `Mono` — `С` 8 колонок из 10, `Smooth/Normal` — 9 из 10; `Smooth/Light` — стойка `к` в две серые колонки.

Размер release: `demo.exe` 1 543 168 Б (+512), `full_table.exe` 1 536 512, `overlay.exe` 1 531 904, `minimal.exe` 1 164 800, `pseudographica.exe` 1 170 944, `widgets.exe` 1 189 888 Б.

## Ограничения

- Вид текста — глазами на машине владельца.
- Ветка «интерпретатор + `Mono`» автотестом не покрыта: инструктированного шрифта в репозитории нет.
- К2 опирается на шрифт репозитория: для него `Mono` и `Smooth/Normal` различаются на `к` и `С` при 16 px; на другом шрифте пара знаков может совпасть — тогда выбрать другие.
