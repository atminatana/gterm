# Отчёт о выполнении: контролы с фокусом, маршрутизацией ввода и сообщениями в `gterm-widgets`

- **Задача:** `2026-10-30-widget-controls`. ТЗ — `docs/spec/2026-10-30-widget-controls.md` (утверждено 2026-10-30), план — `docs/plans/2026-10-30-widget-controls.md`, исследование — `docs/research/2026-10-30-controls-interview.md`.
- **Решение и его ПОЧЕМУ:** ADR-0026.

## Что сделано

| Шаг плана | Результат |
|---|---|
| 1. `gterm`: ячейка и модификаторы | `app/api.rs`: `PointerEvent::{Moved, Input, Wheel}.cell: (i32, i32)`, `Ctx::{modifiers, cell_px}`; `app/options.rs`: `pointer_cell(position, origin_px, cell_px)` — ячейка с учётом центрирования сетки (`centered_origin`); `app/window.rs`: `cell_px_u32`, `App::pointer_cell` (размер окна из `inner_size`); `app/events.rs`: `WindowEvent::ModifiersChanged` → `App.modifiers`, ячейка в трёх событиях, `Ctx` заполняется в `notify`; образцы в `demo/main.rs` и `full_table/main.rs` дополнены `..`. Тесты: `pointer_cell` в `stage6_app`, `Log::{cell_px, modifiers_empty}` в `events_common`, проверка в `stage6_events` |
| 2. Ввод и контрол | `gterm-widgets/src/input.rs` (`KeyMsg`, `MouseMsg`/`MouseKind` с `from_pointer`, `translated`, `Handled`, `Messages<M>`, `Routed<M>`), `src/control.rs` (`NodeState`, `Control<M>`, `Drawable`, `WidgetEvent`, `WidgetEvents`); `winit` — обычная зависимость с фичами `gterm` |
| 3–4. Дерево, фокус, маршрутизация | `src/ui/{mod,element,focus,route}.rs`: `Ui<'a, M>` — узлы в порядке обхода в глубину с индексом родителя, `split`/`group`+`focus_style`/`control`/`stateful`+`on_select`/`on_activate`/`widget`; `layout` пишет `node.area`; `render` рисует рамки контейнеров (стиль фокуса, если фокус в поддереве) и элементы; фокус — кандидаты-листы, `ensure_focus` сбрасывает устаревшие флаги и отдаёт фокус первому, `focus_first/next/prev` по кольцу; `route_key` (Tab-перехват, доставка фокусу), `route_mouse` (захват, верхний под ячейкой, фокус по левому нажатию до доставки) |
| 5. Модальный слой | `src/ui/modal.rs`: `Modal::centered/style/on_escape/content`, `Ui::modal`, весь ввод слою в локальных ячейках, мышь поглощается целиком, `Esc` → `on_escape` или «не обработано», `draw_modal`/`modal_buffer` через `Popup` (его API и `tests/popup.rs` не тронуты) |
| 6. Встроенные контролы | `src/widgets/selection.rs` (`RowMap`, `navigate`, `wheel` — один алгоритм на список и таблицу), `ListState`/`TableState` (`node`, карта строк из `render`, `impl Control`/`WidgetEvents`, `List::focus_style`/`Table::focus_style`), `src/widgets/button.rs` (`Button`, `ButtonState`), `Block::patch_border_style` (crate-private) |
| 7. Пример | `examples/widgets.rs` на `Demo::ui()` и `enum Msg { Picked, OpenDialog, CloseDialog }`: группа со списком, таблица, абзац, кнопка «Открыть окно», подсказка; модальное окно с «OK» и `on_escape`; порядок «маршрутизация → разбор → отрисовка»; запуск с инжекцией — код 1 после повтора кадра |
| 8. Документы | ADR-0026 (уточнён по реализации) + индекс; ТЗ 1.21 (§13 — ввод приложения и контролы); `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md`; отчёты `test`/`verify`; размеры release |

Отклонения от плана (зафиксированы в статусе плана): `Element<'a, M>` несёт время жизни описания ради замыканий привязок; мышь внутри слоя мимо контролов поглощается; захват снимается отпусканием любой кнопки; оконный тест без инжекции ввода проверяет заполнение полей `Ctx`.

## COMPLETENESS
- К1–К7 ТЗ — код и тесты без окна: `tests/{input,focus,route,tree_render,modal,controls}.rs` (32 теста) плюс `pointer_cell` в `stage6_app` и поля `Ctx` в `stage6_events`.
- К8 — пример переписан; запуск `--backend=soft --fail-frames=2` прошёл по ТЗ; глазами на трёх бэкендах — за владельцем.
- К9 — гейты и документы ниже.

## CORRECTNESS
- Мутации (исходник восстановлен, прогон после восстановления зелёный): дерево не перехватывает `Tab` → 2 падения в `tests/focus.rs`; кнопка активируется отпусканием где угодно → падение в `tests/controls.rs`.
- В ходе разработки тесты поймали две настоящие ошибки теста/контракта (фокус слоя двигается только вводом слою; заглушка красит фон области) — исправлены до зелёного прогона.
- `tests/widgets.rs` и `tests/popup.rs` части 1 — без изменения ожиданий (К5, К6).

## EVIDENCE
- Базовый прогон до правок (worktree на `HEAD`): 27 бинарников `cargo test`, все `ok`.
- После последнего изменения кода: `cargo fmt --check` чисто; `cargo clippy --all-targets -- -D warnings` чисто; `cargo test` — **236 тестов, 0 падений** в 33 бинарниках + 4 оконных бинарника `ok` (`stage6_events` печатает ячейку и пустые модификаторы). `gterm-widgets`: 48 тестов.
- Размер release 2026-10-30: `demo.exe` 1 544 192 Б (+1 024), `full_table.exe` 1 537 536 Б (+1 024), `overlay.exe` 1 532 928 Б (+1 024), `examples/minimal.exe` 1 165 824 Б (+1 024), `examples/pseudographica.exe` 1 171 968 Б (+1 024), `gterm-widgets/examples/widgets.exe` 1 206 272 Б (+16 384).
- LOC затронутых файлов (`wc -l`): `examples/widgets.rs` 358, `table.rs` 355, `list.rs` 340, `ui/mod.rs` 252, `input.rs` 207, `window.rs` 203, `events.rs` 172, `api.rs` 160, `options.rs` 147, `button.rs` 186, `ui/element.rs` 177, `ui/modal.rs` 146, `ui/route.rs` 121, `ui/focus.rs` 96, `selection.rs` 93, `control.rs` 73; тесты 108–314. Все ниже ориентиров QG-06; `table.rs`/`list.rs` выросли на ≈80 строк каждый (контрол + карта строк) — одна ответственность «виджет и его состояние».
- Проверочный проход BP-09: заново открыты 7 мест — выводы `cargo test` (полный и крейта), сводка release, `wc -l`, `git status`, результаты двух мутаций и восстановления (grep исходных строк). Отброшенных находок нет.

## ⚠ WARNINGS
- Глазами — за владельцем: `cargo run -p gterm-widgets --example widgets` на GL/GLES/Software (К8), в том числе `ModifiersChanged` → `Shift+Tab`: автоматически не проверяется, оконные тесты не инжектируют ввод.
- `ListState`/`TableState` потеряли `Copy` и получили приватные поля — литеральная конструкция `ListState { offset, selected }` в чужом коде перестанет собираться (в репозитории таких мест нет).
- Захват не хранит кнопку: отпускание любой кнопки снимает захват (одна кнопка за раз).
- Карта строк и области в состояниях соответствуют последнему кадру: клик до первой отрисовки не сопоставляется.
- Коммит кода не делался (разрешение BP-08.4 покрывало только коммит документов перед началом работ).
