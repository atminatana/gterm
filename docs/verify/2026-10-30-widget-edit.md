# Проверка критериев приёмки: поле ввода и выпадающий список (ADR-0027)

ТЗ задачи — `docs/spec/2026-10-30-widget-edit.md`, §4. Второй проход (BP-09): каждый критерий сверен с кодом, тестом или сохранённым выводом команды.

| № | Критерий | Статус | Подтверждение |
|---|---|---|---|
| К1 | `Ui::text` доставляет текст фокусу (экрана или слоя), иначе — приложению; `Control::text` по умолчанию «нет»; обычный набор из `KeyEvent.text` непринятой клавиши — тоже контролу (исправление по проверке владельца) | ✅ | `tests/route.rs::text_goes_to_the_focused_control…`, `text_without_focus_candidates…`; `tests/modal.rs::text_goes_to_the_layer_focus…`; `tests/text_input.rs::typed_text_of_an_unhandled_key…`; `control.rs` — умолчание `Handled::No` |
| К2 | Поле: вставка в курсор, `Backspace`/`Delete`, стрелки/`Home`/`End`, `Enter` → `Submitted`, `Changed` только при изменении, управляющие символы отброшены, инвариант курсора | ✅ | `tests/text_input.rs::text_is_inserted_at_the_cursor…`; мутация «без фильтра» → падение |
| К3 | Поле: прокрутка — курсор виден слева и справа; клик ставит курсор; курсор обменом fg/bg только под фокусом; `placeholder` и `focus_style` | ✅ | `tests/text_input.rs::scrolling_keeps_the_cursor_visible…`, `click_puts_the_cursor…`, `placeholder_focus_style_block_and_disabled_field` |
| К4 | Список: раскрытие `Enter`/пробелом/`↓`/кликом, слой под полем (над — при нехватке), выбор `Enter`/кликом закрывает и даёт `Selected`, `Esc`/клик мимо закрывают, маркер `▾`/`▴` | ✅ с уточнением | `tests/combo_box.rs` (5). Фокус после выбора остаётся на поле (исправление 2 по проверке владельца: `render` при открытом слое не трогает фокус экрана; мутация без условия → падение). Уточнение: закрытие по `Esc`/клику мимо делает сам список без сообщения `on_close` (ТЗ п. 3; причина — два `&mut` на состояние), статус ТЗ и ADR-0027 обновлены, подтверждено владельцем 2026-10-30 |
| К5 | Слой: `Modal::at`, `on_click_outside` один раз на левое нажатие вне области, без привязки — прежнее поглощение | ✅ | `tests/modal.rs::click_outside_publishes_the_binding_once…`; дополнительно `Modal::below` — `tests/combo_box.rs` (под/над полем) |
| К6 | Пример `widget_edit`: три поля, список, две кнопки, статус; `Tab` по кольцу, `Enter` в поле — «Применить», «Сброс» очищает, `Esc` — выход/закрыть список; на трёх бэкендах | 🔶 | код `examples/widget_edit.rs`; запуск `--backend=soft --fail-frames=2` — кадр, повтор, код 1 по ТЗ; GL/GLES и глазами — за владельцем |
| К7 | `gterm` не изменён; тесты части 1 и ADR-0026 без изменения ожиданий | ✅ | `git diff --stat crates/gterm` пуст; `tests/widgets.rs` 6/6, `tests/popup.rs` 2/2, `controls`/`focus`/`tree_render` без правок ожиданий; `route.rs`/`modal.rs` только дополнены |
| К8 | Гейты, размеры release, документы | ✅ | fmt, clippy `--all-targets -D warnings`, 251 тест; размеры в `docs/done/…`; ADR-0027 + индекс, ТЗ 1.22 §13, `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md`, отчёты |

**След прохода.** Открыто заново 6 мест: вывод полного `cargo test` (35 бинарников + 4 оконных), сводка размеров release, `wc -l`, `git diff --stat crates/gterm`, `git status` (23 записи), результаты двух мутаций и `grep` восстановленных строк в `text_input.rs`/`combo_box.rs`.

Отброшенных находок нет. Расхождение с ТЗ одно — К4/п. 3 (закрытие списка без сообщения, `Ui::combo_box`), вынесено владельцу.
