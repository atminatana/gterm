# Отчёт о тестах: двойной клик (ADR-0035) и ползунок `Slider` (ADR-0036)

- **Задача:** `2026-11-16-double-click-slider`
- **Окружение:** Windows 10 Pro 19045, Rust stable 1.98; тесты — debug, размеры — release; живая проверка — OpenGL 3.3.

## Двойной клик

### `crates/gterm/src/app/clicks.rs` — 5 unit-тестов (`#[cfg(test)]`), без окна и без часов

| Тест | Что проверяет |
|---|---|
| `second_left_press_in_the_same_cell_within_the_interval_is_double` | второе левое нажатие через 200 мс в той же ячейке — 2; отпускание несёт номер своего нажатия |
| `the_interval_boundary_is_inclusive` | ровно 500 мс — 2, 501 мс — 1; запоздавшее нажатие начинает новую пару |
| `a_press_in_another_cell_starts_over` | другая ячейка — 1, следующее там же — 2 |
| `another_button_is_never_double_and_breaks_the_left_series` | правая — всегда 1; средняя между левыми прерывает серию |
| `the_press_after_a_double_starts_counting_again` | пять нажатий по 100 мс — 1, 2, 1, 2, 1 |

### `crates/gterm-widgets/tests/input.rs` — +1 тест

| Тест | Что проверяет |
|---|---|
| `click_count_passes_through_and_marks_a_left_double_click` | `MouseMsg::from_pointer` переносит число; `is_double_click` — только у нажатия левой кнопки с числом 2; у движения и отмены число 0 |

### `crates/gterm-widgets/tests/double_click.rs` — 11 тестов через `Ui::route_mouse`, без окна

| Тест | Что проверяет |
|---|---|
| `a_button_publishes_the_binding_on_the_second_press_and_still_activates_on_each_release` | кнопка: каждое отпускание — `on_activate`, второе нажатие — сообщение двойного клика, `Handled::Yes` |
| `a_single_click_gives_no_double_click_message` | два одиночных клика — только активации |
| `a_disabled_button_gives_no_double_click_message` | неактивная кнопка — ничего не принято, сообщений нет |
| `a_checkbox_toggles_on_each_click_and_publishes_the_binding` | флажок: переключение на каждом отпускании и сообщение на втором нажатии |
| `in_a_list_the_row_under_the_pointer_is_selected_when_the_message_arrives` | первое нажатие выделяет строку (`Picked(2)`), на втором — сообщение, выделена строка 2 |
| `a_double_click_on_the_list_scrollbar_gives_no_message` | все четыре строки колонки полосы (стрелки, дорожка, ползунок) — без сообщения, выделение не тронуто |
| `a_double_click_below_the_last_item_gives_no_message` | пустая строка списка — без сообщения |
| `a_list_without_the_binding_routes_a_double_click_like_two_ordinary_clicks` | без привязки результаты и состояние списка те же, что у двух одиночных кликов |
| `a_table_counts_a_double_click_on_a_row_but_not_on_the_header_or_the_scrollbar` | таблица: заголовок и полоса — без сообщения, строка — `Picked(1)`, затем сообщение |
| `an_application_control_gets_the_press_and_the_binding_adds_the_message` | свой контрол: без привязки — `Handled::No` как раньше; с привязкой — нажатие доставлено, сообщение добавлено, захвата нет |
| `a_control_in_a_modal_layer_gets_its_double_click_binding` | контрол в модальном слое — сообщение, нажатие в локальных ячейках слоя |

## Ползунок — `crates/gterm-widgets/tests/slider.rs`, 10 тестов без окна

| Тест | Что проверяет |
|---|---|
| `arrows_step_home_and_end_jump_to_the_bounds_and_a_bound_gives_no_event` | `→`/`←` на шаг 5, `End`/`Home` — границы; в границе `Handled::Yes` без сообщения; прочие клавиши — `No` |
| `a_step_that_overshoots_a_bound_stops_at_the_bound` | шаг 4 от 9 при максимуме 10 — 10 |
| `a_press_on_the_track_sets_the_value_by_the_pointer_position` | колонки 5, 0, 10, 3 → 50, 0, 100, 30 (середина и оба края); захват на нажатии, снят на отпускании; то же значение — без сообщения; правая кнопка — не принята |
| `the_value_rounds_to_the_nearest_column_when_the_range_is_wider_than_the_track` | дорожка 4 ячейки со сдвигом, −50…50 → −50, −17, 17, 50 |
| `dragging_under_capture_follows_the_pointer_until_release` | протяжка: новое значение на новой колонке, та же колонка — без сообщения, за краями — границы; отпускание значение не меняет; без захвата движение не принято |
| `leaving_the_window_ends_the_drag_and_keeps_the_value_reached` | `Cancel` снимает захват, значение 80 остаётся |
| `the_wheel_changes_the_value_by_the_step` | колесо ±шаг, меньше половины строки — без шага, в границе — без сообщения |
| `a_disabled_slider_takes_no_focus_and_no_input` | неактивный: не фокусируется, клавиши, клик и колесо не приняты |
| `programmatic_changes_give_no_event_and_bounds_clamp_the_value` | `set_value` зажимает; `set_range` зажимает значение, `max < min` поднимается; `set_step(0)` → 1; событий нет; `Default` — 0…100, шаг 1 |
| `the_track_shows_the_filled_part_the_thumb_and_the_styles_of_the_state` | снимки `━━━━━█─────`, `█──────────`, `━━━━━━━━━━█` в средней строке; цвет заполненной части; фон фокуса на всей области; неактивный стиль вместо фокуса; свои знаки |

## Результат прогона

| Прогон | Команда | Результат | Журнал |
|---|---|---|---|
| База (`e7c0403`) | `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` | чисто, чисто, 313 тестов, 0 падений | `target/step-dc/baseline.log` |
| После двойного клика | те же | чисто, чисто, 330 (+17), 0 падений | `target/step-dc/feature1.log` |
| После ползунка | те же | чисто, чисто, 340 (+10), 0 падений | `target/step-dc/feature2.log` |
| Итог после документов и мутаций | те же | чисто, чисто, 340 тестов, 0 падений | `target/step-dc/final.log` |
| Независимый прогон перед публикацией | те же | чисто, чисто, 340 тестов в 48 наборах, 0 падений | `target/step-dc/lead-review.log` |
| Мутации | `target/step-dc/mutate.sh`, 8 штук | все пойманы, исходники восстановлены (`cmp`) | `target/step-dc/mutation-*.log` |

Числа — сумма строк `test result` наборов с обвязкой; 5 оконных бинарников `harness = false` (`stage6_events`, `stage6_events_auto`, `stage6_fallback`, `stage8_faults`, `tick_events`) прошли в каждом прогоне (код возврата `cargo test` — 0) и в сумму не входят.

## Ограничения

- Двойной клик в настоящем окне не проверялся: `tools/shot.ps1` шлёт один клик за вызов. Счётчик проверен unit-тестами, дерево — с `click_count: 2` в сообщении.
- Протяжка и колесо ползунка в настоящем окне не проверялись (снимок — клавиши `Tab`/`→`).
- Linux не проверялся.

## Поправка после публикации (2026-11-16) — захват указателя у поля выпадающего списка

| Тест | Файл | Что проверяет |
|---|---|---|
| `a_field_opened_by_a_click_does_not_keep_the_pointer_after_the_list_is_closed` | `crates/gterm-widgets/tests/combo_box.rs` | поле, раскрытое кликом, не держит захват, пока список раскрыт; после закрытия по Esc и кликом мимо слоя следующее нажатие достаётся контролу под указателем, список остаётся закрытым |

Прогон: чисто, чисто, 341 тест в 48 наборах, 0 падений (`target/step-dc/combo-fix.log`).
