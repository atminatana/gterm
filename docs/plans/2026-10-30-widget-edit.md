# План: поле ввода и выпадающий список в `gterm-widgets`, пример `widget_edit`

Статус: **выполнен 2026-10-30** — см. `docs/done/2026-10-30-widget-edit.md`. Отклонения: п. 4 решений — слой списка строит `Ui::combo_box` (поле снимком, состояние у слоя через новый `Ui::owned`), закрытие без сообщения приложению (`Modal::dismiss_on_click_outside`, `Control::dismiss`), `Modal::below` вместо `Modal::at` для списков; программные `set_value`/`clear` без события `Changed`; модальный слой дочернего описания поднимается к корню. Исправление после проверки владельца: факт по коду — обычный набор `winit` отдаёт в `KeyEvent.text`, не `Ime::Commit`; дерево доставляет текст непринятой клавиши контролу (`KeyMsg::typed_text`), `Ui::text` — для IME. Исправление 2: `Ui::render` при открытом слое не трогает фокус экрана — иначе фокус уходил с поля раскрытого списка (снимок без узла) на первый кандидат.

- **ТЗ задачи:** `docs/spec/2026-10-30-widget-edit.md` (утверждено 2026-10-30).
- **Решения:** ADR-0027 (`docs/adr/0027-text-input-combo-box.md`).
- **Опирается на:** ADR-0026 (контролы, `Ui`, `Modal`, `WidgetEvents`), ADR-0023, ADR-0004/ТЗ §13 (курсор на стороне виджета).

## Факты по коду (проверено 2026-10-30)

- `gterm`: IME включён (`app/chain.rs:107`), текст — `Ime::Commit` → `on_text` (`events.rs:44`); `KeyEvent.text` для ввода не используется (демо `demo/main.rs:197-212` собирает строку из `on_text`, `Backspace` — из `on_key`).
- `gterm-widgets`: `Control<M>` (`node`, `focusable`, `key`, `mouse`), `WidgetEvent::{Selected, Activated}`, `Stateful::emit` сопоставляет события привязкам (`ui/element.rs`); `Modal` — `centered`, `style`, `on_escape`, `content`, `route_key/route_mouse` (мышь поглощается целиком), `render` через `Popup` (`ui/modal.rs`); `Ui::modal`, `draw_modal`, `modal_buffer`; `selection.rs` — `RowMap`, `navigate`, `wheel`; `ListState` — контрол со строками; `Button` — образец виджета с состоянием; `BufferExt::set_line(Rect, &StyledLine, scroll)`, `set_styled`, `set_style`.
- `Rect::centered`, `Rect::intersection`, `Rect::contains`; `Block::inner`, `patch_border_style` (crate-private).

## Проектные решения (зафиксированы в ADR-0027)

1. **Текст — третий вход контрола.** `Control::text(&mut self, &str, &mut Messages<M>) -> Handled` (по умолчанию `No`); `Element::text`; `Ui::text(&str) -> Routed<M>` (слой → его фокус; иначе фокус экрана; без фокуса — `ignored`). Для `Stateful` после `text` вызывается `emit`.
2. **События с данными.** `WidgetEvent::Changed(String)`, `Submitted(String)`; привязки `on_change`/`on_submit` (`Box<dyn Fn(String) -> M + 'a>`) в `Stateful` и `Ui`; `Element::bind_change`/`bind_submit`.
3. **`TextInputState`** (`widgets/text_input.rs`): `value: String`, `cursor: usize` (символы), `scroll: usize`, `node`, `enabled`, `event`. Операции: `insert_str`, `backspace`, `delete`, `move_left/right/home/end`, `set_value`, `clear`, `cursor_to_col(col)`; `key` — стрелки/`Home`/`End`/`Backspace`/`Delete`/`Enter`; `text` — фильтр `ch.is_control()`, вставка; `mouse` — левое нажатие → курсор; `focusable = enabled`. Виджет `TextInput<'a>`: `style`, `focus_style`, `placeholder(Line)`, `placeholder_style`, `block`; `render` подгоняет `scroll`, рисует символы с `scroll`, курсор обменом fg/bg (ячейка под курсором или пустая за концом) только при `node.focused`.
4. **`ComboBoxState`** (`widgets/combo_box.rs`): `selected`, `open`, `list: ListState`, `node`, `enabled`, `event`; `key`: `Enter`/пробел/`↓` → `open = true`, `list.selected = selected`; `mouse`: левое нажатие → то же; `dropdown(&mut self, items: &'a [&'a str], height: u16, on_select: impl Fn(usize) -> M + 'a) -> Modal<'a, M>` — слой `Modal::at(area)` под `node.area` (над — если снизу не хватает), рамка `Block::bordered()`, содержимое — `Ui::control(DropdownList { state, items, on_select })`, где `DropdownList` — внутренний контрол: клавиши навигации через `selection::navigate` по `list`, колесо, `Enter`/левое нажатие по строке → `selected`, `open = false`, публикация `on_select(i)` при изменении; рисует `List` с `highlight_style`. Закрытие по `Esc`/клику мимо — `Modal::on_close(M)` (`M: Clone`), ставящий `on_escape` и `on_click_outside`. Виджет `ComboBox<'a>`: `items`, `style`, `focus_style`, `placeholder`, `block`; `render` — значение и маркер `▾`/`▴` в последней ячейке.
5. **`Modal`:** `at(area)`, поле `placement: Placement::{Centered(w, h), At(Rect)}`; `on_click_outside(M)`; `on_close(M) where M: Clone`; `route_mouse` — при левом нажатии вне области публикует `on_click_outside.take()`.
6. **Пример** `examples/widget_edit.rs`: `Form { name, path, size: TextInputState, kind: ComboBoxState, apply, reset: ButtonState, status: String, applied: String }`, `enum Msg { Changed(&'static str, String)?` — проще: `NameChanged(String)`, `PathChanged(String)`, `SizeChanged(String)`, `Submit`, `KindPicked(usize)`, `KindClose`, `Reset` }`; `ui()`: группа-рамка по центру (`Ui::split` c `Fill`/`Length`/`Fill` по горизонтали и вертикали), строки «подпись + поле» (`Ui::split(horizontal [Length(10), Fill])`), список, строка кнопок, статус внизу экрана; при `kind.open` — `.modal(kind.dropdown(KINDS, 6, Msg::KindPicked).on_close(Msg::KindClose))`.

## Шаги

1. **Ввод текста и привязки.** `control.rs` (`text`, `WidgetEvent::{Changed, Submitted}`), `ui/element.rs` (`text`, `bind_change`, `bind_submit`, `emit`), `ui/mod.rs` (`on_change`, `on_submit`), `ui/route.rs` (`Ui::text`), `ui/modal.rs` (`route_text`, `at`, `Placement`, `on_click_outside`, `on_close`). Тесты: `tests/route.rs` (+2: текст фокусу/«не обработано»; заглушка `Stub` получает `text`), `tests/modal.rs` (+2: `at`; `on_click_outside` один раз, движение/колесо — нет; текст слою). Проверка: `cargo test -p gterm-widgets`.
2. **`TextInput`.** `widgets/text_input.rs`, экспорт; `tests/text_input.rs` по К2/К3. Проверка: `cargo test -p gterm-widgets`.
3. **`ComboBox`.** `widgets/combo_box.rs`, экспорт; `tests/combo_box.rs` по К4. Проверка: `cargo test -p gterm-widgets`.
4. **Пример.** `examples/widget_edit.rs`, `[[example]]` в `Cargo.toml`; запуск `--backend=soft --fail-frames=2`; `cargo clippy --all-targets -- -D warnings`; release-размеры (`widget_edit.exe`, `widgets.exe`, `demo.exe`).
5. **Документы и гейты.** ADR-0027 (сверить с кодом) + индекс, ТЗ 1.22 §13, `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md`, отчёты `docs/done|test|verify/2026-10-30-widget-edit.md`; `cargo fmt --check`, полный `cargo test`.

## Открытые решения

Нет.

## Риски

- `Modal::on_close` требует `M: Clone` — у `Ui`/`Modal` в целом ограничения нет, только у метода.
- Рамка слоя списка съедает две строки: `height` — высота содержимого, область слоя `height + 2`.
- Двойной ввод при символьных клавишах исключён тем, что `TextInputState::key` не обрабатывает `Key::Character`; тест на `key` с символом → «не обработано».
