# Проверка критериев приёмки: тема в три уровня (ADR-0038)

ТЗ задачи — `docs/spec/2026-11-23-theme-roles.md`, §4. Второй проход (BP-09): каждый критерий сверен с кодом, тестом или сохранённым выводом команды.

| № | Критерий | Статус | Подтверждение |
|---|---|---|---|
| К1 | Основа: запасные ступени акцента; без `ui.background` или `ui.text` вывода нет | ✅ | `theme/derive.rs` `base_colors`; `nothing_is_derived_…`, `light_theme_…fallback_steps`, `dim_and_error_…`; мутации «акцент пропускает `ui.text.focus`», «вывод работает без `ui.text`» |
| К2 | Тона по формулам для тёмной и светлой темы; заданный тон не заменяется | ✅ | `dark_tones_follow_the_formulas`, `light_theme_…`, `darkness_boundary_and_channel_saturation` — значения посчитаны вручную; шесть мутаций формул |
| К3 | Области виджетов по ссылкам; правка тона меняет ссылающиеся области; заданная область не заменяется; `is_derived` верен | ✅ | `WIDGET_LINKS` (23 области); `widget_scopes_are_links_to_tones`, `a_tone_given_…`, `a_widget_scope_given_…`, `derive_is_repeatable_…` |
| К3а | Тема, описанная вручную, не содержит выведенных цветов | ✅ | `a_theme_described_by_hand_gets_no_computed_color` |
| К4 | Порядок: явный стиль → область темы → прежняя цепочка (только без основы) | ✅ | `buttons_of_a_base_only_theme_…` (кадр дерева); прежняя цепочка — `theme_without_widget_scopes_falls_back_to_helix_scopes` в `theme_widgets.rs`, не правился |
| К5 | Тема без основы и существующие тесты — без изменений поведения | ✅ | 348 прежних тестов зелёные, ни один не правился (`git status`: в `tests/` только новый `theme_derive.rs`) |
| К6 | Темы sniffnet в `examples/themes/`, без предупреждений, источник и лицензия указаны | ✅ | 22 файла `sniffnet_*.toml`; `files::every_example_theme_loads_clean_and_is_complete`; `examples/themes/README.md`; цвета сверены с клоном командой — 10/10 файлов и 6/6 модулей |
| К7 | Пример `theme` помечает выведенные области; вид проверен по снимкам | ✅ | `examples/theme/scopes.rs` (знак `≈`), статус «≈ выведено N»; снимки `target/shots/roles_*.png`: `sniffnet_nord_dark`, `sniffnet_light_bubbles`, `gterm_dark`, `gruvbox`, `catppuccin_latte` — по два |
| К8 | Все темы каталога helix загружаются | ✅ | `every_helix_theme_loads`: тем 116, ошибок 0 |
| К9 | Гейты, размеры release, документы, ADR-0038 | ✅ | fmt, clippy `--all-targets -D warnings` и без фич по умолчанию, 363 теста; размеры в `docs/done/…`; ADR-0038 + индекс, ТЗ 1.31, `AGENTS.md`, `PROJECT_MAP.md`, README, `TODO.md` |

**Контракты §3.** Формат файла не менялся — правок разбора в `load.rs` нет, добавлен один вызов `derive()`. «Заданное не заменяется» — `put` и мутация «выведенное заменяет заданное» (10 падений). «Свои темы выглядят как сейчас» — снимок `roles_gterm_dark_1.png` и тест встроенной темы: недостающие области равны тонам прежних цепочек. «`gterm` не меняется» — в `crates/gterm` правок нет. «Арифметика целочисленная» — `derive.rs`, `f32` нет.

**Расхождения с первой редакцией ТЗ** (внесены в ТЗ как уточнения при планировании, до кода):
- пять ссылок уровня 3 повторяют действующие цепочки ADR-0037 (`ui.checkbox.focus`, `ui.tabs.focus` ← `ui.text.focus` целиком; `ui.input.selection` ← `ui.selection`; `ui.combo*` ← области поля и списка; `ui.checkbox.disabled`, `ui.input.placeholder` ← `ui.text.inactive` целиком) — иначе изменился бы вид своих тем;
- состояния `ui.control.*` считаются от тона `ui.control` (заданного или выведенного), а не от сдвига фона;
- коэффициенты смесей остались начальными: по снимкам править не пришлось.

**Замечания к виду (не критерии, вынесены в ⚠ WARNINGS отчёта о выполнении).** В темах helix без своего акцента (`gruvbox`, `catppuccin_latte`) акцент равен цвету текста под фокусом, и кнопка под фокусом отличается слабо; заданные в чужой теме `ui.selection`, `ui.window` берутся как есть.
