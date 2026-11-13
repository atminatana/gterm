# Отчёт о выполнении: многопоточная отрисовка Software-рендерера

- **Задача:** `2026-10-10-soft-threads`. ТЗ — `docs/spec/2026-10-10-soft-threads.md`, план — `docs/plans/2026-10-10-soft-threads.md`.
- **Решение и его ПОЧЕМУ:** ADR-0016.

## Что сделано

| Шаг плана | Результат |
|---|---|
| 1. Зависимость | `rayon = { version = "1.12", default-features = false }`; в `Cargo.lock` — `rayon 1.12.0`, `rayon-core 1.13.0`, `crossbeam-deque/epoch/utils`, `either` |
| 2. Функции полосы | `src/renderer/soft_band.rs`: `MainPass::draw_band` (`:32`), `OverlayPass::build_band` (`:126`), `layer_cell` (`:191`); из `soft_backend.rs` убраны `draw_row`, `draw_cell`, `fill_pixel`, `composite_row`, `draw_overlay`, `draw_overlay_cell`, поле `Layout::cell_w` |
| 3. Пул | `soft_backend.rs`: `MAX_THREADS` (`:27`), `PARALLEL_MIN_ROWS` (`:30`), `with_threads` (`:95`), `threads` (`:148`), `build_overlay` (`:264`) и `draw_rows` (`:296`) через `for_each_band` (`:444`) |
| 4. Тест выделений | `tests/soft_threads_alloc.rs` — отдельный бинарник со счётчиком |
| 5. Замеры | `docs/test/2026-10-10-soft-threads.md` |
| 6. Документы | ADR-0016 и индекс, ТЗ 1.13 (§8), `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md`, отчёты |

Отклонение от плана: число строк для раздачи полос передаётся явно (`grid_rows`), а не выводится из длины среза — иначе поле окна под сеткой делало полосы неравными.

## COMPLETENESS
- Пул и потоки — код `soft_backend.rs`, тесты `soft_threads` (4) и `soft_threads_alloc` (1).
- Параллельная сборка текстуры — `OverlayPass::build_band`; покрыта `soft_threads` (наложение движется и исчезает) и всеми тестами `tests/overlay.rs` (по умолчанию `new` — до 4 потоков).
- Порог малых кадров — `for_each_band`; замер кадра с одной строкой.

## CORRECTNESS
- Кадр от числа потоков не зависит: `soft_threads` (1 ↔ 4 потока, три размера окна, 6 кадров). Golden не перезаписывались (`git status tests/golden` пуст), `stage4_gl` — «identical, 8 scenes» на GL 3.3 и GLES 3.0.
- Мутации (исходник восстановлен после каждой):
  - сдвиг первой строки полосы в параллельной ветке → падают 3 теста `soft_threads`;
  - `Vec::with_capacity(1)` в раздаче полос → `soft_threads_alloc`: 40 выделений вместо 0.

## EVIDENCE
- Базовый прогон этой задачи — 162 теста (после выноса `pixel.rs`); после — `cargo fmt --check` чисто, `cargo clippy --all-targets -- -D warnings` чисто, `cargo test` — 167 тестов harness, 0 падений, оконные бинарники — ok.
- Однопоточная версия на полосах (шаг 2) прошла все тесты рендера до добавления пула.
- Размер release: `demo.exe` 1 482 240 Б (+28 160), `full_table.exe` 1 476 096 Б, `overlay.exe` 1 470 976 Б.
- LOC: `soft_backend.rs` 466, `soft_band.rs` 195, `pixel.rs` 129.

## ⚠ WARNINGS
- Нагрузка на CPU в простое (до 4 спящих потоков `rayon`) не проверялась — за владельцем, по диспетчеру задач.
- Наложение, меняющееся каждый кадр, по-прежнему не замерялось отдельным сценарием (сборка текстуры теперь параллельна, тест равенства кадров её покрывает).
- Число потоков не настраивается снаружи (вне границ ТЗ). На ВМ с одним vCPU пул не создаётся — рисуем в текущем потоке.
- Глазами (`cargo run`, `cargo run --bin overlay` с `--backend=soft`) — за владельцем.
