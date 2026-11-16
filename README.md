# gterm

**English** | [Русский](README.ru.md)

`gterm` is a Rust library that opens a window with a grid of character cells — a text-mode
console screen for your application. The grid is drawn either by an OpenGL shader or by a CPU
renderer, and the library switches between them automatically: OpenGL 3.3 → OpenGL ES 3.0 →
Software.

`gterm` is the screen, not a shell host: it has no PTY and no escape-sequence parser. The
application writes cells into a `ScreenBuffer` and receives keyboard and mouse events.

## Features

- **Three backends, one picture.** The GL backend is bit-identical to the Software renderer;
  this is checked by tests on both GL levels. If graphics initialisation or a frame fails, the
  library falls back to the next backend instead of panicking.
- **One glyph pixel = one frame pixel.** No scaling and no interpolation: the cell size comes from
  the font size and the display DPI.
- **Fonts on demand.** Nothing is embedded: load a font family from a directory, from files, or
  take a system monospace font. Glyphs are rasterised when first used (Regular, Bold, Italic,
  Bold Italic) into a dynamic atlas.
- **Procedural pseudographics.** Box drawing, block elements, sextants, octants and geometric
  shapes are drawn by formula, so frames join without gaps at any size. Font glyphs can be used
  instead.
- **SVG glyphs.** Your own glyphs from `.svg` files, mapped to code points.
- **Colours with alpha** (`0xAARRGGBB`) and overlay of client buffers in layers.
- **Fast Software renderer.** Only changed rows are redrawn; rendering runs in a thread pool and
  allocates nothing per frame.
- **Key combinations.** A tracker for chords and sequences such as `Ctrl+K`, `Ctrl+[K,U]`.
- **Application tick.** With a period set, the library calls `on_tick` without user input — for
  meters and timers; with no period the idle loop never wakes up.
- **Widgets** (separate crate `gterm-widgets`), immediate mode in the style of ratatui: `Block`,
  `Paragraph`, `List`, `Table`, `Tabs`, `Gauge`, `LineGauge`, `Meter`, `Scrollbar`, `Popup`,
  `Button`, `Checkbox`, `Slider`, `TextInput`, `ComboBox`; layout, focus ring, mouse, modal layers, a hotkey map and
  the clipboard. A double click on any control is bound to a message like a button click
  (`on_double_click`); the window counts the presses, controls need no clock.

## Crates

| Crate | Path | Purpose |
|---|---|---|
| `gterm` | [`crates/gterm`](crates/gterm) | The library: buffer, fonts, renderers, window and event loop |
| `gterm-widgets` | [`crates/gterm-widgets`](crates/gterm-widgets) | Widgets and controls on top of `gterm` |
| `gterm-demo` | [`crates/gterm-demo`](crates/gterm-demo) | Demo binaries and their assets (font, SVG glyphs) |

## Requirements

- Rust 1.85 or newer (edition 2024).
- Windows or Linux (X11 and Wayland).
- OpenGL is optional: without it the Software renderer is used.

## Try it

```sh
git clone https://github.com/atminatana/gterm
cd gterm

cargo run --bin demo                                  # demo window, Esc to quit
cargo run -p gterm --example minimal                  # smallest consumer, system font
cargo run -p gterm --example pseudographica           # pseudographics modes
cargo run -p gterm --example hotkeys                  # key combinations
cargo run -p gterm-widgets --example widgets          # a screen built from widgets
cargo run -p gterm-widgets --example widget_edit      # a form: inputs, slider, checkboxes, combo box
cargo run --bin full_table                            # table of all glyphs
cargo run --bin overlay                               # overlay of client buffers
```

The backend can be forced with `--backend=auto|gl|gles|soft` or the `TERM_BACKEND` environment
variable.

## Use in your project

Add the crates as git dependencies (both lines must point to the same revision):

```toml
[dependencies]
gterm = { git = "https://github.com/atminatana/gterm" }
gterm-widgets = { git = "https://github.com/atminatana/gterm" }   # optional
```

A minimal application:

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

Complete programs are in [`crates/gterm/examples`](crates/gterm/examples) and
[`crates/gterm-widgets/examples`](crates/gterm-widgets/examples).

The release profile of this workspace (`opt-level = "z"`, LTO, `panic = "abort"`) is not
inherited by dependants — set the profile in your own `Cargo.toml`.

## Development

There is no CI; run the three checks before submitting a change:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Run the tests in the debug profile: the release profile uses `panic = "abort"`.

## Documentation

The project documentation is written in Russian:

- [`docs/TZ_terminal_emulator.md`](docs/TZ_terminal_emulator.md) — the specification;
- [`docs/adr/`](docs/adr/README.md) — architecture decision records;
- [`PROJECT_MAP.md`](PROJECT_MAP.md) — the map of the project.

## License

Copyright (c) 2026 Korsakov Roman

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without
any additional terms or conditions.

The demo font in [`crates/gterm-demo/assets/fonts`](crates/gterm-demo/assets/fonts) is a
third-party work under its own license (Apache License 2.0) — see the
[notice](crates/gterm-demo/assets/fonts/README.md) in that directory.
