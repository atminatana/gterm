# Проверка критериев приёмки: библиотека `gterm` без `assets/`; системный моноширинный шрифт

ТЗ задачи — `docs/spec/2026-10-23-lib-split-system-font.md`, §4. Второй проход (BP-09): каждый критерий сверен с кодом, тестом или сохранённым выводом прогона 2026-10-23.

| № | Критерий | Статус | Подтверждение |
|---|---|---|---|
| К1 | Крейт библиотеки без `assets/`, бинарников, `find_folder`, имени шрифта и констант каталогов; собирается отдельно | ✅ | листинг `crates/gterm/` (`src`, `tests`, `examples`, `Cargo.toml`); `grep -rn "find_folder\|find_assets_dir\|ASSETS_DIR" crates/gterm/src crates/gterm/tests` пусто; `grep -rn assets crates/gterm/src` — только слово в документации `font.rs`; `cargo clippy --all-targets` собирает `gterm` как член workspace |
| К2 | Golden и все прежние тесты зелёные без правки эталонов | ✅ | хеши `soft_golden_{16,32}px.bin` равны `HEAD`; `cargo test` 172/0, оконные — ok; `stage4_gl` 1 тест ok |
| К3 | `load(dir, family)` ↔ прежний `load(assets_dir)` | ✅ | `stage2_font::sources::from_files_matches_load` (файлы по начертаниям, метрики, растр бит в бит); `family::*` на новой сигнатуре; golden не изменились — растр тот же |
| К4 | Пропорциональный шрифт отклоняется | ✅ | `sources::proportional_font_is_rejected` (копия TTF с половинным advance `i`); мутация с отключённой проверкой поймана |
| К5 | `system_monospace()` на Windows находит семейство, печатает файлы, демо рисует на трёх бэкендах | 🔶 | автоматически: `Cascadia Mono`, файлы `C:\Windows\Fonts\CascadiaMono-*.otf` в stderr, GL 3.3 (`--bench`), Software (`--fail-frames=2` — кадр нарисован); GLES и окно глазами — за владельцем. Время выбора 151,7–158,0 мс release (тёплый кэш) — выше порога ≈ 100 мс плана, решение за владельцем |
| К6 | `system(family)` по имени; несуществующее — ошибка с перечнем | ✅ | `system::system_family_by_name`: `NoRegularFace { family: "No Such Family", found: [] }`, по имени умолчания — тот же Regular-файл |
| К7 | Детерминизм выбора | ✅ | `system::system_monospace_is_deterministic`: два вызова — те же файлы, метрики, семейство |
| К8 | Демо без `assets/` — системный шрифт; с `assets/` — цепочка О4 | 🔶 | с `assets/`: `font source: assets …` из корня workspace (каталог пакета) и из `cargo test` (cwd пакета); принудительно системный — `--system-font`; копия `demo.exe` в пустой каталог — за владельцем (код пути `font_source`: `find_assets_dir().map_or(System, Assets)`) |
| К9 | Linux: `system_monospace()` через fontconfig | ⏸ | `[data needed: прогон на Linux-машине владельца — resolved by cargo test и cargo run --bin demo -- --system-font]`; код пути: `fontdb` с фичей `fontconfig`, алиас `monospace` первым в списке (`font/system.rs:41`) |
| К10 | NFR-6, NFR-9 | ✅ | `soft_threads_alloc` зелёный (кадр не трогался); `grep -rn unsafe crates/gterm/src` — только `gl_backend.rs` (и упоминание в документации `renderer/mod.rs`) |
| К11 | Гейты, размер | ✅ | fmt, clippy `--all-targets -D warnings`, 172 теста; `demo.exe` 1 522 688 Б (+40 448), `full_table.exe` 1 515 520, `overlay.exe` 1 511 424, `minimal.exe` 1 144 832 Б |
| К12 | Документы | ✅ | ADR-0017 + индекс; `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md`, ТЗ 1.14 (§1, FR-8, FR-8.1, FR-8.5, FR-8.6, NFR-3, §5.1, §10.1) |
| К13 | Минимальный пример без `assets/` | 🔶 | `crates/gterm/examples/minimal.rs` зависит только от `gterm` (без `winit` в импортах); в `crates/gterm/` нет `assets/`; `minimal --backend=soft --fail-frames=2` — шрифт `Cascadia Mono`, кадр нарисован; окно глазами на трёх бэкендах — за владельцем |

**След прохода.** Открыто заново 9 мест: номера строк функций `font.rs`/`font/system.rs` (`grep -n`); `wc -l` затронутых файлов; хеши golden против `HEAD`; сохранённые выводы `cargo test`, `cargo clippy`, трёх release-прогонов `--bench --system-font` и запусков с `--fail-frames=2`; `ls -l target/release`; `Cargo.lock` (`fontdb 0.24.0`, `memmap2`, `fontconfig-parser`); `git status`; листинг `crates/gterm/`.

Отброшенных находок нет. Расхождения с ТЗ: ни одного по контрактам §3; время выбора системного шрифта выше порога плана — зафиксировано как решение владельца (`TODO.md`). Отклонения от плана (перенос `find_assets_dir` на шаге 1, третий источник поиска, метод `family()`, способ построения К4) записаны в отчёте о выполнении.
