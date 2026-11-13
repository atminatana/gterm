# План: сглаживание глифов — 8-бит покрытие, хинтинг skrifa, процедурная псевдографика

- **Решение:** `docs/adr/0002-glyph-coverage-hinting.md`; исследование `docs/research/2026-10-01-glyph-alpha.md`.
- **Рамки:** модуль шрифта (формат v2, растеризатор), Software-рендерер (целочисленное смешивание), новый модуль псевдографики, ТЗ, golden. **Не входит:** GL (этап 4), LUT контраста, растр под масштаб.

## Открытые решения

Нет.

## Шаги

| # | Шаг | Проверка |
|---|---|---|
| 1 | `Cargo.toml`: `fontdue` → `skrifa 0.47` (`std`) + `zeno 0.3` (`eval`, `std`), без default-features | сборка |
| 2 | `src/font.rs`: `FNT_FORMAT_VERSION = 2`, растр `coverage: Vec<u8>` (байт на пиксель), `coverage(slot,x,y)`, `set_coverage`; `Atlas` копирует значения | round-trip, тесты формата |
| 3 | `src/box_drawing.rs`: таблица сторон U+2500–U+257F (сгенерирована из имён Unicode), блоки/тени/квадранты U+2580–U+259F, `draw(cp, w, h, out) -> bool` | тесты: `─` — одна строка 255 на всю ширину, `│` — столбец, `┼` стыкуется, `█` — всё 255, `░` — 64, `▌` — левая половина |
| 4 | `src/font_gen.rs`: растеризация `skrifa` (`HintingInstance`, `Target::Mono`) → `zeno::Mask` (учесть `top + height` при `BottomLeft`); слоты псевдографики — процедурно, перекрывая шрифт; таблица кодовых точек = шрифт ∪ U+2500–U+259F | тесты этапа 2 (обновлённые): детерминизм, кириллица, кэш |
| 5 | `src/renderer/soft_backend.rs`: `a: u8` из атласа, `blend()` целочисленно; UNDERLINE/курсор `255`, BLINK `0` | пиксельные тесты через `blend`, golden перегенерировать и посмотреть |
| 6 | ТЗ: §1.2, FR-8, FR-8.3, FR-8.4, FR-11, §6.3, §7.3, §10.1, NFR-2; `AGENTS.md`; `PROJECT_MAP.md` | grep «fontdue», «1 бит» |
| 7 | Гейты fmt / clippy ×2 / test; размер release с профилем §10.2 (`--config`) | отчёты `docs/done|test|verify` |
