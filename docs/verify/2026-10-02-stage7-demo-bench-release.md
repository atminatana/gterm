# Верификация: этап 7 — демо FR-27, замеры `--bench`, профиль release

| Критерий ТЗ | Статус | Подтверждение |
|---|---|---|
| FR-27 строка статуса: имя бэкенда, FPS, время CPU на кадр, обновление по тику | ✅ (код) | `common::write_status`, `FpsMeter::update` в `Demo::on_tick` и `FullTable::on_tick`; `stats.frames` растёт в оконных тестах; глазами — ⚠ |
| FR-27 курсор демо в противофазе с `BLINK` по фазе из `Ctx` | ✅ (код) | `Demo::sync_cursor_phase`: `cursor_visible = no_blink \|\| !ctx.blink_phase`; фаза в `Ctx` чередуется (`[true, false, true]`); глазами — ⚠ |
| FR-27 прокрутка исключена | ✅ | ТЗ 1.4, решение владельца 2026-10-02 |
| FR-27 `--bench`: 200×60 на GL/GLES, 120×40 на Software, статистика в stderr, код 0 | ✅ | `bench.rs`; три прогона release, коды 0; отчёт в `docs/test` |
| FR-27.1 `full_table`: все глифы постранично, `PgUp`/`PgDn`/`Home`/`End`, 16×16 цветов, клавиши FR-28 | ✅ (код) | `src/bin/full_table/main.rs`; smoke release: 5019 глифов, 3 страницы (Mono v3.5.1), OpenGL 3.3; листание глазами — владелец смотрел 2026-10-03 |
| FR-8 шрифт семейства, вариант Mono: иконки помещаются в ячейку (FR-9, FR-13) | ✅ | `FONT_FAMILY = "RobotoMono Nerd Font Mono"`; диагностика: 2 иконки из 3715 выходят на 0,6 px (было 2000 из 2321, до 9,5 px); golden без изменений |
| FR-28 `1/2/3`, `F11`, `Esc` в обоих бинарниках | ✅ | `common::handle_common_key` вызывается из `Demo::on_key` и `FullTable::on_key`; `demo --fail-gl` → Software |
| FR-7 (дефект этапа 6) фазы `BLINK` чередуются после каждого дедлайна | ✅ | `Blinker::odd`; `catch_up_on_an_odd_number_of_phases_keeps_the_parity`; оконные тесты видят обе фазы (до исправления — 23 × `true`) |
| NFR-1 размер замерен и записан; лимита нет | ✅ | ТЗ 1.4; `demo.exe` 1 078 272 Б, `full_table.exe` 1 068 544 Б (Windows); Linux — ⚠ не перемерен |
| NFR-4 200×60 на GL ≤ 1 мс CPU на кадр | ✅ | GL 3.3: mean 0,578 мс, p95 0,629; GLES 3.0: mean 0,568, p95 0,623 (без VSync — ADR-0006) |
| NFR-5 120×40 Software ≥ 60 FPS при полной перерисовке | ✅ | 97,9 FPS, `total` mean 9,97 мс |
| NFR-6 нет аллокаций в горячем пути | ✅ | замер — два-три `Instant::now()` на кадр; `FrameStats` — `Copy`; `Vec` статистики — только в `Bench` (приложение, не рендер) |
| NFR-7 не паниковать при ошибках графики | ✅ | новых `unwrap`/`expect` в библиотеке нет; ошибочный кадр не попадает в статистику и не вызывает `on_frame` |
| NFR-9 `unsafe` только в `gl_backend.rs` | ✅ | `grep -rl unsafe src/` → `gl_backend.rs` и слово в doc-комментарии `renderer/mod.rs` |
| §5.1 структура `src/bin/demo/`, `src/bin/full_table/` | ✅ | файлы на месте, ТЗ 1.4 §5.1 обновлён |
| §5.2 `FrameStats`, `Ctx { screen, request, blink_phase, stats }`, `on_frame` | ✅ | `app.rs`; ТЗ 1.4 §5.2; ADR-0006 |
| §7.3 / §8 бит-идентичность GL ↔ Software не затронута | ✅ | `stage4_gl`: 4 сцены × 2 уровня, 0 отличий; golden без изменений |
| §10.2 профиль release целиком | ✅ | `Cargo.toml`; `cargo build --release` чисто |
| §11.4 п.1 без ошибок и предупреждений clippy | ✅ | `fmt --check`, `clippy --all-targets -- -D warnings` чисто (stable 1.98.1); две унаследованные правки |
| §11.4 п.2 размер записан в отчёт | ✅ | `docs/test/…stage7` |
| §11.4 п.3 демо на GL и Soft, кадры совпадают | ✅ (программно) | smoke `demo`, `demo --fail-gl`; совпадение — `stage4_gl` |
| §11.4 п.4 `--fail-gl` | ✅ | stderr: оба GL-уровня отвергнуты, `backend Software` |
| §12 этап 7 | ✅ | демо, размер (замер), производительность (замер) — все три пункта содержания закрыты |
| §15 п.5 отчёт о размере и производительности | ◐ | таблицы Windows в `docs/test`; сводная таблица по конфигурациям — этап 8 |
| Переоткрытые места | — | `app.rs` (изменённые блоки), `blink.rs`, `gl_backend.rs` (опции, `render`), все пять файлов `src/bin/`, `events_common`, четыре тестовых бинарника, `Cargo.toml`, ТЗ (все правленые разделы), ADR-0006 — целиком; stderr всех прогонов прочитан; LOC по `wc -l` |

Расхождений с ТЗ 1.4 не найдено. Непроверенное в этом окружении (визуальная часть, Linux-замеры, высокий DPI) — в ⚠ WARNINGS `docs/done/`.
