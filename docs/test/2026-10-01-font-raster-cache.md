# Тесты: этап 2 — растр шрифта из TTF с кэшем `.fnt`

## Состав (`tests/stage2_font.rs`, 17 тестов)

| Модуль | Тест | Проверяет |
|---|---|---|
| `fnt_format` | `round_trip_preserves_header_table_and_bits` | сериализация ↔ разбор, `bytes_per_row` для w=9 |
| | `rejects_bad_magic_version_and_truncation` | `BadMagic`, `UnsupportedVersion`, `Truncated` |
| | `rejects_unsorted_codepoint_table` | `BadCodepointTable` (не возрастает; слот 0 ≠ 0) |
| | `slot_lookup_finds_present_and_misses_absent` | бинарный поиск слота, `EMPTY_SLOT` |
| | `fnv1a64_matches_reference_vectors` | эталонные векторы FNV-1a 64 (`""`, `"a"`) |
| `rasterization` | `repo_font_produces_cell_sized_glyphs_for_all_bmp_codepoints` | > 700 слотов, таблица возрастает, константы в заголовке |
| | `latin_cyrillic_and_box_drawing_have_ink_space_and_absent_do_not` | `A`, `А`, `─` непустые; пробел и U+FFFF пустые |
| | `rasterization_is_deterministic` | два прогона → одинаковые байты |
| | `garbage_ttf_is_a_typed_error_not_a_panic` | `TtfParse`, без паники (NFR-7) |
| `cache` (временный каталог с копией TTF) | `missing_cache_is_created_with_matching_header` | FR-8.2: нет файла → `Regenerated`, файл на диске == данные |
| | `matching_cache_is_reused_without_rewrite` | подложенный растр при верном заголовке возвращается → `Reused` |
| | `cache_with_other_size_is_regenerated` | иной `size_px` → `Regenerated`, данные == эталон |
| | `cache_with_other_ttf_hash_or_name_is_regenerated` | иной хеш / имя → `Regenerated` |
| | `corrupted_cache_is_regenerated` | мусор в файле → `Regenerated` |
| | `missing_ttf_is_an_io_error` | FR-8.5: `Io` |
| `atlas` | `atlas_is_16_slots_wide_and_places_slot_by_formula` | ширина 16·w, слот `(s%16, s/16)` |
| | `atlas_slot_lookup_and_glyph_copy_follow_font_table` | `slot_of` совпадает с `FontData`, `glyph()` |

`tests/stage3_render.rs` (3 теста, унаследованные) — адаптированы под `SoftBackend::new(atlas)`, логику не проверяют глубже «не упало».

## Прогон

```
cargo test                 → 27 passed, 0 failed  (stage2: 0.30 s; всего < 1 с)
cargo test --features gl   → 27 passed, 3 ignored
cargo clippy --all-targets -- -D warnings             → чисто
cargo clippy --all-targets --features gl -- -D warnings → чисто
cargo fmt --check          → чисто
cargo run  ×2              → "cache regenerated" затем "cache reused", exit 0
```

## Ограничения

- `RegeneratedUnsaved` (невозможность записи кэша) — только ручная проверка.
- `find_assets_dir` — только через `cargo run` (cwd-ветка).
- Кэш-тесты пишут во `%TEMP%/gterm-stage2-*`, каталог удаляется в `Drop`; при аварийном завершении теста может остаться мусор в temp.
- Golden-сравнение рендера и бит-идентичность GL↔Soft — этапы 3–4, здесь не проверяются.
