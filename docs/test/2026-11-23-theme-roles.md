# Отчёт о тестах: тема в три уровня, вывод недостающих областей (ADR-0038)

ТЗ — `docs/spec/2026-11-23-theme-roles.md`. Тесты без окна, `crates/gterm-widgets/tests/theme_derive.rs` — 15 тестов.

## Состав

| Тест | Что проверяет | Критерий |
|---|---|---|
| `nothing_is_derived_without_background_or_text` | нет `ui.text`, нет `ui.background`, область есть без нужного цвета — тема не меняется | К1 |
| `light_theme_shifts_the_control_down_and_takes_the_accent_by_fallback_steps` | ступени акцента: `ui.text.focus` → `ui.selection` → цвет текста; светлая тема сдвигает фон вниз | К1, К2 |
| `dim_and_error_take_helix_scopes_when_the_theme_has_them` | `ui.virtual` и `diagnostic.error` | К1 |
| `dark_tones_follow_the_formulas` | все десять тонов и `error` тёмной темы — значения посчитаны вручную; пометки `is_derived` | К2 |
| `darkness_boundary_and_channel_saturation` | сумма каналов 382 / 383; канал не переполняется | К2 |
| `widget_scopes_are_links_to_tones` | все 23 области виджетов: 17 ссылок целиком, 5 «только цвет символа» без начертания, заливка индикатора | К3 |
| `accent_given_as_background_only_is_completed_with_the_text_on_it` | неполный `ui.accent` дополняется, заданное не меняется | К3 |
| `a_tone_given_in_the_theme_reaches_every_widget_that_links_to_it` | заданный `ui.control` доходит до кнопки, поля, списка, состояний и индикатора | К3 |
| `a_widget_scope_given_in_the_theme_is_stronger_than_the_tone` | заданная область виджета не заменяется; `ui.combo` берёт заданное `ui.input` | К3 |
| `a_theme_described_by_hand_gets_no_computed_color` | все тона заданы — в 23 областях виджетов нет цвета, которого нет в заданных | К3а |
| `derive_is_repeatable_and_set_makes_a_derived_scope_given` | повторный вывод, пересчёт после `set`, `set` снимает пометку | К3, контракт |
| `buttons_of_a_base_only_theme_are_painted_with_derived_scopes` | кадр дерева `Ui`: кнопка в четырёх состояниях на теме из одной основы, явный стиль сильнее | К4 |
| `built_in_theme_is_derived_and_keeps_its_given_widget_scopes` | встроенная тема выведена, её заданные области целы, недостающие равны прежним цепочкам | К5, контракт |
| `files::a_loaded_theme_is_derived` | вывод при загрузке из текста | К4 |
| `files::every_example_theme_loads_clean_and_is_complete` | 24 темы `examples/themes/` (22 sniffnet и две свои): без предупреждений, все тона и области виджетов есть | К6 |

Запасные цепочки ADR-0037 для темы без вывода по-прежнему держит `tests/theme_widgets.rs` (`theme_without_widget_scopes_falls_back_to_helix_scopes` и остальные 15) — эти тесты не правились.

## Результат прогона

- `cargo test` (каталог сборки `target/gate`): **363 теста, 0 падений** в 49 бинарниках; `gterm-widgets` — 168. До задачи (коммит `4920525`): 348 / 153.
- Существующие тесты не правились ни в одной строке (К5: перечень правок пуст).
- `GTERM_HELIX_THEMES=D:\Soft\helix\runtime\themes cargo test -p gterm-widgets --test theme_load every_helix`: тем 116, ошибок 0, предупреждений 590 — как до задачи.

## Мутации

Тесты прошли с первого прогона, поэтому проверены 16 мутациями (`target/mutate_derive.py`; исходники восстановлены побайтно, сверка SHA-256) — все дают падения:

| Мутация | Упало тестов |
|---|---|
| доля выделения 1/3 → 1/4 | 2 |
| доля фокуса поля 1/4 → 1/3 | 2 |
| сдвиг 38 → 39 | 6 |
| граница тёмной темы 382 → 381 | 1 |
| смесь без округления | 2 |
| сдвиг с переполнением вместо насыщения | 1 |
| выведенное заменяет заданное | 10 |
| прежние выведенные области не отбрасываются | 1 |
| `ui.button.pressed` ссылается на тон фокуса | 3 |
| «только цвет символа» несёт стиль целиком | 1 |
| акцент пропускает `ui.text.focus` | 1 |
| состояния `ui.control.*` не видят заданный `ui.control` | 1 |
| неполный акцент не дополняется | 1 |
| вывод работает без `ui.text` | 1 |
| `set` не снимает пометку «выведена» | 1 |
| встроенная тема не выведена | 1 |

## Ограничения

- Вид проверен глазами по снимкам окна только на OpenGL 3.3 (Windows) и на пяти темах из 140 доступных; контраст выведенных пар тестами не проверяется.
- Цвета всех 22 тем sniffnet сверены с источником отдельной командой (10 файлов и 6 модулей по два набора — совпадают); автотестом это не закреплено — клона sniffnet в репозитории нет.
- Время полного прогона runner не суммирует — не измерено.
