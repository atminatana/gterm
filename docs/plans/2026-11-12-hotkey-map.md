# План: карта «сочетание → сообщение» в описании экрана, событие фокуса окна

Статус: **выполнен 2026-11-12** — см. `docs/done/2026-11-12-hotkey-map.md`. Отклонений от плана нет; указатель `fn(&M) -> M` в карте остался (запасной вариант из рисков не понадобился).

- **ТЗ задачи:** `docs/spec/2026-11-12-hotkey-map.md` (утверждено 2026-11-12; О1–О3 — рекомендованные варианты).
- **Решения:** ADR-0029 (`docs/adr/0029-hotkey-map.md`).
- **Опирается на:** ADR-0028 (`gterm::hotkey`), ADR-0026/0027 (`Ui`, `Modal`, `Routed`, текст — вход контрола), ADR-0004.

## Факты по коду (проверено 2026-11-12)

- `gterm-widgets/src/ui/route.rs:17` — `Ui::key`: `Released` → `Routed::ignored()`, иначе `route_key(&KeyMsg)`; `route_key` (`:28`): слой → `Tab` → контрол под фокусом → `typed_text` → `Control::text`.
- `ui/mod.rs:29` — `Ui { nodes, areas, modal }`; `container` (`:76`) поднимает `child.modal` к корню («первый побеждает»). Описание строится заново на каждое событие и кадр.
- `ui/modal.rs:125` — `Modal::route_key`: содержимое, затем `on_escape` на непринятый `Esc`.
- `gterm/src/hotkey.rs:570` — `HotkeyTracker::feed_key(key, state, repeat)`: автоповтор и нажатие уже удержанной клавиши игнорируются (`press`, `:584`); `full()`, `current()` отдают канонический `Hotkey`; `Hotkey::matches(&self, pattern)`, `Hotkey::keys()`.
- `gterm/src/app/events.rs:21` — ветки `WindowEvent::Focused` нет (уходит в `_ => {}`); `api.rs:135` — `AppHandler`, все методы с телом по умолчанию.
- Примеры: `widget_edit.rs:298` и `widgets.rs:321` — `on_key` выходит на `Released` до `ui().key`; `widgets.rs:334` — окно по `"p" | "з"` из непринятого; `Msg` обоих примеров — `Clone`. `gterm/examples/hotkeys.rs:212` — `on_key` кормит трекер.
- Тесты `gterm-widgets/tests/`: `common/mod.rs` (`Stub`, `Msg`, `key_msg`), `route.rs`, `modal.rs`, `text_input.rs:246` (набор через `route_key` с `KeyMsg { text }`).

## Проектные решения (зафиксированы в ADR-0029)

1. **`HotkeyMap<M>`** (`gterm-widgets/src/hotkeys.rs`, реэкспорт из `lib.rs`): `bindings: Vec<(Hotkey, M)>` и указатель `clone: fn(&M) -> M` (берётся в `new()` под `M: Clone`, чтобы маршрутизация `Ui` не требовала `M: Clone` у тех, кто карту не подключает). `new`, `bind(Hotkey, M)`, `bind_str(&str, M) -> Result<Self, ParseError>` (строитель по значению), `len`, `is_empty`, `message_for(full: &Hotkey, current: &Hotkey) -> Option<M>` — первая привязка, совпавшая с `full`, иначе первая, совпавшая с `current`.
2. **`Ui`**: поле `hotkeys: Option<{ tracker: &'a mut HotkeyTracker, map: &'a HotkeyMap<M> }>`; `Ui::hotkeys(self, tracker, map)`; `container` поднимает `child.hotkeys` к корню, как слой. **`Modal`**: поле `hotkeys: Option<&'a HotkeyMap<M>>`, `Modal::hotkeys(self, map)`.
3. **Маршрутизация** (`ui/route.rs`): `Ui::key(event, ctx)` → `route_key_event(Key::from_event(event), event.state, event.repeat, &KeyMsg)` (точка входа тестов без окна). Порядок: «учтено ли нажатие» (`key` есть, не автоповтор, клавиши нет среди `tracker.current().keys()`) → `tracker.feed_key` → `Released` → `ignored` → `route_key(msg)` (без изменений) → если `handled == No` и нажатие учтено: карта слоя при слое, иначе карта экрана → `message_for(full, current)` → сообщение в `routed.messages`, `handled = Yes`.
4. **`gterm`**: `AppHandler::on_focus(&mut self, _focused: bool, _ctx: &mut Ctx<'_>) {}`; `events.rs` — `WindowEvent::Focused(focused)` → `notify` + `apply_request`.
5. **Примеры**: `widget_edit` — поля `tracker`, `keys` (`Ctrl+S` → `Apply`, `Ctrl+R` → `Reset`), `on_key` передаёт в `ui().key` оба состояния, `on_focus(false)` → `reset()`; `widgets` — карта экрана `P` → `OpenDialog`, карта слоя `P` → `CloseDialog`, разбор `"p" | "з"` убирается; `hotkeys` — `on_focus(false)` → `reset()` и перерисовка.

## Шаги

1. **Карта.** `src/hotkeys.rs`, `lib.rs`. Проверка: `cargo build -p gterm-widgets`.
2. **Подключение и маршрутизация.** `ui/mod.rs`, `ui/modal.rs`, `ui/route.rs`. Проверка — вместе с шагом 3 (одна логическая единица, QG-03).
3. **Тесты** `gterm-widgets/tests/hotkeys.rs` (К1–К5): карта и ошибка разбора; простое сочетание на нажатие, повтор без отпускания модификатора, отпускание и автоповтор молчат; аккорд, приоритет полной истории, альтернативы; контрол раньше карты (`TextInput`, кнопка, `Ctrl+S` в поле); слой — карта экрана молчит, карта слоя работает, `on_escape` раньше, трекер кормится. Проверка: `cargo test -p gterm-widgets`.
4. **Фокус окна.** `api.rs`, `events.rs`. Проверка: `cargo test -p gterm` (оконные бинарники).
5. **Примеры.** `widget_edit.rs`, `widgets.rs`, `hotkeys.rs`; сборка, запуск с `--backend=soft --fail-frames=2` (код 1 после повтора кадра — пример стартует и рисует), clippy по всем целям.
6. **Документы и гейты.** ТЗ проекта 1.24 (§13, FR-25), `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md`, отчёты `docs/done|test|verify/2026-11-12-hotkey-map.md`; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, полный `cargo test`; размеры release.

## Открытые решения

Нет.

## Риски

- `KeyEvent` без окна не сконструировать: `Ui::key` и `on_focus` проверяются чтением кода и глазами; логика — через `route_key_event`.
- Указатель `fn(&M) -> M` в карте — способ не распространять `M: Clone` на весь `Ui`; если окажется неудобным — вернуться к ограничению только на `Ui::hotkeys`/`Modal::hotkeys` с тем же указателем в описании.
- После правки примеров `on_key` получает отпускания в `ui().key`: перерисовка на каждое отпускание — лишний кадр, не дефект.
