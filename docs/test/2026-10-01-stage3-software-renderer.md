# Тесты: этап 3 — Software-рендерер

## Состав (`tests/stage3_render.rs`, 15 тестов; 2 из них — GL `#[ignore]`)

| Модуль | Тест | Проверяет |
|---|---|---|
| `algorithm` | `glyph_pixels_follow_atlas_and_background_is_always_drawn` | каждый пиксель ячейки = fg/bg по атласу; пустая ячейка — фон, не рамка; размер кадра = сетке |
| | `frame_format_is_00rrggbb_everywhere` | старший байт 0 во всём кадре (включая рамку) |
| | `inverse_swaps_colors` | INVERSE: чернила → bg, фон → fg |
| | `underline_fills_last_glyph_row_with_fg` | UNDERLINE: последняя строка глифа fg, предпоследняя bg |
| | `blink_hides_ink_only_in_off_phase` | BLINK: фаза on — видно, off — bg |
| | `bold_lightens_fg_by_0x55_per_channel_with_saturation` | `0x102030 → 0x657585`, `0xF000FF → 0xFF55FF` |
| | `cursor_block_inverts_underline_takes_two_rows_bar_takes_left_column` | три формы курсора, невидимый курсор, соседняя ячейка не затронута |
| | `integer_scale_replicates_each_glyph_pixel_as_a_block` | scale 3 = каждый пиксель scale-1 кадра блоком 3×3 |
| | `grid_is_centered_and_leftover_is_border` | origin = остаток/2, углы и поля — рамка |
| | `window_smaller_than_grid_clips_without_panic` | кадр 7×5 при сетке больше — без паники |
| | `zero_scale_is_an_error_not_a_panic` | `scale = 0` → `Err` |
| `dirty_rows` | `second_render_redraws_only_dirty_rows_and_keeps_the_rest` | не-dirty строки сохраняются, dirty перерисована, флаги сброшены |
| | `view_change_forces_full_redraw` | смена `ViewParams` + resize → рамка новым цветом, новый размер |
| `golden` | `software_frame_matches_golden_dump` | побайтное равенство с `tests/golden/soft_golden_1x.bin`; `GTERM_UPDATE_GOLDEN=1` перезаписывает |
| | `rendering_the_same_scene_twice_is_bit_identical` | детерминизм |
| GL | `gl_backend_context_lost_without_window`, `gl_backend_frame_matches_software_frame` | унаследованные, `#[ignore]` до этапа 4 |

## Прогон

```
cargo test                 → 39 passed (7 + 17 + 15), stage3 0.07 s
cargo test --features gl   → 39 passed, 2 ignored
cargo clippy --all-targets -- -D warnings (+ --features gl) → чисто
cargo fmt --check          → чисто
./target/debug/demo.exe    → окно открылось, stderr: "font … glyph 10x18, 820 slots, cache Reused"; снято по таймауту 6 с
```

## Ограничения

- Golden — самосогласованный эталон (сгенерирован этим же рендерером, проверен визуально как PNG), защищает от регрессий, не доказывает соответствие §7.3 сам по себе — это делают пиксельные тесты.
- Окно/`softbuffer` проверяются только запуском; `Esc`, resize, свёрнутое окно — вручную.
- NFR-6 — без инструментального теста.
