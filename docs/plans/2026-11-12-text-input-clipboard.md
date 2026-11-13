# План: выделение текста и буфер обмена в `TextInput`

Статус: **выполнен 2026-11-12** — см. `docs/done/2026-11-12-text-input-clipboard.md`. Уточнения при реализации: в трейт `Clipboard` добавлен `is_available` — вырезание без подключённого или при недоступном буфере текст не удаляет; три теста `tests/hotkeys.rs` прошлой задачи шли с фокусом в поле, а не на заглушке (`focus_next` без фокуса даёт первый контрол) — фокус теперь ставится явно и проверяется.

- **ТЗ задачи:** `docs/spec/2026-11-12-text-input-clipboard.md` (утверждено 2026-11-12; О1–О4 — варианты (a); `Ctrl+A` оставлен — рекомендация принята вместе с ТЗ).
- **Решения:** ADR-0030 (`docs/adr/0030-text-selection-clipboard.md`).
- **Опирается на:** ADR-0027 (`TextInput`), ADR-0026 (`Control`, захват), ADR-0029 (`route_key_event`).

## Факты по коду и зависимостям (проверено 2026-11-12)

- `widgets/text_input.rs`: `TextInputState { value, cursor, scroll, node, enabled, text_x, event }`; `key` (`:181`) — стрелки, `Home`/`End`, `Backspace`, `Delete`, `Enter`; `mouse` (`:205`) — левое нажатие ставит курсор, `Move` не обрабатывается; `insert_str` отбрасывает управляющие символы; курсор — обмен fg/bg ячейки (`:326`).
- `control.rs:29` — `Control<M>`: `key`, `mouse`, `text`, `dismiss` с телами по умолчанию. `ui/element.rs` — обёртки `Custom`, `Owned`, `Stateful` (после обработчика `emit` превращает `WidgetEvent` в сообщение), `Stateless`.
- `ui/route.rs:33` — `route_key_event(key, state, repeat, msg)`: трекер → `Released` → `route_key` → карта. `ui/modal.rs:128` — `Modal::route_key` передаёт содержимому только `KeyMsg`.
- `arboard 3.7.0`: фича по умолчанию `image-data` (отключается), `wayland-data-control` — отдельная. `cargo tree -F arboard`: Windows — `clipboard-win 5.4.3`, `error-code`, `log`, `windows-sys 0.61` (уже в lock); Linux — `x11rb 0.13.2` (уже в lock), `parking_lot` (+ `lock_api`, `scopeguard`, `parking_lot_core`), `percent-encoding`, `log`. В `Cargo.lock` добавилось 7 пакетов.

## Проектные решения (зафиксированы в ADR-0030)

1. **`src/clipboard.rs`**: трейт `Clipboard { get, set }`; `MemoryClipboard`; `EditCommand { Copy, Cut, Paste, SelectAll }` с `from_key(Key, ModifiersState)`; внутренний пустой буфер для описания без подключения; `SystemClipboard` под `#[cfg(feature = "system-clipboard")]` (ленивое создание `arboard::Clipboard`, ошибка — один раз в stderr; «буфер пуст» ошибкой не считается).
2. **`Control::edit(&mut self, command, clipboard, out) -> Handled`** с телом по умолчанию `No`; `Element::edit` во всех обёртках (`Stateful` — с `emit`).
3. **`Ui`**: поле `clipboard: Option<&'a mut dyn Clipboard>`, `Ui::clipboard(…)`, подъём к корню в `container`. `route_key_event`: после трекера и проверки `Released` — `EditCommand::from_key` → `route_edit` (слой — его содержимому тем же буфером; иначе контрол под фокусом); `Yes` — возврат, `No` — прежний путь. Автоповтор команду вызывает (удержанный `Ctrl+V` вставляет повторно).
4. **`TextInputState`**: поле `anchor: Option<usize>`, `text_width` (для прокрутки при протяжке); `selection`, `selected_text`, `select_all`, `clear_selection`, `delete_selection`, `paste_str` (нормализация перевода строки и табуляции); движение со `Shift` расширяет, без — снимает; `mouse`: `Down` — якорь (`Shift` — расширение), `Move` под захватом — расширение. `TextInput::selection_style`.
5. **`Cargo.toml` `gterm-widgets`**: `arboard` optional, `[features] default = ["system-clipboard"]`, `system-clipboard = ["dep:arboard"]`; пример `widget_edit` — `required-features = ["system-clipboard"]`.

## Шаги

1. **Буфер и команды.** `clipboard.rs`, `Cargo.toml`, `lib.rs`, `Control::edit`. Проверка: `cargo build -p gterm-widgets` и `--no-default-features`.
2. **Маршрутизация.** `ui/element.rs`, `ui/mod.rs`, `ui/modal.rs`, `ui/route.rs`.
3. **Поле.** `widgets/text_input.rs`: выделение, клавиши, мышь, команды, отрисовка.
4. **Тесты.** `tests/text_selection.rs` (К1–К3, К6), `tests/clipboard.rs` (К4, К5). Проверка: `cargo test -p gterm-widgets` (шаги 2–4 — одна логическая единица, QG-03).
5. **Пример.** `widget_edit.rs` — `SystemClipboard`, подсказка; запуск `--backend=soft --fail-frames=2`; clippy по всем целям.
6. **Документы и гейты.** ТЗ проекта 1.25 (§13), `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md`, отчёты `docs/done|test|verify/`; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, полный `cargo test`; размеры release.

## Открытые решения

Нет.

## Риски

- API `arboard` (`Clipboard::new`, `get_text`, `set_text`, `Error::ContentNotAvailable`) — сверяется компиляцией на шаге 1.
- Системный буфер автотестом не проверяется; Linux (X11, Wayland через XWayland) — глазами на машине владельца.
- Существующие тесты `text_input.rs` фиксируют поведение без выделения — должны остаться зелёными без правок.
