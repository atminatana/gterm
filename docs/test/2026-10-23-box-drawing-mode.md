# Отчёт о тестах: режим псевдографики и пример `pseudographica`

- **Задача:** `2026-10-23-box-drawing-mode` (ADR-0018)
- **Окружение:** Windows 10 Pro 19045, NVIDIA GTX 1060 3GB (драйвер 581.57), Rust stable 1.98.1; тесты — debug, размеры — release (`opt-level = "z"`); системный моноширинный шрифт — `Cascadia Mono`

## Состав

| Файл | Тесты |
|---|---|
| `crates/gterm/tests/box_drawing_mode.rs` (3) | **К2** `modes_pick_their_source`: умолчание `Procedural`; по всем 160 знакам U+2500–U+259F шрифта репозитория — формула рисует, `Combined` = `Font` при наличии глифа, глиф шрифта отличается от формулы хотя бы у одного знака (факт: у 138); ветка «глифа нет» печатает пропуск. **К3** `style_independence_follows_mode`: в `Procedural` после `ensure(┼, Regular)` готовы все четыре канала; смена на `Combined` — `resets` 0 → 1, прежний слот недействителен, готов только канал Regular; тот же режим — без сброса. **К4** `switching_mode_redraws_the_whole_frame`: Software после `set_box_drawing_mode(Combined)` при экране без dirty-строк даёт кадр, равный кадру свежего рендерера в `Combined`; обратно в `Procedural` — равен первому |
| `tests/stage4_gl.rs` (+2 сцены) | `box font 16px`, `box combined 16px`: рамки трёх видов, стыки, штриховые, дуги, тени и блоки, строка с `BOLD | ITALIC`; GL и Software переключают режим на том же атласе перед сценой |
| `tests/events_common/mod.rs` (`Probe`) | на втором кадре `ctx.request.box_drawing = Some(Font)` во всех четырёх оконных бинарниках — кадры продолжают идти (`assert_kept_drawing`) |
| прежние (172) | без изменений эталонов |

## Результат прогона

- `cargo test` из корня: **175 тестов** harness, 0 падений; оконные `stage6_events`, `stage6_events_auto`, `stage6_fallback`, `stage8_faults` — ok.
- `stage4_gl -- --nocapture`: «OpenGL 3.3: GL ↔ Software identical, 10 scenes», «OpenGL ES 3.0: GL ↔ Software identical, 10 scenes».
- `box_drawing_mode -- --nocapture`: «шрифт репозитория: 160 глифов диапазона в cmap, 0 отсутствуют», «глиф шрифта отличается от формулы у 138 из 160», «ветка «глифа нет в шрифте» не покрыта».
- Golden: `git status crates/gterm/tests/golden` пуст.
- Мутации (исходник восстановлен, прогон после восстановления зелёный): без сброса страницы при смене режима — 2 падения; `is_style_independent` без учёта режима — 1 падение.
- Гейты: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` — чисто.

## Запуски

| Команда | stderr / код возврата |
|---|---|
| `cargo run -p gterm --example pseudographica -- --backend=soft --fail-frames=2` | `pseudographica: font 'Cascadia Mono' — C:\Windows\Fonts\CascadiaMono-Regular.otf`, `backend Software`, кадр нарисован, инжекция → код 1 (штатно, ADR-0007) |

Размер release: `demo.exe` 1 522 688 Б (без изменений), `full_table.exe` 1 515 520, `overlay.exe` 1 511 424, `minimal.exe` 1 144 832, `pseudographica.exe` 1 147 392 Б.

## Ограничения

- Окно глазами (три таблицы, `q/w/e` на `1/2/3`, статус и подсказка, стыки шрифтовых рамок) — на машине владельца.
- Ветка «в шрифте нет глифа рамки» не покрыта автотестом — шрифт репозитория содержит весь диапазон.
- Оконные тесты не сравнивают содержимое кадра после `Request::box_drawing`.
