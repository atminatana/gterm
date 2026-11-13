# Отчёт о выполнении: мусор Software на KWin Wayland — непрозрачный старший байт буфера окна

- **Задача:** сообщение владельца 2026-10-24: «на Linux Wayland в режиме 3 при перерисовке не очищается альфа-канал, изображение накладывается на предыдущий кадр». Диагностика по DBG, без ТЗ и плана (исправление локальное).
- **Решение и его ПОЧЕМУ:** ADR-0024.

## DBG-07

~~~
PROBLEM:      Software на KWin 6.8 Wayland (масштаб 1.7): окно «полупрозрачно», кадры складываются, картинка белеет
FILE:LINE:    crates/gterm/src/app/backend.rs — копирование кадра в буфер окна softbuffer (Xrgb8888)
CHAIN BREAK:  [softbuffer: wl_shm Xrgb8888, старший байт 0x00] → [KWin: читает байт как предумноженную альфу]
EVIDENCE:
  - лог --debug владельца: все показы — «full copy», размеры буфера и кадра совпадают, age 0/0/2/2
  - пробный тест при ячейке 22×49: частичная перерисовка Software = кадр с нуля, 0 отличий
  - soft_band.rs: пиксель пишется присваиванием поверх однотонной подложки — накапливаться нечему
  - X11 с WINIT_X11_SCALE_FACTOR=1.7 — чисто; GL/GLES на том же KWin — чисто
  - гипотеза «повреждение i32::MAX как у Mesa» (коммит be6271a) — артефакты те же, отвергнута
  - зонд winit + softbuffer без gterm (коммит d17a8a2) — те же артефакты; с | 0xFF000000 — чисто
CAUSE:        encode_color даёт 0x00RRGGBB; KWin трактует нулевой старший байт Xrgb8888 как альфу 0
              при ненулевом цвете — чистое сложение с фоном и предыдущим содержимым
FIX:          на Wayland копировать кадр в буфер окна через pixel::copy_opaque (pixel | 0xFF000000);
              X11/Win32 — прежняя copy_from_slice; формат кадра не меняется
~~~

## Что сделано

| Файл | Изменение |
|---|---|
| `crates/gterm/src/renderer/pixel.rs` | `OPAQUE_BYTE`, `copy_opaque(dst, src)` с проверкой длин |
| `crates/gterm/src/app/backend.rs` | поле `SoftState::is_wayland`; `copy_to_window` в полном показе и в полосах; экспериментальное повреждение `i32::MAX` снято |
| `crates/gterm/src/app/chain.rs` | `software_on` определяет Wayland по `RawWindowHandle` |
| `crates/gterm/tests/soft_pixel_math.rs` | `copy_opaque_sets_top_byte_and_keeps_color`, `copy_opaque_panics_on_length_mismatch` |
| `crates/gterm/examples/shm_probe.rs`, `Cargo.toml` | зонд удалён (по решению владельца; воспроизведение — коммит `d17a8a2`) |
| Документы | ADR-0024 + индекс, `AGENTS.md` (подводные камни, размер), `PROJECT_MAP.md`, `TODO.md` |

Диагностические строки `--debug` в `present` и `resize_window` (коммит `9e5499a`) остаются по согласованию с владельцем.

## COMPLETENESS
- Исправление — код (`copy_opaque`, `copy_to_window`, `is_wayland`) и тест (`soft_pixel_math`, 2 теста); ветка Wayland в `present` без окна не тестируется — проверена владельцем зондом с тем же преобразованием.
- Зонд и снятая гипотеза удалены из кода.

## CORRECTNESS
- Мутация `*s | OPAQUE_BYTE` → `*s`: падает `copy_opaque_sets_top_byte_and_keeps_color` (исходник восстановлен, прогон после восстановления зелёный).
- На Windows ветка показа прежняя: `demo --backend=soft --debug` печатает «full copy», как до правки.
- Причина подтверждена владельцем: зонд с `0xFF` в старшем байте чист и устойчив при движении мыши и перемещении окна.

## EVIDENCE
- `cargo fmt --check` чисто; `cargo clippy --all-targets -- -D warnings` чисто; `cargo test` — **201 тест**, 0 падений (было 199), оконные бинарники ok; golden без изменений.
- Размер release Windows: `demo.exe` 1 542 656 Б (было 1 540 608 Б, +2 048 Б).
- Прочитано: `softbuffer` 0.4.8 `backends/wayland/{mod,buffer}.rs` (540 строк), фрагменты `x11.rs`/`win32.rs` с `present_with_damage`; `winit` 0.30.13 `wayland/window/state.rs` (viewport, `set_buffer_scale`, непрозрачная область); `soft_band.rs`, `pixel.rs`, `app/{backend,chain}.rs`.
- Проверочный проход BP-09: заново открыты 5 мест — вывод мутации, `grep -c "OPAQUE_BYTE;"` после восстановления (1), сводка `cargo test`, `ls -l target/release/demo.exe`, `git status`. Отброшенных находок нет.

## ⚠ WARNINGS
- Эффект исправления в самом `gterm` на KWin глазами — за владельцем (`git pull && cargo run --bin demo -- --backend=soft`); подтверждён пока зондом.
- Замер Software на Wayland после замены `memcpy` на OR по пикселю — за владельцем (`cargo run --release --bin demo -- --bench --backend=soft 2> bench.txt`).
- Причина внутри KWin 6.8 не локализована; сообщение в KDE/`softbuffer` — по желанию владельца, воспроизведение — `d17a8a2`.
- Строки `--debug` в `present`/`resize_window` оставлены по согласованию.
- Коммит не делался.
