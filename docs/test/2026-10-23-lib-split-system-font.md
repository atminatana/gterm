# Отчёт о тестах: библиотека `gterm` без `assets/`; системный моноширинный шрифт

- **Задача:** `2026-10-23-lib-split-system-font` (ADR-0017)
- **Окружение:** Windows 10 Pro 19045, NVIDIA GTX 1060 3GB (драйвер 581.57), Rust stable 1.98.1; тесты — debug, замеры — release (`opt-level = "z"`); в системе 1 709 лиц шрифтов (`C:\Windows\Fonts` 1 695 файлов, 2,4 ГБ + 57 в `%LOCALAPPDATA%`)

## Состав

| Файл | Тесты |
|---|---|
| `crates/gterm/tests/stage2_font.rs`, модуль `family` (4) | прежние тесты на новой сигнатуре `load(fonts_dir, FAMILY)`: четыре начертания по `OS/2`, цепочка подстановки на временных каталогах с подмножеством файлов, нет Regular → `NoRegularFace`, нет каталога → `Io` |
| `stage2_font.rs`, модуль `sources` (3) | чужое имя семейства → `NoRegularFace { family, found: 4 файла }`; **К3** — `from_files` ↔ `load`: файлы по начертаниям, метрики, растр `A` Regular и `Я` Bold бит в бит; **К4** — копия Regular с расширенной `hhea.numberOfHMetrics` и половинным advance `i` → `NotMonospace { glyph: 'i', advance < expected }` |
| `stage2_font.rs`, модуль `system` (2) | **К5/К7** — `system_monospace()` дважды: то же семейство, те же файлы по начертаниям, те же метрики; время первого вызова в stderr; **К6** — `system("No Such Family")` → `NoRegularFace` с пустым `found`, `system(<имя семейства умолчания>)` даёт тот же Regular-файл, что умолчание. Без моноширинного шрифта в системе — печать причины и выход (как `stage4_gl` без GL) |
| `crates/gterm-demo/src/assets.rs` (1 юнит) | `find_assets_dir()` из cwd пакета находит `crates/gterm-demo/assets`, в нём есть `fonts/` |
| прежние (162 + 4 оконных) | golden `stage3_render`, `stage4_gl` (8 сцен, GL 3.3 и GLES 3.0), `stage6_app`, `overlay`, `soft_*`, `svg_glyphs`, `stage1/5`, юнит-тесты `full_table`; оконные `stage6_events`, `stage6_events_auto`, `stage6_fallback`, `stage8_faults` — шрифт из `../gterm-demo/assets/fonts` |

## Результат прогона

- `cargo test` из корня workspace: **172 теста** harness, 0 падений; четыре оконных бинарника — ok. `stage2_font`: 48 (`family` 4 + `sources` 3 + `system` 2 + растеризация/псевдографика/атлас 39).
- `stage2_font::system::system_monospace_is_deterministic` (stderr): `system_monospace: 'Cascadia Mono' C:\Windows\Fonts\CascadiaMono-Regular.otf in 11.98 s` при первом (холодном) запуске debug; повторные — ≈ 0,3 с.
- Golden: хеши `soft_golden_{16,32}px.bin` равны `HEAD` (`git hash-object` ↔ `git rev-parse HEAD:…`), эталоны не перезаписывались.
- Мутации (исходник восстановлен, прогон после восстановления зелёный): отключённая проверка моноширинности → падает `proportional_font_is_rejected`; снятый фильтр по имени семейства в `from_files` → падает `wrong_family_name_is_no_regular_face_listing_every_file`.
- Гейты: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` — чисто для обоих пакетов и примера.

## Запуски (К5, К8, К13 — автоматизируемая часть)

| Команда | stderr / код возврата |
|---|---|
| `cargo run --bin demo -- --bench` (debug и release) | `font source: assets D:\…\crates\gterm-demo\assets — 'RobotoMono Nerd Font Mono' in 4.5 ms`; GL 3.3; release 1050,3 FPS |
| `target\release\demo.exe --bench --system-font` ×3 | `font source: system — 'Cascadia Mono'` in 151,7 / 155,3 / 153,0 мс; файлы `C:\Windows\Fonts\CascadiaMono-{Regular,Bold,…}.otf` |
| `target\release\demo.exe --bench --system-font --backend=soft` | 158,0 мс; Software 120×40 — 384,6 FPS |
| `cargo run --bin full_table -- --system-font --backend=soft --fail-frames=2` | источник `system`, кадр нарисован, инжекция → код 1 (штатно, ADR-0007) |
| `cargo run --bin overlay -- --system-font --backend=soft --fail-frames=2` | то же |
| `cargo run -p gterm --example minimal -- --backend=soft --fail-frames=2` | `minimal: font 'Cascadia Mono' — C:\Windows\Fonts\CascadiaMono-Regular.otf`, `backend Software`, кадр нарисован, инжекция → код 1 |
| `cargo run --bin demo -- --help` | строка `--system-font    системный моноширинный шрифт вместо assets/ (ADR-0017)` |

Размер release: `demo.exe` 1 522 688 Б (+40 448 к 1 482 240), `full_table.exe` 1 515 520 Б, `overlay.exe` 1 511 424 Б, `examples/minimal.exe` 1 144 832 Б.

## Ограничения

- Окно глазами (сцена на `Cascadia Mono` на трёх бэкендах, `minimal`, копия `demo.exe` в пустом каталоге) — на машине владельца.
- Linux/fontconfig (К9) не проверялся: нет Linux-машины в этом окружении.
- Тесты `system::*` зависят от окружения: на машине без моноширинного шрифта они печатают причину и завершаются успешно.
- Release при холодном кэше ОС не замерялся; известен только debug — 12 с.
