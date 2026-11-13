# Отчёт о тестах: выделение и буфер обмена в `TextInput` (ADR-0030)

- **Задача:** `2026-11-12-text-input-clipboard`
- **Окружение:** Windows 10 Pro 19045, Rust stable 1.98.1; тесты — debug, размеры — release; системный шрифт примера — `Cascadia Mono`.

## Состав

**`crates/gterm-widgets/tests/text_selection.rs` (6 тестов)** — состояние поля напрямую и поле в дереве (мышь через `Ui::route_mouse`).

| Тест | Что проверяет |
|---|---|
| `shift_with_movement_keys_extends_the_selection_and_plain_movement_drops_it` | `Shift+←/→/Home/End` от якоря, `←` — в начало выделения, `→` — в конец, у края выделения нет, `select_all`, `clear_selection` |
| `selection_is_counted_in_characters_and_a_disabled_field_ignores_everything` | кириллица — позиции в символах; неактивное поле клавишу не принимает |
| `typing_and_deleting_replace_the_selection_with_a_single_change_event` | набор заменяет выделенное одним `Changed`; `Backspace`/`Delete` удаляют выделенное; без выделения — как раньше; `Enter` выделение не трогает, `set_value` снимает |
| `dragging_with_the_mouse_selects_and_scrolls_past_the_edges` | движение без нажатия не принимается; нажатие — захват; протяжка выделяет; указатель правее и левее поля прокручивает; отпускание выделение сохраняет |
| `a_click_drops_the_selection_and_shift_click_extends_it` | клик снимает выделение, `Shift` + клик расширяет от курсора, набор после клика ничего лишнего не удаляет |
| `the_selection_is_drawn_only_under_focus_and_replaces_the_cursor_cell` | обмен fg/bg у выделенных ячеек, `selection_style`, без фокуса выделение не рисуется, без выделения — курсор |

**`crates/gterm-widgets/tests/clipboard.rs` (6 тестов)** — экран: поле, заглушка без поддержки правки, карта сочетаний с `Ctrl+C/X/V/A`, `MemoryClipboard`; события через `Ui::route_key_event`, логическая клавиша — кириллица.

| Тест | Что проверяет |
|---|---|
| `commands_come_from_physical_keys_with_exact_modifiers` | таблица `EditCommand::from_key`: семь сочетаний, отказ при лишнем или отсутствующем модификаторе |
| `copy_cut_and_paste_go_through_the_clipboard_and_report_changes` | `Ctrl+A`, `Ctrl+C`, `Ctrl+X` (`Changed("")`), `Ctrl+V` (`Changed`), вставка заменяет выделенное, пустое выделение буфер не трогает |
| `classic_combinations_work_and_pasted_line_breaks_become_spaces` | `Ctrl+Insert`, `Shift+Delete`, `Shift+Insert`; `"a\r\nb\tc\u{7}d\ne"` → `"a b cd e"` |
| `the_field_absorbs_edit_commands_and_other_controls_let_them_reach_the_map` | при фокусе в поле привязки карты молчат; при фокусе на заглушке срабатывают все четыре |
| `without_a_connected_clipboard_the_field_still_absorbs_and_keeps_its_text` | описание без `Ui::clipboard`: команды поглощены, вырезание текст не удаляет, посторонний буфер не тронут |
| `a_field_in_a_modal_layer_uses_the_clipboard_of_the_screen` | поле слоя копирует и вставляет через буфер экрана, поле экрана под слоем не тронуто |

**`tests/hotkeys.rs`** — фокус на заглушку ставится явно (`focus_first` + `focus_next`) с утверждением: прежде три теста шли с фокусом в поле.

## Результат прогона

- Базовый прогон до правок — коммит `e27d9c7`: 265 тестов, 0 падений.
- Первый прогон после правки поля: 1 падение в `tests/hotkeys.rs` (`a_simple_combination_fires_on_press…`, строка 193: `left: []`, `right: [Custom("copy")]`) — тест рассчитывал на фокус заглушки, фактически фокус был в поле, которое теперь поглощает `Ctrl+C`; исправлена посылка теста, утверждения не менялись.
- `cargo test -p gterm-widgets`: 82 теста, 0 падений; с `--no-default-features` — 82. `cargo test` в корне: **277 тестов**, 0 падений, 39 бинарников, код возврата 0.
- Мутации (исходник восстановлен побайтно): поле не поглощает → 5 падений; вырезание без `is_available` → 1; табуляция не заменяется → 1 (`left: "a bcd e"`); выделение без фокуса → 1; `←` мимо начала выделения → 1.
- Гейты: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` (и `-p gterm-widgets --no-default-features`) — чисто.

## Запуски

| Команда | stderr |
|---|---|
| `cargo run -p gterm-widgets --example widget_edit -- --backend=soft --fail-frames=2` | `backend Software`, `frame failed … retrying once`, `render failed … injected by --fail-frames`, код 1 |

Размеры release — в `docs/done/2026-11-12-text-input-clipboard.md`.

## Ограничения

- `SystemClipboard` автотестом не покрыт (общий ресурс машины) и вживую не запускался — только компиляция.
- `Ui::key` с настоящим `KeyEvent` — глазами; логика — через `route_key_event`.
- Время полного прогона runner не суммирует — не измерено.
