//! Комбинации клавиш (ADR-0028) на системном шрифте без `assets/`: трекер `gterm::hotkey`
//! кормится нажатиями и отпусканиями из `on_key`; на экране — что нажато сейчас, полная
//! комбинация живой последовательности, последняя завершённая, журнал сигналов и образцы из
//! строк с отметкой совпавших. Клавиши берутся по физическому коду: `Ctrl+K` совпадает и в
//! русской раскладке. `Esc`, когда ничего не удержано, — выход. Потеря фокуса окна сбрасывает
//! трекер (ADR-0029): отпускания вне окна до него не дойдут.
//! Запуск: `cargo run -p gterm --example hotkeys [-- --backend=gl|gles|soft]`.

use std::collections::VecDeque;
use std::error::Error;
use std::process::ExitCode;

use gterm::app::{App, AppHandler, AppOptions, Ctx, GridMode};
use gterm::buffer::ScreenBuffer;
use gterm::cell::{CellFlags, Color, Style};
use gterm::config::{BACKEND_ENV, Config};
use gterm::font::FontFamily;
use gterm::hotkey::{Hotkey, HotkeyEvent, HotkeyTracker};
use gterm::rasterizer::Rasterizer;
use winit::event::KeyEvent;
use winit::keyboard::{Key, NamedKey};

const COLS: usize = 72;
const ROWS: usize = 16;
const FG: Color = Color::from_rgb(0xE0, 0xE0, 0xE0);
const BG: Color = Color::from_rgb(0x10, 0x18, 0x28);
const DIM: Color = Color::from_rgb(0x80, 0x88, 0x98);
const ACCENT: Color = Color::from_rgb(0x60, 0xA0, 0xFF);
const HIT: Color = Color::from_rgb(0x60, 0xE0, 0x80);
const LOG_LINES: usize = 4;

/// Образцы: запись → подпись.
const PATTERNS: [(&str, &str); 6] = [
	("Ctrl+S", "сохранить"),
	("Ctrl+Shift+Z", "повторить"),
	("Ctrl+[K,U]", "аккорд K, U"),
	("K|U", "K или U"),
	("Space+[Ctrl+[K,U]]", "вложенный аккорд"),
	("Alt+F4|Ctrl+Q", "закрыть"),
];

fn main() -> ExitCode {
	let env_backend = std::env::var(BACKEND_ENV).ok();
	let config = match Config::parse(std::env::args().skip(1), env_backend.as_deref()) {
		Ok(config) if config.help => {
			println!("{}", Config::usage("hotkeys"));
			return ExitCode::SUCCESS;
		}
		Ok(config) => config,
		Err(e) => {
			eprintln!("hotkeys: {e}");
			return ExitCode::FAILURE;
		}
	};
	match run(config) {
		Ok(()) => ExitCode::SUCCESS,
		Err(e) => {
			eprintln!("hotkeys: {e}");
			ExitCode::FAILURE
		}
	}
}

fn run(config: Config) -> Result<(), Box<dyn Error>> {
	println!("hotkeys: searching for system monospace font (this may take a moment)...");
	let family = FontFamily::system_monospace()?;
	eprintln!(
		"hotkeys: font '{}' — {}",
		family.family(),
		family.face(Style::Regular).path.display()
	);
	let patterns = PATTERNS
		.iter()
		.map(|(text, label)| Ok((text.parse::<Hotkey>()?, *label)))
		.collect::<Result<Vec<_>, gterm::hotkey::ParseError>>()?;
	let screen = ScreenBuffer::with_default_colors(COLS, ROWS, FG, BG);
	let options = AppOptions {
		grid_mode: GridMode::FixedGrid,
		..AppOptions::default()
	};
	App::new(
		"gterm hotkeys",
		screen,
		Rasterizer::new(family)?,
		config,
		options,
	)
	.run(Demo {
		tracker: HotkeyTracker::new(),
		last_complete: None,
		log: VecDeque::new(),
		patterns,
		matched: Vec::new(),
	})?;
	Ok(())
}

struct Demo {
	tracker: HotkeyTracker,
	last_complete: Option<Hotkey>,
	/// Последние сигналы: запись и признак завершения.
	log: VecDeque<(String, bool)>,
	patterns: Vec<(Hotkey, &'static str)>,
	/// Подписи образцов, совпавших с последним сигналом.
	matched: Vec<&'static str>,
}

impl Demo {
	fn write(screen: &mut ScreenBuffer, row: usize, text: &str, fg: Color, flags: CellFlags) {
		for (col, ch) in text.chars().enumerate().take(COLS) {
			screen.set_cell(col, row, ch, fg, BG, flags);
		}
	}

	fn draw(&self, screen: &mut ScreenBuffer) {
		screen.clear_all();
		Self::write(
			screen,
			0,
			"gterm::hotkey — комбинации клавиш по физическим кодам (ADR-0028)",
			ACCENT,
			CellFlags::BOLD,
		);
		Self::write(
			screen,
			1,
			"Нажимайте сочетания; Esc при пустой комбинации — выход.",
			DIM,
			CellFlags::empty(),
		);
		let current = self.tracker.current().to_string();
		let full = self.tracker.full().to_string();
		let last = self
			.last_complete
			.as_ref()
			.map_or(String::new(), ToString::to_string);
		Self::write(
			screen,
			3,
			&format!("сейчас:     {current}"),
			FG,
			CellFlags::BOLD,
		);
		Self::write(
			screen,
			4,
			&format!("полная:     {full}"),
			FG,
			CellFlags::empty(),
		);
		Self::write(
			screen,
			5,
			&format!("последняя:  {last}"),
			FG,
			CellFlags::empty(),
		);
		Self::write(
			screen,
			7,
			"сигналы (hotkey, complete):",
			DIM,
			CellFlags::empty(),
		);
		for (i, (text, complete)) in self.log.iter().enumerate() {
			let mark = if *complete { "✔" } else { "·" };
			Self::write(
				screen,
				8 + i,
				&format!("  {mark} {text}"),
				FG,
				CellFlags::empty(),
			);
		}
		Self::write(screen, 13, "образцы:", DIM, CellFlags::empty());
		let mut line = String::new();
		for (hotkey, label) in &self.patterns {
			line.push_str(&format!("{hotkey} — {label};  "));
		}
		Self::write(screen, 14, &line, DIM, CellFlags::empty());
		let matched = if self.matched.is_empty() {
			"совпало: —".to_owned()
		} else {
			format!("совпало: {}", self.matched.join(", "))
		};
		Self::write(screen, 15, &matched, HIT, CellFlags::BOLD);
	}

	fn note(&mut self, event: HotkeyEvent) {
		self.matched = self
			.patterns
			.iter()
			.filter(|(pattern, _)| event.hotkey.matches(pattern))
			.map(|(_, label)| *label)
			.collect();
		if self.log.len() == LOG_LINES {
			self.log.pop_front();
		}
		self.log
			.push_back((event.hotkey.to_string(), event.complete));
		if event.complete {
			self.last_complete = Some(event.hotkey);
		}
	}
}

impl AppHandler for Demo {
	fn on_backend_changed(&mut self, _name: &'static str, ctx: &mut Ctx<'_>) {
		self.draw(ctx.screen);
		ctx.redraw();
	}

	fn on_key(&mut self, event: &KeyEvent, ctx: &mut Ctx<'_>) {
		// Трекер получает и нажатия, и отпускания (ADR-0028).
		let was_idle = !self.tracker.is_active();
		if let Some(signal) = self.tracker.feed(event) {
			self.note(signal);
		}
		if was_idle
			&& event.state == winit::event::ElementState::Pressed
			&& matches!(event.logical_key, Key::Named(NamedKey::Escape))
		{
			ctx.exit();
			return;
		}
		self.draw(ctx.screen);
		ctx.redraw();
	}

	fn on_focus(&mut self, focused: bool, ctx: &mut Ctx<'_>) {
		if !focused {
			self.tracker.reset();
			self.draw(ctx.screen);
			ctx.redraw();
		}
	}
}
