# Тесты: этап 5 — цепочка fallback, CLI/ENV

## Состав

`tests/stage5_config.rs` — 7 тестов:

| Тест | Проверяет |
|---|---|
| `defaults_are_auto_with_gl_allowed` | умолчания |
| `backend_from_cli_in_both_spellings_and_aliases` | `--backend=X`, `--backend X`, регистр, синонимы `opengl`/`software` |
| `backend_from_env_and_cli_overrides_env` | `TERM_BACKEND`, приоритет аргумента |
| `flags_and_booleans` | `--fail-gl`, `--debug`, `--allow-sw-gl=false`, `=0`/`=yes`, `-h`/`--help` |
| `errors_are_typed_and_name_the_source` | `BadBackend` (CLI и ENV), `MissingValue`, `BadBool`, `UnknownArgument`, `Display` |
| `chains_start_at_requested_level_and_end_in_software` | цепочки всех четырёх значений заканчиваются Software; имена уровней |
| `usage_mentions_every_option` | справка упоминает каждую опцию и переменную |

`tests/stage4_gl.rs` — расширен: оба GL-уровня в одном цикле событий.

## Прогон

```
cargo test                 → 60 passed (8 + 24 + 20 + 1 + 7)
cargo test --test stage4_gl -- --nocapture
  OpenGL 3.3:    GL ↔ Software identical, 3 scenes
  OpenGL ES 3.0: GL ↔ Software identical, 3 scenes   (контекст: OpenGL ES 3.2 NVIDIA 581.57)
cargo clippy --all-targets -- -D warnings → чисто;  cargo fmt --check → чисто

demo --fail-gl       → "OpenGL 3.3 unavailable: … forced failure (--fail-gl)",
                       "OpenGL ES 3.0 unavailable: …", "backend Software"; окно работает
demo --backend=soft  → "backend Software"
TERM_BACKEND=soft    → "backend Software"
demo --backend=gles  → "backend OpenGL ES 3.0 — NVIDIA … OpenGL ES 3.2 NVIDIA 581.57"
demo --debug         → GL 3.3, заголовок "gterm [OpenGL 3.3]"
demo --backend=vulkan → "--backend: unknown backend 'vulkan' (expected auto|gl|gles|soft)" + usage
demo --frob          → код возврата 1
demo --help          → usage, код 0
release (--config профиль §10.2) → 1 051 136 байт
```

## Ограничения

- Потеря контекста (FR-21) и повтор кадра (§5.3 п.5) — без автотестов и без воспроизведения.
- `--allow-sw-gl=false` — без прогона на `llvmpipe`.
- Сценарии демо проверены по stderr и факту работы окна до таймаута; визуально — владельцем.
