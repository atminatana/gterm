# gterm

[English](README.md) | **Русский**

`gterm` — библиотека на Rust, которая открывает окно с сеткой символьных ячеек: текстовый экран
консоли для вашего приложения. Сетку рисует либо шейдер OpenGL, либо CPU-рендерер, и библиотека
переключается между ними сама: OpenGL 3.3 → OpenGL ES 3.0 → Software.

`gterm` — это экран, а не хост оболочки: в ней нет PTY и разбора escape-последовательностей.
Приложение пишет ячейки в `ScreenBuffer` и получает события клавиатуры и мыши.

## Возможности

- **Три бэкенда, одна картинка.** GL-бэкенд бит-идентичен Software-рендереру; это проверяют
  тесты на обоих уровнях GL. Если инициализация графики или кадр завершились ошибкой, библиотека
  не паникует, а откатывается на следующий бэкенд.
- **Один пиксель глифа = один пиксель кадра.** Без масштабирования и интерполяции: размер ячейки
  задают размер шрифта и DPI экрана.
- **Шрифты по требованию.** Ничего не вшито: семейство загружается из каталога, из файлов или
  берётся системный моноширинный шрифт. Глифы растеризуются при первом использовании (Regular,
  Bold, Italic, Bold Italic) в динамический атлас.
- **Процедурная псевдографика.** Рамки, блоки, секстанты, октанты и геометрические фигуры
  рисуются по формулам, поэтому рамки стыкуются без зазоров при любом размере. Вместо формул
  можно взять глифы шрифта.
- **SVG-глифы.** Собственные глифы из файлов `.svg`, привязанные к кодовым точкам.
- **Цвета с альфой** (`0xAARRGGBB`) и наложение клиентских буферов слоями.
- **Быстрый Software-рендерер.** Перерисовываются только изменённые строки; отрисовка идёт в
  пуле потоков и не выделяет память в кадре.
- **Комбинации клавиш.** Трекер аккордов и последовательностей вида `Ctrl+K`, `Ctrl+[K,U]`.
- **Тик приложения.** По заданному периоду библиотека зовёт `on_tick` без пользовательского
  ввода — для индикаторов и таймеров; без периода цикл в простое не просыпается.
- **Виджеты** (отдельный крейт `gterm-widgets`), немедленный режим по образцу ratatui: `Block`,
  `Paragraph`, `List`, `Table`, `Tabs`, `Gauge`, `LineGauge`, `Meter`, `Scrollbar`, `Popup`,
  `Button`, `Checkbox`, `Slider`, `TextInput`, `ComboBox`; раскладка, кольцо фокуса, мышь, модальные слои, карта
  сочетаний клавиш и буфер обмена. Двойной клик по любому контролу привязывается к сообщению, как
  клик кнопки (`on_double_click`); нажатия считает окно, часы контролам не нужны.
- **Темы.** `Theme` читает файлы тем редактора Helix (области, палитра, `inherits`); один вызов
  `Ui::theme(&theme)` красит экран, рамки, модальные слои и все контролы, а свои области
  приложение читает по имени. Теме достаточно основы — фона, текста и акцента: остальные тона
  и области виджетов выводятся, и любую из них можно задать по имени.

## Крейты

| Крейт | Путь | Назначение |
|---|---|---|
| `gterm` | [`crates/gterm`](crates/gterm) | Библиотека: буфер, шрифты, рендереры, окно и цикл событий |
| `gterm-widgets` | [`crates/gterm-widgets`](crates/gterm-widgets) | Виджеты и контролы поверх `gterm` |
| `gterm-demo` | [`crates/gterm-demo`](crates/gterm-demo) | Демо-бинарники и их ресурсы (шрифт, SVG-глифы) |

## Требования

- Rust 1.85 или новее (edition 2024).
- Windows или Linux (X11 и Wayland).
- OpenGL необязателен: без него работает Software-рендерер.

## Попробовать

```sh
git clone https://github.com/atminatana/gterm
cd gterm

cargo run --bin demo                                  # демо-окно, Esc — выход
cargo run -p gterm --example minimal                  # минимальный потребитель, системный шрифт
cargo run -p gterm --example pseudographica           # режимы псевдографики
cargo run -p gterm --example hotkeys                  # комбинации клавиш
cargo run -p gterm-widgets --example widgets          # экран из виджетов
cargo run -p gterm-widgets --example widget_edit      # форма: поля ввода, ползунок, флажки, выпадающий список
cargo run -p gterm-widgets --example widget_edit -- --theme=gterm_light   # с темой; F2 — следующая
cargo run -p gterm-widgets --example theme -- --theme-dir=<каталог>   # просмотр тем: перечень, элементы, образец
cargo run --bin full_table                            # таблица всех глифов
cargo run --bin overlay                               # наложение клиентских буферов
```

Бэкенд можно задать явно: `--backend=auto|gl|gles|soft` или переменная окружения `TERM_BACKEND`.

## Подключение к своему проекту

Добавьте крейты как git-зависимости (обе строки должны указывать на одну ревизию):

```toml
[dependencies]
gterm = { git = "https://github.com/atminatana/gterm" }
gterm-widgets = { git = "https://github.com/atminatana/gterm" }   # по желанию
```

Минимальное приложение:

```rust
use std::error::Error;

use gterm::app::{App, AppHandler, AppOptions};
use gterm::buffer::ScreenBuffer;
use gterm::cell::{CellFlags, Color};
use gterm::config::{BACKEND_ENV, Config};
use gterm::font::FontFamily;
use gterm::rasterizer::Rasterizer;

const FG: Color = Color::from_rgb(0xE0, 0xE0, 0xE0);
const BG: Color = Color::from_rgb(0x10, 0x18, 0x28);

struct Hello;

impl AppHandler for Hello {}

fn main() -> Result<(), Box<dyn Error>> {
	let env_backend = std::env::var(BACKEND_ENV).ok();
	let config = Config::parse(std::env::args().skip(1), env_backend.as_deref())?;

	let mut screen = ScreenBuffer::with_default_colors(40, 5, FG, BG);
	for (i, ch) in "Hello, gterm!".chars().enumerate() {
		screen.set_cell(2 + i, 2, ch, FG, BG, CellFlags::BOLD);
	}

	let family = FontFamily::system_monospace()?;
	App::new(
		"hello",
		screen,
		Rasterizer::new(family)?,
		config,
		AppOptions::default(),
	)
	.run(Hello)?;
	Ok(())
}
```

Полные программы — в [`crates/gterm/examples`](crates/gterm/examples) и
[`crates/gterm-widgets/examples`](crates/gterm-widgets/examples).

Профиль release этого workspace (`opt-level = "z"`, LTO, `panic = "abort"`) зависимым проектам
не передаётся — задайте профиль в своём `Cargo.toml`.

## Разработка

CI нет; перед отправкой изменений выполните три проверки:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Тесты запускаются в профиле debug: в профиле release включён `panic = "abort"`.

## Документация

- [`docs/TZ_terminal_emulator.md`](docs/TZ_terminal_emulator.md) — техническое задание;
- [`docs/adr/`](docs/adr/README.md) — архитектурные решения;
- [`PROJECT_MAP.md`](PROJECT_MAP.md) — карта проекта.

## Лицензия

Copyright (c) 2026 Korsakov Roman

Проект распространяется на условиях одной из двух лицензий — по вашему выбору:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) или
  <https://www.apache.org/licenses/LICENSE-2.0>);
- лицензия MIT ([LICENSE-MIT](LICENSE-MIT) или <https://opensource.org/licenses/MIT>).

Если вы явно не указали иное, любой вклад, намеренно предложенный вами для включения в проект
(в смысле лицензии Apache-2.0), лицензируется на тех же двух условиях без дополнительных
оговорок.

Шрифт демо в каталоге [`crates/gterm-demo/assets/fonts`](crates/gterm-demo/assets/fonts) —
сторонняя работа под собственной лицензией (Apache License 2.0), см.
[уведомление](crates/gterm-demo/assets/fonts/README.md) в этом каталоге.
