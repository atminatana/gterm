# Отчёт о выполнении: двойной клик для любого контрола и ползунок `Slider`

- **Задача:** `2026-11-16-double-click-slider`. ТЗ — `docs/spec/2026-11-16-double-click-slider.md` (утверждение унаследовано от G-3/G-4 ТЗ `sfplayer`, 2026-11-16), план — `docs/plans/2026-11-16-double-click-slider.md`.
- **Решения и их ПОЧЕМУ:** ADR-0035 (двойной клик), ADR-0036 (ползунок).
- **Коммиты:** двойной клик — `d1f59f7`; ползунок, размеры и отчёты — следующий коммит.

## Двойной клик (G-4, ADR-0035)

| Что | Где |
|---|---|
| Счётчик нажатий подряд (время аргументом) и 5 unit-тестов в модуле | `crates/gterm/src/app/clicks.rs` (новый) |
| Поле `click_count: u8` у `PointerEvent::Input` | `crates/gterm/src/app/api.rs` |
| Счётчик в `App`, подсчёт на `WindowEvent::MouseInput` | `crates/gterm/src/app/mod.rs`, `app/events.rs` |
| `MouseMsg::click_count`, `MouseMsg::is_double_click()` | `crates/gterm-widgets/src/input.rs` |
| `Control::accepts_double_click(&self, &MouseMsg) -> bool` (по умолчанию `true`) и проброс через элементы | `src/control.rs`, `src/ui/element.rs` |
| Привязка `Ui::on_double_click(M)` в узле дерева; публикация в `Ui::route_mouse` после доставки нажатия | `src/ui/mod.rs`, `src/ui/route.rs` |
| Отказ: список и таблица — вне строк и на полосе (`BarState::contains`); неактивные кнопка и флажок | `src/widgets/{list,table,scrollbar,button,checkbox}.rs` |
| Тесты: `tests/double_click.rs` (11), `tests/input.rs` (+1); поле в литералах `tests/common/mod.rs` (+ помощники `left_double_down`/`left_double_up`), `tests/input.rs` | `crates/gterm-widgets/tests/` |
| Пример: двойной клик по пункту списка пишет его в строку позиции внизу | `examples/widgets.rs` |
| Документы: ADR-0035 + индекс, ТЗ проекта 1.29 (шапка, FR-25), `AGENTS.md`, `PROJECT_MAP.md`, README ×2, ТЗ и план задачи | `docs/`, корень |

## Ползунок (G-3, ADR-0036)

| Что | Где |
|---|---|
| `Slider` (виджет) и `SliderState` (контрол): значение `i64`, границы, шаг, клавиши, мышь с захватом, колесо, `Default` 0…100 | `crates/gterm-widgets/src/widgets/slider.rs` (новый), экспорт — `widgets/mod.rs` |
| `WidgetEvent::ValueChanged(i64)`, привязка `Ui::on_value` | `src/control.rs`, `src/ui/element.rs`, `src/ui/mod.rs` |
| Тесты: `tests/slider.rs` (10) | `crates/gterm-widgets/tests/` |
| Пример: строка «Уровень» — ползунок 0…100 с шагом 5 и значение справа; сообщение — в строку статуса; «Сброс» ставит 50 программно | `examples/widget_edit.rs` |
| Документы: ADR-0036 + индекс, ТЗ проекта 1.29 (§13), `AGENTS.md`, `PROJECT_MAP.md`, README ×2, размеры release, эти отчёты | `docs/`, корень |

## COMPLETENESS
- Д1–Д5 (двойной клик): код и тесты — счётчик (5 unit-тестов), перенос числа (`tests/input.rs`), привязка на кнопке, флажке, списке, таблице, своём контроле и в модальном слое, отказ на полосе прокрутки, заголовке и пустых строках, неактивная кнопка, одиночный клик, без привязки — как раньше (`tests/double_click.rs`, 11).
- П1–П4 (ползунок): код и 10 тестов `tests/slider.rs`.
- О1: гейты, размеры, документы, примеры — ниже; живая проверка — см. EVIDENCE и WARNINGS.

## CORRECTNESS
- Все новые тесты прошли с первого прогона, поэтому проверены мутациями (файл копировался в `target/step-dc/`, после прогона восстанавливался и сверялся `cmp`; `git status` после серии — без посторонних правок). 8 мутаций, все пойманы:
  - интервал `<` вместо `<=` → `the_interval_boundary_is_inclusive`;
  - двойной не сбрасывает счёт → `the_press_after_a_double_starts_counting_again`;
  - список не отказывает на полосе → `a_double_click_on_the_list_scrollbar_gives_no_message`;
  - дерево не публикует сообщение → 6 тестов `double_click`;
  - неактивная кнопка принимает двойной клик → `a_disabled_button_gives_no_double_click_message`;
  - значение ползунка по колонке округляется вниз → `the_value_rounds_to_the_nearest_column…`;
  - событие ползунка без проверки изменения → 4 теста `slider`;
  - движение под захватом не ведёт значение → 2 теста `slider`.

## EVIDENCE
- Базовый прогон до правок (коммит `e7c0403`): `fmt` и `clippy --all-targets -D warnings` чисты; `cargo test` — 313 тестов в наборах с обвязкой, 0 падений, плюс 5 оконных бинарников; журнал `target/step-dc/baseline.log`.
- После двойного клика: три гейта зелёные, 330 тестов, 0 падений — `target/step-dc/feature1.log`.
- После ползунка: три гейта зелёные, 340 тестов, 0 падений — `target/step-dc/feature2.log`; итоговый прогон после всех правок — `target/step-dc/final.log` (цифры — в `docs/test/2026-11-16-double-click-slider.md`).
- Живая проверка (`tools/shot.ps1`, OpenGL 3.3, по одной попытке): `widgets` — клик по пункту «Рамки одиночные» выделил его, внизу строка «выделено 4 из 14, двойной клик — открыть» (`target/shots/dc-1.png`); `widget_edit` — четыре `Tab` и два `→`: ползунок под фокусом (фон фокуса), ползунок на 60 %, справа «60 %», в статусе «уровень: 60 %» (`target/shots/slider-1.png`).
- Размер release 2026-11-16 (`cargo build --release --bins --examples`, `target/step-dc/sizes.txt`): `demo.exe` 1 546 752 Б (±0), `full_table.exe` 1 540 608 Б (+512), `overlay.exe` 1 536 000 Б (+512), `minimal.exe` 1 168 384 Б (+512), `pseudographica.exe` 1 174 016 Б (±0), `hotkeys.exe` 1 205 760 Б (±0), `widgets.exe` 1 259 520 Б (+2 048), `widget_edit.exe` 1 260 032 Б (+6 144). Приросты — к значениям 2026-11-13 в `AGENTS.md`; на `e7c0403` размеры заново не снимались.
- LOC затронутых файлов (`wc -l`): `app/clicks.rs` 136 (из них тесты 76), `app/events.rs` 217, `app/api.rs` 179, `app/mod.rs` 162; `input.rs` 228, `control.rs` 127, `ui/mod.rs` 373, `ui/route.rs` 286, `ui/element.rs` 378, `widgets/list.rs` 454, `widgets/table.rs` 484, `widgets/scrollbar.rs` 452, `widgets/button.rs` 191, `widgets/checkbox.rs` 246, `widgets/slider.rs` 337; тесты `double_click.rs` 346, `slider.rs` 275, `input.rs` 160, `common/mod.rs` 221; примеры `widgets.rs` 516, `widget_edit.rs` 426.

## ⚠ WARNINGS
- **Сам двойной клик в окне не проверен:** `tools/shot.ps1` умеет один `-Click` за вызов и двойного клика не шлёт. Владельцу: в `cargo run -p gterm-widgets --example widgets` дважды щёлкнуть пункт списка — внизу «открыт «…»»; двойной щелчок по полосе прокрутки списка — строка не меняется. Протяжка и колесо ползунка в окне тоже не проверялись (снимок — только клавиши): в `widget_edit` потянуть ползунок мышью и покрутить колесо над ним.
- `MouseMsg` и `PointerEvent::Input` получили поле: внешний код с литералами перестанет собираться. В `sfplayer` литералы `MouseMsg` — в `src/ui/{buttons,input_dialog,main_screen,window,piano}/tests.rs` (7 мест); нужно добавить `click_count` (1 для `Down`/`Up`, 0 для прочего).
- `examples/widgets.rs` — 516 строк (было 501, ориентир QG-06 — 500, превышение унаследовано и выросло на 15 строк): файл делает одно дело — показывает все виджеты одним описанием `ui()`; делить ради числа не стал.
- В `crates/gterm/src/` это первые unit-тесты внутри модуля (`#[cfg(test)]` в `clicks.rs`) — осознанное отклонение от привычки репозитория, записано в ADR-0035.
- Интервал 500 мс не берётся из настроек ОС; при диапазоне ползунка шире дорожки мышь даёт только значения, попадающие в колонки (точно — клавишами и колесом). Оба пункта — в «Последствиях» ADR.
- Linux (X11, Wayland) не проверялся: хост — Windows.
- `PROJECT_MAP.md` в шапке ссылается на «ТЗ 1.19» — унаследовано, не трогал.

## Поправка после публикации — захват указателя у поля выпадающего списка (2026-11-16)

Найдено при подключении выпадающего списка в sfplayer (чтением `route.rs` и `combo_box.rs`), подтверждено опытом до правки.

```
PROBLEM: после раскрытия выпадающего списка кликом и закрытия по Esc или кликом мимо следующее нажатие в любом месте снова раскрывает список
FILE:LINE: crates/gterm-widgets/src/ui/route.rs (захват после принятого нажатия); src/widgets/combo_box.rs — ComboBoxState::mouse и Ui::combo_box
CHAIN BREAK: [дерево: нажатие на поле → захват] → [слой списка: парное отпускание] — отпускание до поля не доходит, захват не снимается
EVIDENCE:
  - опытный тест до правки: после клика open=true captured=true; после Esc open=false captured=true; после нажатия на соседний контрол open=true
  - Ui::combo_box: пока список раскрыт, поля в дереве нет (вместо него лист-снимок) — снять захват некому
CAUSE: поле принимает нажатие, дерево ставит ему захват; следующее описание уже со слоем, и отпускание уходит слою
FIX: Ui::combo_box при построении раскрытого списка снимает у поля флаг захвата — в этот момент поля в дереве нет и держать указатель ему нечем
```

- `crates/gterm-widgets/src/widgets/combo_box.rs`: одна строка в `Ui::combo_box` и комментарий о причине.
- `crates/gterm-widgets/tests/combo_box.rs`: регрессионный тест `a_field_opened_by_a_click_does_not_keep_the_pointer_after_the_list_is_closed` — закрытие по Esc и кликом мимо слоя.

**COMPLETENESS** — дефект закрыт для обоих способов закрытия; раскрытие клавишами захвата не ставило и не затронуто.

**CORRECTNESS** — тот же сценарий до правки падал (опытный тест, удалён после переноса в регрессионный); после правки тест зелёный. Правка локальна в коде выпадающего списка: маршрутизация дерева и другие контролы не менялись.

**EVIDENCE** — `cargo fmt --check` ok; `cargo clippy --all-targets -- -D warnings` чисто; `cargo test`: 48 наборов, 341 тест (+1), 0 падений (`target/step-dc/combo-fix.log`; первый прогон остановился на форматировании нового теста — `combo-fix.fmt-failure.log`). Размеры release (`target/step-dc/combo-fix-sizes.txt`): все восемь бинарников — без изменений.

**⚠ WARNINGS** — в живом окне не проверено; правила отладки прочитаны после того, как причина была найдена чтением кода, до правки.
