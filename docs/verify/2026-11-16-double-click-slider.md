# Проверка критериев приёмки: двойной клик (ADR-0035) и ползунок `Slider` (ADR-0036)

Источник требований — разделы G-3 и G-4 ТЗ `sfplayer` `docs/spec/2026-11-16-dialogs-and-keys.md` (утверждены владельцем 2026-11-16) и их пересказ — `docs/spec/2026-11-16-double-click-slider.md`, §4. Второй проход (BP-09): каждый пункт сверен с кодом или тестом, места открыты заново.

## G-4. Двойной клик

| Пункт G-4 | Статус | Подтверждение |
|---|---|---|
| Двойной клик — два нажатия левой кнопки подряд по одному месту с паузой не больше 500 мс; интервал и допуск по месту — в ADR | ✅ | `crates/gterm/src/app/clicks.rs` — `DOUBLE_CLICK_INTERVAL` 500 мс, `press`: левая кнопка, та же ячейка, `<=`; тесты `the_interval_boundary_is_inclusive`, `a_press_in_another_cell_starts_over`, `another_button_is_never_double…`; ADR-0035 п.1 (допуск — та же ячейка) |
| Нажатия считает окно; число нажатий приходит в сообщении мыши; часов в контролах нет | ✅ | `app/events.rs` — `WindowEvent::MouseInput` → `self.clicks.press(…, Instant::now())` → `PointerEvent::Input::click_count`; `gterm-widgets/src/input.rs` — `MouseMsg::click_count`, `is_double_click`; тест `click_count_passes_through…`; в коде `gterm-widgets/src` вызовов `Instant::now` нет (одно упоминание — в doc-комментарии `Ui::tick`, время туда приносит приложение) |
| Привязку двойного клика может получить любой контрол дерева; сообщение — когда двойной клик пришёлся на область контрола | ✅ | `ui/mod.rs` — `Ui::on_double_click` в узле (`Node::on_double_click`) для любого элемента с состоянием; `ui/route.rs` — `hit` по области и `is_double_click_on`; тесты на кнопке, флажке, списке, таблице, своём контроле, в модальном слое |
| Контрол без привязки ведёт себя как раньше | ✅ | `route.rs` — без привязки возвращается прежний `Routed::new(handled, out)`; тесты `a_list_without_the_binding_routes_a_double_click_like_two_ordinary_clicks`, первая половина `an_application_control_gets_the_press…`; прежние 313 тестов проходят без правок утверждений (правились только литералы `MouseMsg`/`PointerEvent::Input`) |
| Первое нажатие действует как обычный клик — к сообщению выделена строка под указателем | ✅ | `route.rs` — нажатие доставляется контролу до публикации; тест `in_a_list_the_row_under_the_pointer_is_selected_when_the_message_arrives` (`Picked(2)` на первом нажатии, `selected() == Some(2)` при сообщении) |
| Двойной клик по полосе прокрутки списка сообщения не даёт | ✅ | `widgets/list.rs` и `table.rs` — `accepts_double_click` (`!self.scroll.contains(msg)` и строка по карте); тесты `a_double_click_on_the_list_scrollbar_gives_no_message`, `a_table_counts…`; мутация «нет отказа на полосе» → падение |
| Третье нажатие подряд начинает счёт заново | ✅ | `clicks.rs` — условие `last.count == 1`; тест `the_press_after_a_double_starts_counting_again`; мутация → падение |
| Работа — в `gterm-git` по правилам репозитория | ✅ | ADR-0035 + индекс, ТЗ и план задачи, пример `widgets`, гейты (`target/step-dc/feature1.log`), коммит `d1f59f7` |

## G-3. Ползунок

| Пункт G-3 | Статус | Подтверждение |
|---|---|---|
| Горизонтальная дорожка заданной ширины с заполненной частью; целое значение в границах; шаг для клавиш и колеса | ✅ | `widgets/slider.rs` — `SliderState { value, min, max, step }` (`i64`), `Slider::render` на ширину области; тест `the_track_shows_the_filled_part…` |
| Контрол с фокусом: ← → на шаг, Home и End — границы; в границах значение не меняется | ✅ | `SliderState::key`; тесты `arrows_step_home_and_end…`, `a_step_that_overshoots_a_bound…` |
| Нажатие на дорожке ставит значение по положению, протяжка с захватом до отпускания, колесо на шаг | ✅ | `SliderState::mouse`, `value_at`; тесты `a_press_on_the_track…` (оба края и середина), `the_value_rounds…`, `dragging_under_capture…`, `leaving_the_window…`, `the_wheel_changes…`; мутации «округление вниз», «нет протяжки» → падения |
| Сообщение с новым значением только при изменении пользователем; программная установка сообщения не даёт | ✅ | `change_by_user` → `WidgetEvent::ValueChanged`; `set_value`/`set_range`/`set_step` событие не пишут; `Ui::on_value`; тест `programmatic_changes_give_no_event…`; мутация «событие без изменения» → 4 падения |
| Виды: обычный, под фокусом, неактивный; неактивный не получает фокус и не реагирует | ✅ | `Slider::render` — `focus_style`/`disabled_style`; `focusable() == enabled`, ранний выход в `key`/`mouse`; тесты `a_disabled_slider…`, `the_track_shows…` |
| Работа — в `gterm-git` по правилам репозитория | ✅ | ADR-0036 + индекс, пример `widget_edit`, гейты (`target/step-dc/feature2.log`), снимок окна `target/shots/slider-1.png` |

**Расхождения с буквой G-3/G-4.** Нет. Уточнения сверх текста (записаны в ADR): неактивные кнопка, флажок и ползунок двойного клика не принимают (ADR-0035 п.5); в границе клавиша ползунка принимается без события (ADR-0036 п.3); `SliderState::default()` — 0…100 (ADR-0036 п.2).

**Не проверено вживую:** двойной клик и протяжка/колесо ползунка в настоящем окне (`tools/shot.ps1` двойного клика не шлёт; снимок ползунка — клавишами). Глазами — за владельцем.

**След прохода.** Открыто заново 9 мест: `app/clicks.rs::press`, `app/events.rs` (ветка `MouseInput`), `input.rs::from_pointer`/`is_double_click`, `ui/mod.rs::on_double_click`, `ui/route.rs::route_mouse`/`is_double_click_on`, `list.rs`/`table.rs::accepts_double_click`, `slider.rs` (`key`, `mouse`, `change_by_user`, `render`), журналы `feature1.log`/`feature2.log`, результаты 8 мутаций. Отброшенных находок нет.
