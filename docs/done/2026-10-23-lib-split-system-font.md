# Отчёт о выполнении: библиотека `gterm` без `assets/` и бинарников; системный моноширинный шрифт

- **Задача:** `2026-10-23-lib-split-system-font`. ТЗ — `docs/spec/2026-10-23-lib-split-system-font.md`, план — `docs/plans/2026-10-23-lib-split-system-font.md`.
- **Решение и его ПОЧЕМУ:** ADR-0017.

## Что сделано

| Шаг плана | Результат |
|---|---|
| 0. Базовая линия | fmt, clippy чисто; 167 тестов harness + 4 оконных бинарника; `demo.exe` 1 482 240, `full_table.exe` 1 476 096, `overlay.exe` 1 470 976 Б |
| 1. Workspace | корневой `Cargo.toml` (`[workspace]`, `resolver = "3"`, `[profile.release]`); `crates/gterm/` — библиотека и тесты (`git mv`, 69 файлов, golden совпадают с `HEAD` по хешу), `crates/gterm-demo/` — бинарники, `src/lib.rs` (`assets`, `common`), `assets/`; `find_folder` только в демо; шрифт тестов — `tests/common::repo_fonts_dir` → `../gterm-demo/assets/fonts` |
| 2. API шрифта | `src/font.rs`: `FontFamily::load(fonts_dir, family)` (`:154`), `from_files` (`:168`), `from_faces(faces, family, found)` (`:198`), `family()`; `check_monospace` (`:313`) → `FontError::NotMonospace`; `NoRegularFace.family: String`; `FONT_FAMILY`, `ASSETS_DIR_NAME`, `FONTS_DIR_NAME`, `find_assets_dir`, `fonts_dir`, `svg_dir`, `AssetsDirNotFound` ушли из библиотеки — `crates/gterm-demo/src/assets.rs` |
| 3. Системный шрифт | `src/font/system.rs`: `FontFamily::system` (`:28`), `system_monospace` (`:37`), кандидаты по платформе, `FontError::NoSystemMonospace { tried }`; `fontdb 0.24.0` (`fs`, `memmap`, `fontconfig`) в `Cargo.lock` с `fontconfig-parser 0.5.8`, `memmap2 0.9.11`, `slotmap`, `tinyvec` |
| 4. Демо и пример | `common.rs`: `FontSource` (`Assets(dir)` / `System`), `font_source(force)`, флаг `--system-font` (`SYSTEM_FONT`), `usage(program)`, `load_family(source, px)` с логом `font source: … in …`, `load_rasterizer(source)` — SVG только при `assets`; три бинарника и `bench` принимают источник; `crates/gterm/examples/minimal.rs` — потребитель без `assets/` |
| 5. Замеры и документы | ниже; ADR-0017 (последствия с замерами), ТЗ 1.14 (§1, FR-8, FR-8.1, FR-8.5, FR-8.6, NFR-3, §5.1, §10.1), `AGENTS.md`, `PROJECT_MAP.md`, `TODO.md`, отчёты `test`/`verify` |

Отклонения от плана:
- `find_assets_dir` и его тест переехали в пакет демо на шаге 1, а не 2: после переноса `assets/` в `crates/gterm-demo/` путь от exe и от cwd его не находит, и без переноса гейты шага 1 не прошли бы. Добавлен третий источник поиска — каталог пакета на момент сборки (`env!("CARGO_MANIFEST_DIR")`), иначе `cargo run --bin demo` из корня workspace не видит `assets/`.
- У `FontFamily` появился метод `family()` (имя семейства): нужен логу демо и примеру, в ТЗ не назван.
- Тест К4 построен без внешнего пропорционального шрифта: копия Regular репозитория с расширенной `hmtx` и половинным advance `i` (`stage2_font::sources::with_narrow_i`).

## COMPLETENESS
- Workspace и перенос — манифесты, `gterm_demo::assets` (+1 юнит-тест `find_assets_dir_locates_package_assets_from_test_cwd`), все прежние тесты на новых путях.
- API шрифта — код `font.rs`; тесты `stage2_font::family::*` (на новой сигнатуре), `sources::wrong_family_name_is_no_regular_face_listing_every_file`, `sources::from_files_matches_load` (К3), `sources::proportional_font_is_rejected` (К4).
- Системный шрифт — код `font/system.rs`; тесты `stage2_font::system::system_monospace_is_deterministic` (К5/К7), `system::system_family_by_name` (К6); без моноширинного шрифта в системе — печать причины и выход.
- Демо и пример — код `common.rs`, бинарники, `examples/minimal.rs`; проверены запусками (ниже), окно глазами — за владельцем.

## CORRECTNESS
- Golden не перезаписывались: хеши `soft_golden_{16,32}px.bin` равны `HEAD`; `stage4_gl`, `stage6_*`, `stage8_faults`, `overlay`, `soft_*` зелёные без правок эталонов.
- К3: семейство из `from_files` даёт те же файлы по начертаниям, те же метрики и тот же растр `A` Regular / `Я` Bold, что `load`.
- Мутации (исходник восстановлен после каждой, тесты снова зелёные):
  - `check_monospace` никогда не отказывает → падает `proportional_font_is_rejected`;
  - `from_files` не фильтрует по имени семейства → падает `wrong_family_name_is_no_regular_face_listing_every_file`.
- Демо на системном шрифте: `demo --bench --system-font` — `font source: system — 'Cascadia Mono'`, GL 3.3; `full_table`/`overlay` с `--system-font --backend=soft --fail-frames=2` — кадр нарисован, штатный код возврата 1 после инжекции; `minimal --backend=soft --fail-frames=2` — то же (кадр рендерится, затем инжектированная ошибка).

## EVIDENCE
- Гейты после последнего изменения: `cargo fmt --check` чисто, `cargo clippy --all-targets -- -D warnings` чисто (оба пакета, пример включён), `cargo test` — **172 теста** harness (167 базовых − 1 перенесённый + 1 в демо + 3 `sources` + 2 `system`), 0 падений, 4 оконных бинарника — ok; `cargo test --test stage3_render golden` из корня — 3 теста ok.
- Размер release (Windows, профиль §10.2): `demo.exe` **1 522 688 Б** (+40 448), `full_table.exe` 1 515 520 Б (+39 424), `overlay.exe` 1 511 424 Б (+40 448); `examples/minimal.exe` 1 144 832 Б.
- Выбор системного шрифта (К5), машина владельца: 1 695 файлов в `C:\Windows\Fonts` + 57 в `%LOCALAPPDATA%` (2,4 ГБ), `fontdb` видит 1 709 лиц. Release, тёплый кэш ОС: 151,7 / 155,3 / 153,0 мс (`demo --bench --system-font` ×3), 158,0 мс с `--backend=soft`; debug — 292–309 мс; первый вызов в debug при холодном кэше — 11,98 с. Шрифт из `assets/` — 4,5 мс.
- `--bench` release: GL 3.3 на `assets/` — 1050,3 FPS; Software 120×40 на `Cascadia Mono` — 384,6 FPS (ячейка другая, с прежними замерами не сравнивается).
- LOC: `font.rs` 364, `font/system.rs` 84, `common.rs` 238, `assets.rs` 66, `examples/minimal.rs` 108, `tests/stage2_font.rs` 900 (тестовый файл: пять модулей по темам), `tests/common/mod.rs` 160.
- Проверочный проход BP-09: заново открыты 9 мест — номера строк функций `font.rs`/`system.rs` (`grep -n`), `wc -l`, хеши golden, сохранённые выводы `cargo test` и трёх release-прогонов, `ls -l target/release`, `Cargo.lock`, `git status`. Отброшенных находок нет.

## ⚠ WARNINGS
- Задержка выбора системного шрифта — 150–158 мс в release при тёплом кэше, выше порога ≈ 100 мс из плана; холодный кэш в release не замерялся (в debug — 12 с). Решение о запасном пути — за владельцем (запись в `TODO.md`).
- К5/К8/К13 глазами (окно на трёх бэкендах, копия `demo.exe` без `assets/`) и К9 (Linux, fontconfig) — за владельцем; автоматически проверены только stderr, код возврата и факт отрисовки кадра.
- `from_files` читает только первое лицо файла: семейство из коллекции `.ttc` не соберётся (у кандидатов по умолчанию коллекций нет).
- Путь каталога пакета (`CARGO_MANIFEST_DIR`) вшит в `demo.exe` как третий источник поиска `assets/`; у установленного exe он просто не существует.
- Строка «`Cargo.lock` есть, но репозиторий без коммитов» в `AGENTS.md` устарела ещё до задачи — не трогал (вне объёма).
- Коммитов не делалось (BP-08.4): перенос — 69 переименований плюс правки; предложение — два коммита (перенос, затем API и системный шрифт) либо один по решению владельца.
