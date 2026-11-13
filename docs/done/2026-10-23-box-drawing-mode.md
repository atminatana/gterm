# Отчёт о выполнении: режим псевдографики и пример `pseudographica`

- **Задача:** `2026-10-23-box-drawing-mode`. ТЗ — `docs/spec/2026-10-23-box-drawing-mode.md`, план — `docs/plans/2026-10-23-box-drawing-mode.md`.
- **Решение и его ПОЧЕМУ:** ADR-0018.

## Что сделано

| Шаг плана | Результат |
|---|---|
| 1. Библиотека | `rasterizer.rs`: `BoxDrawingMode` (`:24`), поле `box_drawing`, `set_box_drawing_mode`/`box_drawing_mode` (`:120`), `is_style_independent` по режиму (`:131`), `has_font_glyph` по режиму, `rasterize` → ветки режима + выделенный `rasterize_font_glyph` (`:239`); `atlas.rs`: `set_box_drawing_mode` (`:122`) — режим + `clear_page`; `gl_backend.rs:435`, `soft_backend.rs:189` — как после `set_size`; `app/backend.rs` — диспетчер; `app/api.rs:49` — `Request::box_drawing`; `app/events.rs` — применение к бэкенду (или к растеризатору до бэкенда) + перерисовка; `lib.rs` — документация |
| 2. Тесты | `tests/box_drawing_mode.rs` (3): источники по режиму, каналы атласа и сброс, кадр Software после смены; `stage4_gl` — поле `Scene::box_mode`, сцены `box font 16px` и `box combined 16px` (10 сцен); `events_common::Probe` — `Request::box_drawing = Font` на втором кадре |
| 3. Пример | `crates/gterm/examples/pseudographica.rs`, `[[example]]` в манифесте: `FixedGrid` 88×13, системный шрифт 11 px, три таблицы 22×5 по общему набору линий (`Lines`: одиночные, двойные, жирная рамка с тонкими перегородками), строка дополнений (стыки одиночных с двойными, штриховые, дуги, тени и блоки), статус в строке 0, подсказка в последней; `1/2/3`, `q/w/e` (и `й/ц/у`), `Esc` |
| 4. Документы | ADR-0018 + индекс; ТЗ 1.15 (FR-8.4, FR-25, §5.1); `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md`; отчёты `test`/`verify` |

Отклонений от плана нет. Уточнение по К2: шрифт репозитория содержит все 160 знаков U+2500–U+259F, поэтому ветка «глифа нет в шрифте» (`Font` → пусто, `Combined` → формула) автотестом не покрыта — тест печатает это и проверяет ветку «глиф есть» по всему диапазону (138 из 160 знаков отличаются от формулы, `Combined` = `Font` на каждом).

## COMPLETENESS
- Три режима и переключение — код библиотеки; тесты `box_drawing_mode` (3), `stage4_gl` (две новые сцены, оба уровня GL), оконные тесты через `Probe` (запрос на втором кадре на всех четырёх бинарниках).
- Пример — код; проверен запуском с `--backend=soft --fail-frames=2` (кадр нарисован на `Cascadia Mono`); окно глазами — за владельцем.
- Умолчание неизменно — golden не менялись, все прежние тесты зелёные.

## CORRECTNESS
- `stage4_gl`: «OpenGL 3.3: GL ↔ Software identical, 10 scenes», «OpenGL ES 3.0: … 10 scenes» — рамки из шрифта и комбинированные бит-идентичны на обоих уровнях, включая каналы по начертаниям (`BOLD | ITALIC` в сцене).
- Мутации (исходник восстановлен, прогон после восстановления зелёный):
  - `Atlas::set_box_drawing_mode` без сброса страницы → падают `style_independence_follows_mode` и `switching_mode_redraws_the_whole_frame`;
  - `is_style_independent` без учёта режима → падает `style_independence_follows_mode`.
- Кадр Software после смены режима равен кадру свежего рендерера в том же режиме при экране без dirty-строк, и возвращается к исходному при обратной смене.

## EVIDENCE
- Гейты после последнего изменения кода: `cargo fmt --check` чисто, `cargo clippy --all-targets -- -D warnings` чисто, `cargo test` — **175 тестов** harness (172 + 3), 0 падений, 4 оконных бинарника ok; `git status crates/gterm/tests/golden` пуст.
- Размер release (Windows, профиль §10.2): `demo.exe` 1 522 688 Б (без изменений), `full_table.exe` 1 515 520, `overlay.exe` 1 511 424, `minimal.exe` 1 144 832 (без изменений), `pseudographica.exe` 1 147 392 Б.
- LOC: `rasterizer.rs` 293, `atlas.rs` 313, `examples/pseudographica.rs` 344, `tests/box_drawing_mode.rs` 177, `tests/stage4_gl.rs` 550 (тестовый файл: таблица сцен).
- Проверочный проход BP-09: заново открыты 7 мест — `grep -n` по новым функциям, `wc -l`, вывод `stage4_gl`, вывод `box_drawing_mode -- --nocapture`, `git status` golden, `ls -l target/release`, индекс ADR. Отброшенных находок нет.

## ⚠ WARNINGS
- Ветка «глифа нет в шрифте» не покрыта автотестом (шрифт репозитория полный); проверка — глазами на системном шрифте без блоков/теней или тестом на TTF с урезанным `cmap` (записано в `TODO.md`).
- Окно глазами (три таблицы, переключение `q/w/e` на `1/2/3`, статус и подсказка) — за владельцем.
- `Probe` оконных тестов проверяет только, что после `Request::box_drawing` кадры идут дальше; содержимое кадра окна не сравнивается (как и для остальных запросов `Probe`).
- Коммит не делался.
