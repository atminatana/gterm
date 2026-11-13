# Отчёт о выполнении: выделение текста и буфер обмена в `TextInput`

- **Задача:** `2026-11-12-text-input-clipboard`. ТЗ — `docs/spec/2026-11-12-text-input-clipboard.md` (утверждено 2026-11-12), план — `docs/plans/2026-11-12-text-input-clipboard.md`.
- **Решение и его ПОЧЕМУ:** ADR-0030.

## Что сделано

| Шаг плана | Результат |
|---|---|
| 1. Буфер и команды | `crates/gterm-widgets/src/clipboard.rs`: трейт `Clipboard` (`get`, `set`, `is_available`), `MemoryClipboard`, `EditCommand::{Copy, Cut, Paste, SelectAll}` с `from_key`, `SystemClipboard` на `arboard` под фичей `system-clipboard`; `Cargo.toml` — `arboard 3.7` (optional, `default-features = false`), `default = ["system-clipboard"]`; `control.rs` — `Control::edit` с телом по умолчанию; `lib.rs` — реэкспорт |
| 2. Маршрутизация | `ui/element.rs` — `Element::edit` во всех обёртках (`Stateful` — с превращением события в сообщение); `ui/mod.rs` — поле и `Ui::clipboard`, подъём к корню; `ui/modal.rs` — `Modal::route_edit`; `ui/route.rs` — распознавание команды в `route_key_event` до `route_key`, `route_edit`, `edit_focused` |
| 3. Поле | `widgets/text_input.rs`: якорь выделения, `selection`, `selected_text`, `select_all`, `clear_selection`, `delete_selection`, `paste_str`; `Shift` с клавишами движения; мышь — якорь на нажатии, протяжка под захватом с прокруткой за краем, `Shift` + нажатие; `edit`; `TextInput::selection_style`; отрисовка выделения |
| 4. Тесты | `tests/text_selection.rs` (6), `tests/clipboard.rs` (6); `tests/hotkeys.rs` — явная установка фокуса на заглушку |
| 5. Пример | `widget_edit`: `SystemClipboard` в состоянии, `Ui::clipboard`, подсказка; `required-features = ["system-clipboard"]` |
| 6. Документы | ADR-0030 + индекс; ТЗ проекта 1.25 (шапка, §13); `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md`; отчёты `test`/`verify`; размеры release |

Отличия от текста ТЗ (записаны в его статусе и в плане): перечисление операций — `EditCommand` с `SelectAll`, метод — `Control::edit`; в трейт добавлен `is_available`.

## COMPLETENESS
- К1–К6 — код и 12 тестов без окна.
- К7 — `SystemClipboard` собирается, пример стартует; обмен с другими программами — глазами за владельцем.
- К8 — гейты, размеры и документы ниже.

## CORRECTNESS
- Новые тесты прошли с первого прогона, поэтому проверены пятью мутациями (исходник восстановлен побайтно, полный прогон после восстановления зелёный): поле не поглощает команду → 5 падений; вырезание без проверки доступности буфера → 1; табуляция при вставке не заменяется → 1; выделение рисуется без фокуса → 1; `←` игнорирует начало выделения → 1.
- Дефект собственной реализации, найден до тестов и исправлен: вырезание без подключённого буфера удаляло текст, никуда его не сохранив (`is_available`).
- Вскрытая слабость тестов задачи `2026-11-12-hotkey-map`: три теста `tests/hotkeys.rs` шли с фокусом в поле ввода, а не на заглушке (`focus_next` без фокуса даёт первый контрол); один из них упал, когда поле стало поглощать `Ctrl+C`. Поведение кода верное; фокус в тестах теперь ставится явно и проверяется утверждением.

## EVIDENCE
- Базовый прогон до правок — коммит `e27d9c7`: 265 тестов, 0 падений.
- После последнего изменения кода: `cargo fmt --check` чисто; `cargo clippy --all-targets -- -D warnings` чисто, то же для `gterm-widgets` с `--no-default-features`; `cargo test` — **277 тестов, 0 падений** в 39 бинарниках, код возврата 0; `gterm-widgets` — 82 теста, с `--no-default-features` — те же 82.
- Запуск `cargo run -p gterm-widgets --example widget_edit -- --backend=soft --fail-frames=2`: кадр, повтор, код 1 (как задано инжекцией).
- Зависимости (`cargo tree -F arboard`): Windows — `clipboard-win 5.4.3`, `error-code`, `log`, `windows-sys 0.61`; Linux — `x11rb 0.13.2`, `parking_lot`, `percent-encoding`, `log`. В `Cargo.lock` +7 пакетов.
- Размер release 2026-11-12, сборка всего workspace (`cargo build --release --bins --examples`): `demo.exe` 1 545 216 Б (+1 024), `full_table.exe` 1 538 560 Б (+1 024), `overlay.exe` 1 533 952 Б (+1 024), `examples/minimal.exe` 1 166 848 Б (+512), `examples/pseudographica.exe` 1 171 968 Б, `examples/hotkeys.exe` 1 204 224 Б, `widgets.exe` 1 239 552 Б (+1 024), `widget_edit.exe` 1 247 744 Б (+8 704). `demo.exe` отдельной сборкой `-p gterm-demo --bin demo` — 1 544 192 Б, без изменений.
- LOC: `clipboard.rs` 175, `control.rs` 103, `text_input.rs` 553, `ui/element.rs` 311, `ui/mod.rs` 334, `ui/modal.rs` 254, `ui/route.rs` 240, `tests/text_selection.rs` 252, `tests/clipboard.rs` 267, `tests/hotkeys.rs` 355, `widget_edit.rs` 358. `text_input.rs` пересёк ориентир QG-06 (500): файл — один контрол (состояние с правкой и выделением, обработчики ввода, виджет с отрисовкой), как `combo_box.rs`; делить по «состояние / виджет» — отдельное решение владельца.
- Проверочный проход BP-09: заново открыты 6 мест — `ui/route.rs:44-62` и список методов `edit` по файлам, вывод полного `cargo test`, вывод прогона без фичи, сводка размеров и повторная сборка `demo.exe`, результаты мутаций с побайтным сравнением восстановленного файла, `cargo tree`. Отброшенных находок нет.

## ⚠ WARNINGS
- Системный буфер автотестом не проверяется. Проверено владельцем 2026-11-12 на его машине: «работа с буфером обмена и горячими клавишами проходит корректно». Linux (X11, Wayland) не проверялся.
- Linux: без фичи `arboard/wayland-data-control` буфер на Wayland идёт через XWayland; без XWayland — недоступен (сообщение в stderr, поле работает без буфера). Не проверено.
- Рост `demo.exe`, `full_table.exe`, `overlay.exe`, `widgets.exe` на 1 024 Б и `minimal.exe` на 512 Б наблюдается только при сборке всего workspace одной командой; причина названа по косвенному признаку (отдельная сборка `demo.exe` размера не меняет) — объединение фич общих зависимостей, какая именно фича — не выяснялось.
- `text_input.rs` — 553 строки (см. EVIDENCE); разбиение — по решению владельца.
- При фокусе в поле `Ctrl+A/C/X/V` и классические сочетания до карты сочетаний не доходят; `Shift+Delete` без выделения ничего не удаляет (команда «вырезать» поглощена).
- Контракт «неактивное поле буфер не трогает» отдельным тестом не покрыт: неактивное поле не получает фокус, и дерево его не вызывает; ветка `enabled == false` в `edit` — чтением кода.
- Коммит сделан по запросу владельца 2026-11-12 после его проверки; пуш не делался.
