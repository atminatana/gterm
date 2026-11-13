# Тесты: частичная перерисовка Software

## Состав

`tests/soft_partial_redraw.rs` — 8 тестов, сетка 10×5, строка 2 мигает; каждый частичный кадр сравнивается с кадром нового рендерера, нарисованным с нуля:

| Тест | Проверяет |
|---|---|
| `first_frame_and_resize_damage_the_whole_frame` | первый кадр и кадр после `resize` — один прямоугольник на весь кадр |
| `unchanged_frame_has_no_damage` | без изменений полос нет, кадр тот же |
| `blink_toggle_redraws_only_rows_with_blink` | три смены фазы — ровно полоса строки 2; кадр = кадр с нуля |
| `dirty_and_blink_rows_merge_into_bands` | dirty-строка 1 + мигающая 2 → одна полоса; строки 0 и 4 → две полосы |
| `border_color_change_redraws_the_whole_frame` | смена рамки — полный кадр |
| `bands_are_clipped_by_a_window_smaller_than_the_grid` | окно уже сетки и режет мигающую строку — полоса обрезана по окну |
| `row_below_the_window_gives_no_band` | мигающая строка целиком за окном — полос нет |
| `long_sequence_of_partial_frames_equals_frame_from_scratch` | 12 шагов (строки, новые `BLINK`, фазы) в окне больше сетки с нечётными полями |

`tests/stage4_gl.rs` — у каждой из 6 сцен третий кадр со сменой фазы мигания: Software частично по тому же экрану, GL полностью; допуск 0, оба уровня GL.

Мутация: отключение перерисовки строк с `BLINK` роняет 4 из 8 тестов.

## Результат прогона

`cargo test`: 131 тест ok + 4 оконных бинарника ok; `stage4_gl`: `GL ↔ Software identical, 6 scenes` на OpenGL 3.3 и OpenGL ES 3.0.

## Замеры

См. `docs/done/2026-10-08-soft-partial-redraw.md`, EVIDENCE: кадр смены фазы 6,095 → 0,226 мс; `--bench` Software без регрессии (A/B 9,6–9,7 против 9,8–10,9 мс).

## Ограничения

- Вывод в окно (`SoftState::present`) автотестами не покрыт — нужен настоящий `softbuffer`; исполняется оконными тестами и запусками, ветка `age = 2` (Wayland) здесь не исполнялась.
- CPU процесса в простое на этой машине шумный (движение мыши, фон) — не использован как доказательство.
