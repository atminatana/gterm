# Тесты: инжекция ошибок рендера и политика §5.3 п.5 (этап 8)

## Состав

`tests/stage8_faults.rs` — оконный бинарник (`harness = false`), 5 сценариев; каждый — отдельный процесс (`--scenario=…`), родитель сверяет код возврата и stderr:

| Сценарий | `Config` | Проверяет в процессе сценария | Проверяет родитель по stderr |
|---|---|---|---|
| `retry` | `fail_frames = 1` | бэкенд один; сетка, тики, обе фазы, `on_frame` = кадрам | `retrying once` ×1, `injected by --fail-frames`; нет `switching to Software` |
| `fallback` | `fail_frames = 2` | `[GL-уровень, Software]`, `App::run` → `Ok`; сетка, тики, кадры | `retrying once` ×1, `switching to Software` |
| `context-lost` | `lose_context` | то же | `context lost`, `switching to Software`; нет `retrying once` |
| `software-stops` | `backend = Soft`, `fail_frames = 2` | `[Software]`, `App::run` → `AppError::Render`, кадров 0, `on_frame` 0 | `retrying once` ×1; нет `switching to Software` |
| `chain-end` | `fail_frames = 4` | `[GL-уровень, Software]`, `App::run` → `AppError::Render`, кадров 0 | `retrying once` ×2, `switching to Software` |

`tests/stage5_config.rs` — +1 тест `fault_injection_flags`: оба написания `--fail-frames`, `--lose-context[=false]`, `MissingValue`, `BadNumber` для `many`, `-1` и пустого значения; умолчания и справка дополнены.

## Прогон (2026-10-04, Windows 10, NVIDIA GTX 1060, stable 1.98.1)

```
cargo fmt --check                          → чисто
cargo clippy --all-targets -- -D warnings  → чисто
cargo test                                 → 96 passed (9 + 42 + 20 + 1 + 8 + 9 + 7), 0 failed; 4 оконных бинарника — ok

cargo test --test stage8_faults
  инжекция ошибок кадра, retry: ok
  инжекция ошибок кадра, fallback: ok          (OpenGL 3.3 → Software)
  инжекция ошибок кадра, context-lost: ok      (OpenGL 3.3 → Software)
  инжекция ошибок кадра, software-stops: ok
  инжекция ошибок кадра, chain-end: ok

demo --backend=soft --fail-frames=2 → "frame failed (…injected by --fail-frames); retrying once",
                                      "gterm: render failed: …", код возврата 1
demo --fail-frames=abc              → "--fail-frames: expected a non-negative integer, got 'abc'" + usage, код 1
demo --help                         → справка с обоими флагами, код 0
cargo build --release               → demo.exe 1 080 832 Б, full_table.exe 1 070 592 Б
```

Базовый прогон до правок: 95 passed + 3 оконных бинарника.

## Ограничения

- Без окна каждый сценарий печатает причину и проходит («пропуск»); без GL пропускаются `fallback`, `context-lost`, `chain-end`.
- Откат проверен с уровня GL 3.3; с GLES 3.0 (`--backend=gles --fail-frames=2`) — не прогонялся.
- Пиксели кадра после повтора и отката не сравниваются.
- `demo --fail-frames=2` и `demo --lose-context` на GL вручную не запускались (окно ждёт `Esc`) — тот же путь покрыт сценариями `fallback` и `context-lost`; глазами — на машине владельца.
- Время прогона `stage8_faults` тест-раннер не сообщает — время не измерено.
