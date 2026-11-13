# Верификация: этап 4 — GL-бэкенд

| Критерий ТЗ | Статус | Подтверждение |
|---|---|---|
| FR-12 один draw call, полноэкранный треугольник, логика в фрагментном шейдере | ✅ | `draw`: единственный `draw_arrays(TRIANGLES, 0, 3)`; `cell.vert` по `gl_VertexID`; VAO пустой |
| FR-13 `NEAREST`, целочисленный масштаб, пиксель-в-пиксель | ✅ | `set_nearest_clamp`; `texelFetch`; тест 0 отличий при scale 1 и 2 |
| FR-14 рамка | ✅ | `u_border` в шейдере; сцена 3 |
| FR-15 мигание через uniform без обновления данных | ✅ | `u_blink`; смена фазы меняет только `last_view` → полная перезагрузка данных (не атласа) |
| FR-16 подчёркивание/курсор по координатам в ячейке | ✅ | `cell.frag`; сцена 2 с курсором Underline |
| FR-17 порядок GL 3.3 core → GLES 3.0 → Software | ✅ (код) | `GlContext::new` две попытки; `App::try_gl` → Software; третий уровень в тесте не воспроизводим |
| FR-18 откат при ошибке конфигурации/контекста/шейдера/текстуры/`glGetError` первого кадра | ✅ (код) | все пути — `RenderError`; `check_gl_error` после init и первого кадра |
| FR-20 бэкенд и причина в stderr | ✅ | `create_window_and_backend` |
| §6.1 формат данных | ✅ | `upload_data`: R = слот \| флаги « 16, G = fg, B = bg, A = 0 |
| §6.2 dirty-строки, переиспользуемый буфер, пересоздание при `resize` сетки, лимит текстуры | ✅ | `data_staging`, `recreate_data_texture` с проверкой `GL_MAX_TEXTURE_SIZE` |
| §6.3 атлас `RGBA8UI`, `NEAREST`, `CLAMP`, `UNPACK_ALIGNMENT 1` | ✅ | `init`, `upload_*` |
| §7.1 один исходник, два заголовка | ✅ | `HEADER_GL` / `HEADER_GLES` + `include_str!` |
| §7.2 uniform'ы | ✅ | все из таблицы + `u_viewport_h`, `u_atlas_cols`, `u_style_channel` (вписаны в ТЗ) |
| §7.3 алгоритм | ✅ | `cell.frag` построчно соответствует псевдокоду; 0 отличий от CPU |
| §9 п.1–7 | ✅ | шаблон конфигурации, две попытки API, поверхность, current, VSync без паники, `glow`, проверки, лог |
| §11.2 допуск 0, пропуск без GL | ✅ | `stage4_gl`; 3 сцены × 2 кадра |
| NFR-6 горячий путь | ✅ (чтение) | в `render`: `prepare_glyphs` → `upload_*` в предвыделенные буферы → uniform'ы → draw; `u_style_channel` — массив на стеке; аллокаций в `render` нет |
| NFR-7 | ✅ | нет `unwrap`/паник на путях GL; `expect` только для `NonZeroU32::new(…max(1))` и выбора конфига из непустого итератора |
| NFR-9 | ✅ | `unsafe`-блоки только в `gl_backend.rs` (15, каждый с SAFETY-комментарием); в `renderer/mod.rs` слово встречается лишь в doc-комментарии |
| §4.5 гейты | ✅ | fmt / clippy / 53 теста |
| Переоткрытые места | — | `gl_backend.rs`, `cell.frag`, `app.rs`, `stage4_gl.rs` — целиком; сравнение кадров — инструментом (тест) |

Расхождений с ТЗ не найдено. Открытые пункты (реальный отказ GL, GLES-путь, FR-21) — в ⚠ WARNINGS `docs/done/`.
