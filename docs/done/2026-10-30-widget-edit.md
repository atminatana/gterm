# Отчёт о выполнении: поле ввода и выпадающий список в `gterm-widgets`, пример `widget_edit`

- **Задача:** `2026-10-30-widget-edit`. ТЗ — `docs/spec/2026-10-30-widget-edit.md` (утверждено 2026-10-30), план — `docs/plans/2026-10-30-widget-edit.md`.
- **Решение и его ПОЧЕМУ:** ADR-0027.

## Что сделано

| Шаг плана | Результат |
|---|---|
| 1. Ввод текста и привязки | `control.rs`: `Control::text`, `Control::dismiss`, `WidgetEvent::{Changed, Submitted}` (`WidgetEvent` теряет `Copy`); `ui/element.rs`: `text`, `dismiss`, `bind_change`/`bind_submit`, новый элемент `Owned` (контрол, которым владеет описание); `ui/mod.rs`: `Ui::text`, `on_change`/`on_submit`, `Ui::owned`, подъём модального слоя дочернего описания к корню, `dismiss_all`; `ui/modal.rs`: `Placement::{Centered, At, Below}`, `Modal::at`/`below`, `on_click_outside`, `on_close` (`M: Clone`), `dismiss_on_click_outside`, `route_text`. Тесты: `route.rs` (+2), `modal.rs` (+2) |
| 2. `TextInput` | `widgets/text_input.rs`: `TextInputState` (значение, курсор в символах, прокрутка, `enabled`, операции `insert_str`/`backspace`/`delete`/`move_*`/`cursor_to_col`/`set_value`/`clear`), `Control` (стрелки, `Home`/`End`, `Backspace`, `Delete`, `Enter` → `Submitted`; символьные клавиши — «не обработано»; `text` → вставка без управляющих символов → `Changed`; клик ставит курсор), виджет `TextInput` (`style`, `focus_style`, `placeholder`+`placeholder_style`, `block`, прокрутка держит курсор видимым, курсор обменом fg/bg только под фокусом). Тесты: `tests/text_input.rs` (5) |
| 3. `ComboBox` | `widgets/combo_box.rs`: `ComboBoxState` (`selected`, `open`, `list`, `enabled`, узел списка), `Control` закрытого поля (`Enter`/пробел/`↓`/клик раскрывают), внутренний контрол `DropdownList` (навигация, колесо, `Enter`/клик по строке выбирают и закрывают, `Esc` закрывает, `dismiss` закрывает), виджет `ComboBox` (`items`, `style`, `focus_style`, `highlight_style`, `placeholder`, `block`, `rows`; маркер `▾`/`▴`), снимок `ComboBoxView` на время раскрытия, `Ui::combo_box(widget, &mut state, on_select)` — поле плюс слой `Modal::below(anchor, rows + 2)` с рамкой. Тесты: `tests/combo_box.rs` (5) |
| 4. Пример | `examples/widget_edit.rs` + `[[example]]`: форма по центру — «Имя», «Путь», «Размер», список «Тип», кнопки «Применить»/«Сброс», подсказка и строка статуса; `enum Msg`, `update`, `on_text` → `Ui::text`; запуск с инжекцией — код 1 после повтора кадра |
| 5. Документы | ADR-0027 (уточнён по реализации) + индекс; ТЗ 1.22 (§13); `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md`; отчёты `test`/`verify`; размеры release |
| Исправление после проверки владельцем | Симптом: текст в поле не набирается. Цепочка: `winit` → `events.rs` → `Form::on_text` → `Ui::text` → поле. Разрыв: `winit` → `events.rs` — на Windows `Ime::Commit` приходит только при IME-композиции (`event_loop.rs:1527-1598`), обычный набор идёт в `KeyEvent.text` (`keyboard.rs:467-630`) и попадал в `on_key`, где поле намеренно игнорировало символьные клавиши; на X11 то же (`event_processor.rs:968-1010`). Причина — неверное допущение ADR-0027 о едином пути IME. Исправление: `KeyMsg::typed_text` (текст без `Ctrl`/`Alt`/`Super` и с печатным символом) и доставка его контролу в `Ui::route_key`, если клавиша не принята; `Ui::text` остаётся для IME. Тест `text_input.rs::typed_text_of_an_unhandled_key…` (набор, Shift, Ctrl/Alt/Super — нет, Esc и Enter не глотаются) |
| Исправление 2 после проверки владельцем | Симптом: после выбора в списке фокус уходит на первый контрол. Разрыв: `Ui::render` → `ensure_focus` при открытом слое — поле списка в это время снимок без узла, фокус переназначался первому кандидату; после закрытия флаг поднят у двух контролов, побеждает первый по порядку. Исправление: при открытом слое `render` фокус экрана не трогает (контракт ADR-0026). Регрессия — `tests/combo_box.rs::click_on_a_row…` (фокус на поле до и после выбора); мутация без условия → падение |

**Отклонения от ТЗ** (зафиксированы в статусе ТЗ, плане и ADR-0027; требуют подтверждения владельца): п. 3 ТЗ предполагал `ComboBoxState::dropdown(..)` и закрытие по `Esc`/клику мимо через сообщение приложения `on_close`. При реализации выяснилось, что поле (`Ui::stateful(.., &mut state)`) и слой (`dropdown(&mut state)`) не могут одновременно держать `&mut` на одно состояние — приложение так написать не сможет. Поэтому поле и слой строит один вызов `Ui::combo_box`, состояние на время раскрытия держит слой, а закрытие список делает сам (`Modal::dismiss_on_click_outside` → `Control::dismiss`, `Esc` в контроле списка). `Modal::on_click_outside`/`on_close` при этом оставлены как общие средства слоя (К5). Программные `set_value`/`clear` событий `Changed` не дают — иначе «Сброс» в форме давал бы ложное сообщение при следующем вводе.

## COMPLETENESS
- К1–К5 — код и тесты без окна: `tests/{text_input,combo_box}.rs` (10), дополнения `route.rs`/`modal.rs` (4).
- К6 — пример написан и собран; запуск `--backend=soft --fail-frames=2` прошёл по ТЗ; глазами на трёх бэкендах — за владельцем.
- К7 — `git diff --stat crates/gterm` пуст; тесты части 1 и ADR-0026 без изменения ожиданий.
- К8 — гейты и документы ниже.

## CORRECTNESS
- Мутации (исходник восстановлен, прогон после восстановления зелёный): фильтр управляющих символов в `insert_str` убран → падение `tests/text_input.rs`; `dismiss` списка не закрывает → падение `tests/combo_box.rs`.
- Тесты модального слоя и поля ловили реальные ошибки по ходу (борьба за `&mut` состояние списка, снимок без последней строки) — исправлены до зелёного прогона.

## EVIDENCE
- Базовый прогон до правок — коммит `5fa0b3f`: 236 тестов, 0 падений.
- После последнего изменения кода: `cargo fmt --check` чисто; `cargo clippy --all-targets -- -D warnings` чисто; `cargo test` — **251 тест, 0 падений** в 35 бинарниках + 4 оконных бинарника `ok`. `gterm-widgets`: 63 теста.
- Размер release 2026-10-30: `gterm-widgets/examples/widget_edit.exe` 1 208 320 Б (новый), `widgets.exe` 1 207 808 Б (+1 536), `demo.exe` 1 544 192 Б (без изменений — `gterm` не тронут).
- LOC затронутых файлов (`wc -l`): `combo_box.rs` 408, `text_input.rs` 337, `examples/widget_edit.rs` 332, `ui/mod.rs` 289, `ui/element.rs` 271, `ui/modal.rs` 225, `ui/route.rs` 136, `control.rs` 88; тесты 204–311. Все ниже ориентиров QG-06.
- Проверочный проход BP-09: заново открыты 6 мест — вывод полного `cargo test`, сводка release, `wc -l`, `git diff --stat crates/gterm`, `git status`, результаты двух мутаций с `grep` восстановленных строк. Отброшенных находок нет.

## ⚠ WARNINGS
- Отклонение от ТЗ п. 3 (закрытие списка без сообщения приложению, `Ui::combo_box` вместо `dropdown`) подтверждено владельцем 2026-10-30 после проверки глазами (ввод, список, фокус, маркер `▾`).
- Пока список раскрыт, поле — снимок: его область в состоянии не обновляется, при смене размера окна с раскрытым списком слой остаётся на прежнем месте до закрытия.
- Глазами — за владельцем: `cargo run -p gterm-widgets --example widget_edit` на GL/GLES/Software после исправления ввода; ввод через IME автоматически не проверяется (оконные тесты не инжектируют ввод). Первая проверка владельца: список работает, текст не набирался — исправлено (см. таблицу).
- Вне задачи: строка ввода демо `crates/gterm-demo/src/bin/demo/main.rs:197-212` собирает текст только из `on_text` — на Windows и X11 при обычном наборе она тоже не получает символов (тот же разрыв). Предлагаю отдельную задачу.
- `ListState`/`TableState`-подобные поля состояний (`TextInputState`, `ComboBoxState`) имеют приватные поля — литеральная конструкция в чужом коде недоступна, конструкторы `new`/`with_*`.
- Коммит не делался.
