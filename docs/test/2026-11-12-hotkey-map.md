# Отчёт о тестах: карта «сочетание → сообщение» (ADR-0029)

- **Задача:** `2026-11-12-hotkey-map`
- **Окружение:** Windows 10 Pro 19045, Rust stable 1.98.1; тесты — debug, размеры — release; системный шрифт примеров — `Cascadia Mono`.

## Состав (`crates/gterm-widgets/tests/hotkeys.rs`, 7 тестов)

Тестовый экран: поле ввода `TextInput` (фокус по умолчанию) и свой контрол-заглушка; по желанию — модальный слой со своим контролом, картой и `on_escape`. События подаются через `Ui::route_key_event` (физическая клавиша, состояние, автоповтор, `KeyMsg`).

| Тест | Что проверяет |
|---|---|
| `map_keeps_declaration_order_and_reports_parse_errors_with_position` | пустая карта, `len`, из двух совпавших привязок — объявленная раньше, альтернатива образца, `bind_str("Ctrl+Ж")` → `UnknownKey` на позиции 5 |
| `a_simple_combination_fires_on_press_and_again_without_releasing_the_modifier` | модификатор сам привязку не вызывает; `Ctrl+S` — на нажатие `S`; автоповтор и отпускание молчат; при удержанном `Ctrl` подряд `S`, `C`, `V` — каждая своя привязка, история `Ctrl+[S,S,C,V]` |
| `a_chord_fires_on_its_last_press_and_the_full_history_wins_over_held_keys` | `Ctrl+[K,U]` — на нажатие `U`, хотя `Ctrl+U` объявлена раньше; без `K` — `Ctrl+U`; образец `K\|U` |
| `the_focused_control_goes_first_and_only_an_unhandled_press_reaches_the_map` | буква при фокусе в поле набирается, привязка `K` молчит; `Ctrl+S` при фокусе в поле — привязка, значение поля не меняется; при фокусе на заглушке клавиша и текст предложены ей, затем карта; контрол, принявший нажатие, — привязки нет, трекер нажатие учёл |
| `under_a_modal_layer_only_its_own_map_works` | без слоя — карта экрана; слой без карты — тишина, трекер кормится; карта слоя — её `P`, `Q` экрана не действует; `Esc` — карта слоя без `on_escape`, только `on_escape` при его наличии; клавиши при слое получал его контрол |
| `a_map_attached_to_a_nested_description_is_lifted_to_the_root` | `hotkeys` на дочернем описании работает после сборки контейнера |
| `without_a_map_key_routing_is_unchanged` | без карты: отпускание контролам не идёт, нажатие — контролу под фокусом |

## Результат прогона

- Базовый прогон до правок — коммит `27b67d5`: 258 тестов, 0 падений.
- `cargo test -p gterm-widgets --test hotkeys`: 7 тестов, 0 падений. `cargo test -p gterm-widgets`: 70 тестов. `cargo test` в корне: **265 тестов**, 0 падений, 37 бинарников; 4 оконных (`stage6_events`, `stage6_events_auto`, `stage6_fallback`, `stage8_faults`) запущены, код возврата 0.
- Мутации (исходники восстановлены, прогон после восстановления зелёный): порядок `[current, full]` → 1 падение (аккорд); карта без условия `handled == No` → 2 падения (контрол раньше карты, слой); карта экрана под слоем без карты → 1 падение (слой).
- Гейты: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` — чисто.

## Запуски

| Команда | stderr |
|---|---|
| `cargo run -p gterm --example hotkeys -- --backend=soft --fail-frames=2` | `font 'Cascadia Mono'`, `backend Software`, `frame failed … retrying once`, `render failed … injected by --fail-frames`, код 1 |
| `cargo run -p gterm-widgets --example widgets -- --backend=soft --fail-frames=2` | то же, код 1 |
| `cargo run -p gterm-widgets --example widget_edit -- --backend=soft --fail-frames=2` | то же, код 1 |

Размеры release — в `docs/done/2026-11-12-hotkey-map.md`.

## Ограничения

- `KeyEvent` без окна не сконструировать, оконные тесты ввод и фокус не инжектируют: обёртка `Ui::key`, ветка `WindowEvent::Focused` и `on_focus` примеров проверяются чтением кода и глазами; логика — через `route_key_event`.
- Раскладко-независимость привязок на окне — только глазами.
- Время полного прогона runner не суммирует — не измерено.
