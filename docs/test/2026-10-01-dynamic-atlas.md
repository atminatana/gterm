# Тесты: динамический атлас, начертания, `assets/fonts/`

## Состав

`tests/stage2_font.rs` — 24 теста:

| Модуль | Тесты | Проверяют |
|---|---|---|
| `family` (5) | 4 начертания по `OS/2`; `find_assets_dir` из cwd тестов; подстановка (только Regular → все Regular; без BoldItalic → Italic; Regular+Bold → Italic→Regular, BoldItalic→Bold); нет Regular / пустой / нет `fonts/` → ошибки; метрики = `cell_metrics(Regular)` | FR-8.1…8.5 |
| `rasterization` (5) | полные и частичные пиксели; пробел и U+FFFF пустые; начертания различаются, Bold тяжелее; подстановка байт-в-байт; псевдографика во всех начертаниях; детерминизм | FR-8.4 |
| `procedural_box_drawing` (6) | как раньше | ADR-0002 |
| `atlas` (8) | лимит страницы (None→1024, 512→1024, 1536→1536, 16384→2048); промах/попадание; ленивые каналы в одном слоте; отсутствующий → слот 0 без выделения; псевдографика во всех каналах сразу; переполнение → сброс и работа; dirty-слоты один раз; геометрия слотов | FR-9 |

`tests/stage3_render.rs` — 20 тестов (18 + 2 GL `#[ignore]`): + `bold_and_italic_flags_select_atlas_channels`, `atlas_reset_mid_frame_still_yields_the_same_frame`, `screen_larger_than_atlas_page_is_an_error_not_garbage`; golden со строками Bold/Italic/BoldItalic.

`tests/stage1_buffer.rs` — 8 тестов (без изменений).

## Прогон

```
cargo test                 → 52 passed (8 + 24 + 20); stage2 0.08 s, stage3 0.15 s
cargo test --features gl   → 52 passed, 2 ignored
cargo clippy --all-targets -- -D warnings (+ --features gl) → чисто
cargo fmt --check          → чисто
demo.exe из target/debug и из /tmp → четыре файла найдены в assets/fonts, окно открылось
release (--config профиль §10.2) → 919 552 байт
```

## Ограничения

- NFR-6 — по чтению кода, без счётного аллокатора.
- `find_folder` в деплое (exe вне дерева проекта) — вручную.
- GL-канал атласа (`RGBA8UI`, `dirty_slots`) — этап 4.
