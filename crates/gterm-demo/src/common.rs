//! Общая обвязка демо-бинарников `demo`, `full_table` и `overlay` (ТЗ §5.1, FR-27, FR-27.1,
//! FR-28): шрифт и его лог, флаги демо поверх `Config`, клавиши FR-28, строка статуса с именем
//! бэкенда и статистикой кадров, счётчик FPS по разности кадров между событиями (ADR-0006).

use std::error::Error;
use std::fmt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use gterm::app::{AppOptions, Ctx, FrameStats, GridMode};
use gterm::buffer::ScreenBuffer;
use gterm::cell::{Color, Style};
use gterm::config::Config;
use gterm::font::{FONT_SCALE, FontFamily, glyph_px_for};
use gterm::rasterizer::Rasterizer;
use gterm::renderer::RendererKind;
use gterm::svg::SvgGlyphs;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, NamedKey};

use crate::assets::{FONT_FAMILY, find_assets_dir, fonts_dir, svg_dir};

/// Цвета строки статуса (строка 0).
pub const STATUS_FG: Color = Color::WHITE;
pub const STATUS_BG: Color = Color::from_rgb(0, 0, 0x80);
/// Цвет подсказки по клавишам.
pub const HELP_FG: Color = Color::from_rgb(0x80, 0x80, 0x80);

/// Снять флаг демо из аргументов до разбора `Config` — библиотека его не знает.
pub fn take_flag(args: &mut Vec<String>, flag: &str) -> bool {
	let before = args.len();
	args.retain(|a| a != flag);
	args.len() != before
}

/// Разобрать конфигурацию библиотеки. `--help` печатает справку и даёт `Ok(None)`; ошибка
/// разбора возвращается текстом вместе со справкой.
pub fn parse_config(args: Vec<String>, usage: &str) -> Result<Option<Config>, String> {
	let env = std::env::var(gterm::config::BACKEND_ENV).ok();
	match Config::parse(args, env.as_deref()) {
		Ok(config) if config.help => {
			println!("{usage}");
			Ok(None)
		}
		Ok(config) => Ok(Some(config)),
		Err(e) => Err(format!("{e}\n{usage}")),
	}
}

/// Логический размер шрифта `demo` и `full_table` (ADR-0008 п.8): крупнее библиотечного
/// `FONT_SIZE_PX`, чтобы SVG-тайлы были различимы. `--bench` остаётся на `FONT_SIZE_PX` —
/// иначе сетка замера не помещается на экран и замеры NFR-4/NFR-5 несравнимы с этапом 7.
pub const DEMO_FONT_SIZE_PX: u16 = 22;
/// Флаг демо: системный моноширинный шрифт вместо `assets/` (ADR-0017).
pub const SYSTEM_FONT: &str = "--system-font";

/// Источник шрифта демо (ADR-0017): `assets/` рядом с exe или в репозитории, иначе системный
/// моноширинный; `--system-font` — принудительно системный.
#[derive(Debug)]
pub enum FontSource {
	/// Семейство `FONT_FAMILY` из `assets/fonts/`, SVG-глифы из `assets/svg/`.
	Assets(PathBuf),
	/// `FontFamily::system_monospace()`; иконок Nerd Font и SVG-тайлов нет.
	System,
}

impl fmt::Display for FontSource {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Assets(dir) => write!(f, "assets {}", dir.display()),
			Self::System => f.write_str("system"),
		}
	}
}

/// Выбор источника шрифта: `force_system` — флаг [`SYSTEM_FONT`].
pub fn font_source(force_system: bool) -> FontSource {
	if force_system {
		return FontSource::System;
	}
	find_assets_dir().map_or(FontSource::System, FontSource::Assets)
}

/// Справка: ключи библиотеки плюс общий ключ демо-бинарников `--system-font`.
pub fn usage(program: &str) -> String {
	format!(
		"{}\n\
         \x20 --system-font    системный моноширинный шрифт вместо assets/ (ADR-0017)",
		Config::usage(program)
	)
}

/// Семейство из источника с логом источника и времени его выбора, выбранных файлов и логической
/// ячейки при `font_size_px` (FR-8). Физический размер выведет DPI окна при создании (FR-24,
/// ADR-0005).
pub fn load_family(source: &FontSource, font_size_px: u16) -> Result<FontFamily, Box<dyn Error>> {
	let started = Instant::now();
	let family = match source {
		FontSource::Assets(dir) => FontFamily::load(&fonts_dir(dir), FONT_FAMILY)?,
		FontSource::System => {
			println!("gterm: searching for system monospace font (this may take a moment)...");
			FontFamily::system_monospace()?
		}
	};
	eprintln!(
		"gterm: font source: {source} — '{}' in {:?}",
		family.family(),
		started.elapsed()
	);
	for style in Style::ALL {
		let face = family.face(style);
		let note = if family.style_source(style) == style {
			String::new()
		} else {
			format!(" (fallback to {:?})", family.style_source(style))
		};
		eprintln!("gterm: {style:?}: {}{note}", face.path.display());
	}
	let glyph_px = glyph_px_for(font_size_px, FONT_SCALE, 1.0);
	let m = family.metrics_at(glyph_px)?;
	eprintln!(
		"gterm: logical {font_size_px}px → cell {}x{}, baseline {}; glyph_px at dpi 1.0 = {glyph_px}",
		m.glyph_w, m.glyph_h, m.baseline,
	);
	Ok(family)
}

/// Растеризатор демо-бинарников: семейство при `DEMO_FONT_SIZE_PX` и, при источнике `assets`,
/// SVG-глифы из `assets/svg/` (ADR-0008) с логом числа глифов и перекрытых глифов
/// шрифта/псевдографики.
pub fn load_rasterizer(source: &FontSource) -> Result<Rasterizer, Box<dyn Error>> {
	// Сразу под размер демо при DPI 1.0: иначе окно перестраивает хинтеры с 16 px на 26 px.
	let glyph_px = glyph_px_for(DEMO_FONT_SIZE_PX, FONT_SCALE, 1.0);
	let mut rasterizer = Rasterizer::new_at(load_family(source, DEMO_FONT_SIZE_PX)?, glyph_px)?;
	let FontSource::Assets(assets_dir) = source else {
		return Ok(rasterizer);
	};
	let dir = svg_dir(assets_dir);
	let svg = SvgGlyphs::load(&dir);
	let overridden: Vec<String> = svg
		.codepoints()
		.filter(|&ch| rasterizer.has_font_glyph(ch, Style::Regular))
		.map(|ch| format!("U+{:04X}", u32::from(ch)))
		.collect();
	let overrides = if overridden.is_empty() {
		String::from("no overrides")
	} else {
		format!("override font: {}", overridden.join(" "))
	};
	eprintln!(
		"gterm: svg: {} glyphs from {} ({overrides})",
		svg.len(),
		dir.display()
	);
	rasterizer.set_svg_glyphs(svg);
	Ok(rasterizer)
}

/// `AutoGrid`: сетка следует за окном, размер шрифта не трогается (FR-23); сцену под новую сетку
/// достраивает приложение в `on_grid_resized`.
pub fn demo_options() -> AppOptions {
	AppOptions {
		grid_mode: GridMode::AutoGrid,
		font_size_px: DEMO_FONT_SIZE_PX,
		..AppOptions::default()
	}
}

/// Клавиши FR-28, общие для демо-бинарников: `Esc` — выход, `F11` — полный экран, `1/2/3` —
/// бэкенд. Полный экран хранит приложение — библиотека состояние не держит (FR-22).
/// Возвращает `true`, если клавиша обработана.
pub fn handle_common_key(event: &KeyEvent, fullscreen: &mut bool, ctx: &mut Ctx<'_>) -> bool {
	if event.state != ElementState::Pressed {
		return false;
	}
	match &event.logical_key {
		Key::Named(NamedKey::Escape) => ctx.exit(),
		Key::Named(NamedKey::F11) => {
			*fullscreen = !*fullscreen;
			ctx.request.fullscreen = Some(*fullscreen);
		}
		// `Key::Character` в winit 0.30 несёт строку, а не символ.
		Key::Character(c) if &**c == "1" => ctx.request.backend = Some(RendererKind::OpenGl33),
		Key::Character(c) if &**c == "2" => ctx.request.backend = Some(RendererKind::Gles30),
		Key::Character(c) if &**c == "3" => ctx.request.backend = Some(RendererKind::Software),
		_ => return false,
	}
	true
}

/// Строка 0 — имя активного бэкенда и статистика кадров (FR-20, FR-27): FPS за последний
/// интервал между обновлениями статуса, CPU-часть и полное время последнего кадра.
pub fn write_status(screen: &mut ScreenBuffer, backend: &str, stats: FrameStats, fps: f64) {
	if screen.rows() == 0 {
		return;
	}
	screen.fill_row(0, STATUS_FG, STATUS_BG);
	let text = format!(
		"gterm — renderer: {backend} · {fps:.1} fps · cpu {:.2} ms · кадр {:.2} ms",
		millis(stats.cpu),
		millis(stats.total)
	);
	screen.write_row(0, &text, STATUS_FG, STATUS_BG);
}

/// Длительность в миллисекундах для печати.
pub fn millis(d: Duration) -> f64 {
	d.as_secs_f64() * 1e3
}

/// FPS по разности счётчика кадров между двумя вызовами `update` (по событиям приложения).
/// В простое без перерисовок честно показывает 0.
pub struct FpsMeter {
	frames: u64,
	at: Instant,
	fps: f64,
}

impl Default for FpsMeter {
	fn default() -> Self {
		Self {
			frames: 0,
			at: Instant::now(),
			fps: 0.0,
		}
	}
}

impl FpsMeter {
	/// Учесть текущее число кадров; возвращает FPS за интервал с прошлого вызова.
	pub fn update(&mut self, frames: u64) -> f64 {
		let now = Instant::now();
		let seconds = now.duration_since(self.at).as_secs_f64();
		if seconds > 0.0 {
			self.fps = frames.saturating_sub(self.frames) as f64 / seconds;
		}
		self.frames = frames;
		self.at = now;
		self.fps
	}
}
