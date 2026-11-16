# План: двойной клик для любого контрола и ползунок `Slider`

Статус: в работе.

- **ТЗ задачи:** `docs/spec/2026-11-16-double-click-slider.md` (утверждение унаследовано от G-3/G-4 ТЗ `sfplayer` 2026-11-16).
- **Решения:** ADR-0035 (двойной клик), ADR-0036 (ползунок).
- **Опирается на:** ADR-0026, ADR-0031, ADR-0032, ADR-0034.

## Факты по коду (проверено 2026-11-16, коммит `e7c0403`)

- `crates/gterm/src/app/events.rs:74-87` — `WindowEvent::MouseInput` → `PointerEvent::Input { state, button, cell }`; ячейка — `App::pointer_cell()`.
- `crates/gterm/src/app/api.rs:30-34` — `PointerEvent::Input`; сопоставления в репозитории — `gterm-widgets/src/input.rs:88` и `gterm-demo/src/bin/demo/main.rs:223` (оба с `..`); литерал — `gterm-widgets/tests/input.rs:44,49`.
- `crates/gterm-widgets/src/input.rs:72-79` — `MouseMsg { kind, button, col, row, modifiers }`; литералы — `src/input.rs`, `tests/common/mod.rs:182`, `tests/input.rs:107`, `tests/text_selection.rs:198`.
- `src/ui/route.rs:175-199` — `Ui::route_mouse`: захват, затем верхний контрол под ячейкой, фокус до доставки, захват по `Yes` на `Down`.
- `src/ui/element.rs` — привязки хранит `Stateful` и только для встроенных контролов; у `Custom`/`Owned` привязок нет.
- `src/widgets/list.rs:150-189`, `table.rs:140-178` — нажатие сначала проверяется полосой (`BarState::press`), затем картой строк (`RowMap::item_at`).
- Базовый прогон: `fmt`, `clippy --all-targets -D warnings` чисты; `cargo test` — 313 тестов в наборах с обвязкой и 5 оконных бинарников, 0 падений (`target/step-dc/baseline.log`).

## Проектные решения

1. **Счётчик нажатий** — `crates/gterm/src/app/clicks.rs`, по образцу `tick.rs`: состояние в `App`, время аргументом; unit-тесты в том же модуле.
2. **Привязка двойного клика** — в узле дерева (`Node`), а не в `Stateful`: так она есть у любого контрола (`stateful`, `control`, `owned`); публикует `Ui::route_mouse` после доставки нажатия контролу.
3. **Отказ контрола** — метод трейта `Control` с ответом «принимает» по умолчанию; список и таблица отказывают вне строк и на полосе; неактивные кнопка, флажок и ползунок — тоже.
4. **Ползунок** — по образцу флажка (`checkbox.rs`): вид — виджет-значение, поведение — состояние с `Control<M>` и `WidgetEvents`; геометрия дорожки — область узла.

## Шаги

1. **Двойной клик.** `app/clicks.rs`, `app/mod.rs`, `app/events.rs`, `app/api.rs`; `gterm-widgets`: `input.rs`, `control.rs`, `ui/mod.rs`, `ui/element.rs`, `ui/route.rs`, `widgets/{list,table,button,checkbox,scrollbar}.rs`; тесты `tests/double_click.rs`, правка литералов в `tests/common/mod.rs`, `tests/input.rs`, `tests/text_selection.rs`; пример `widgets`; ADR-0035; документы. Проверка: три гейта, снимок окна. Коммит.
2. **Ползунок.** `widgets/slider.rs`, `widgets/mod.rs`, `lib.rs`, `control.rs`, `ui/mod.rs`, `ui/element.rs`; `tests/slider.rs`; пример `widget_edit`; ADR-0036; документы. Проверка: три гейта, снимок окна.
3. **Итог.** Размеры release, отчёты `docs/done|test|verify/2026-11-16-double-click-slider.md`. Коммит вместе с шагом 2.

## Открытые решения

Нет: дизайн задан ведущим агентом по решениям владельца; отклонения записываются в ADR.

## Риски

- Литералы `MouseMsg` у потребителей (`sfplayer`) потребуют поля `click_count`.
