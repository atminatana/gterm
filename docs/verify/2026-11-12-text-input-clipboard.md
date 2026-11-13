# Проверка критериев приёмки: выделение и буфер обмена в `TextInput` (ADR-0030)

ТЗ задачи — `docs/spec/2026-11-12-text-input-clipboard.md`, §4. Второй проход (BP-09): каждый критерий сверен с кодом, тестом или сохранённым выводом команды.

| № | Критерий | Статус | Подтверждение |
|---|---|---|---|
| К1 | Выделение клавиатурой, `Ctrl+A`, снятие движением, границы | ✅ | `tests/text_selection.rs::shift_with_movement_keys…`, `selection_is_counted_in_characters…`; `Ctrl+A` — `tests/clipboard.rs::copy_cut_and_paste…` |
| К2 | Замена выделения набором и вставкой, удаление, одно `Changed` | ✅ | `text_selection.rs::typing_and_deleting_replace…`; вставка вместо выделенного — `clipboard.rs::copy_cut_and_paste…` |
| К3 | Мышь: нажатие, протяжка с захватом, `Shift` + нажатие, прокрутка за краем | ✅ | `text_selection.rs::dragging_with_the_mouse…`, `a_click_drops_the_selection…` |
| К4 | Копировать/вырезать/вставить через `MemoryClipboard` обоими наборами сочетаний; физические клавиши | ✅ | `clipboard.rs::commands_come_from_physical_keys…`, `copy_cut_and_paste…`, `classic_combinations_work…` (логическая клавиша в событиях — «ы») |
| К5 | Поглощение: карта молчит при фокусе в поле, срабатывает на контроле без буфера; поле в слое | ✅ | `clipboard.rs::the_field_absorbs_edit_commands…`, `a_field_in_a_modal_layer…`; мутация «не поглощает» → 5 падений |
| К6 | Отрисовка: стиль выделения, только под фокусом, курсор при выделении не рисуется | ✅ | `text_selection.rs::the_selection_is_drawn_only_under_focus…`; мутация → падение |
| К7 | `SystemClipboard`: сборка, пример `widget_edit` — обмен с другими программами | 🔶 | сборка с фичей и без неё; запуск примера `--backend=soft --fail-frames=2` — код 1 по инжекции; обмен с другими программами (Windows, Linux X11/Wayland) — глазами за владельцем |
| К8 | Гейты, размеры release, документы | ✅ | fmt, clippy `--all-targets -D warnings`, 277 тестов; размеры в `docs/done/…`; ADR-0030 + индекс, ТЗ 1.25, `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md`, отчёты |

**Контракты §3.** «Поле поглощает всегда, и без подключённого буфера» — `without_a_connected_clipboard…`. «Вставляемый текст: перевод строки и табуляция → пробел» — `classic_combinations_work…`. «Без `Ui::clipboard` прежние пути не меняются» — 70 прежних тестов `gterm-widgets` зелёные, тесты `text_input.rs` без правок. «Неактивное поле буфер не трогает» — чтением `text_input.rs::edit` (ветка `!self.enabled`), тестом не покрыто.

**Расхождения с буквой ТЗ.** `ClipboardOp`/`Control::clipboard` реализованы как `EditCommand` (с `SelectAll`) и `Control::edit`; в трейт `Clipboard` добавлен `is_available`. Оба записаны в статусе ТЗ, плане и ADR-0030.

**След прохода.** Открыто заново 6 мест: `ui/route.rs:44-62` и перечень методов `edit`, вывод полного `cargo test` (39 бинарников), вывод прогона `--no-default-features`, сводка размеров release и повторная сборка `demo.exe`, результаты пяти мутаций с `cmp` восстановленного файла, `cargo tree -F arboard`.

Отброшенных находок нет.
