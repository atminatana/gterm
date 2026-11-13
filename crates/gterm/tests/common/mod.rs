//! Общее для интеграционных тестов: шрифт репозитория и golden-сцена (§11.2).

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use gterm::atlas::{Atlas, PAGE_PX_MIN};
use gterm::buffer::ScreenBuffer;
use gterm::cell::{CellFlags, Color};
use gterm::font::FontFamily;
use gterm::overlay::Overlay;
use gterm::rasterizer::Rasterizer;
use gterm::renderer::{RenderError, Renderer};
use gterm::svg::SvgGlyphs;

pub const BORDER: Color = Color::from_rgb(0x12, 0x34, 0x56);
pub const FG: Color = Color::from_rgb(0x10, 0x20, 0x30);
pub const BG: Color = Color::from_rgb(0xA0, 0xB0, 0xC0);
/// Окно golden: вмещает сетку 24×6 при ячейке до 10×23 с полями под рамку.
pub const GOLDEN_W: u32 = 250;
pub const GOLDEN_H: u32 = 140;
/// Второй размер растеризации для сцены «крупный шрифт» вместо прежнего масштаба 2 (ADR-0005).
pub const BIG_FONT_PX: u16 = 32;
/// Имя семейства шрифта репозитория (`name` ID 1).
pub const REPO_FONT_FAMILY: &str = "RobotoMono Nerd Font Mono";

pub fn repo_assets_dir() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("../gterm-demo/assets")
}

pub fn repo_fonts_dir() -> PathBuf {
	repo_assets_dir().join("fonts")
}

pub fn repo_rasterizer() -> Rasterizer {
	Rasterizer::new(FontFamily::load(&repo_fonts_dir(), REPO_FONT_FAMILY).unwrap()).unwrap()
}

/// Растеризатор другого физического размера (ADR-0005): ячейка выводится из метрик заново.
pub fn repo_rasterizer_at(size_px: u16) -> Rasterizer {
	Rasterizer::new_at(
		FontFamily::load(&repo_fonts_dir(), REPO_FONT_FAMILY).unwrap(),
		size_px,
	)
	.unwrap()
}

/// Минимальная страница: тесты создают много атласов, 16 МБ на каждый ни к чему.
pub fn repo_atlas() -> Atlas {
	Atlas::new(repo_rasterizer(), PAGE_PX_MIN)
}

pub fn repo_atlas_at(size_px: u16) -> Atlas {
	Atlas::new(repo_rasterizer_at(size_px), PAGE_PX_MIN)
}

/// Тестовые SVG-глифы (ADR-0008) — отдельно от `assets/`, чтобы golden шрифта от них не зависел.
pub fn test_svg_dir() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/svg")
}

/// Растеризатор шрифта репозитория с тестовыми SVG-глифами.
pub fn svg_rasterizer_at(size_px: u16) -> Rasterizer {
	let mut rasterizer = repo_rasterizer_at(size_px);
	rasterizer.set_svg_glyphs(SvgGlyphs::load(&test_svg_dir()));
	rasterizer
}

pub fn svg_atlas_at(size_px: u16) -> Atlas {
	Atlas::new(svg_rasterizer_at(size_px), PAGE_PX_MIN)
}

/// Компактная сцена со всеми ветками алгоритма: текст, флаги, начертания, цвета,
/// псевдографика, пустые ячейки. Курсора нет — его рисует приложение (ADR-0004).
pub fn golden_scene() -> ScreenBuffer {
	let mut screen = ScreenBuffer::new(24, 6);
	screen.write_row(
		0,
		"gterm golden Привет ─┼█",
		Color::WHITE,
		Color::from_rgb(0, 0, 0x80),
	);
	screen.write_row(1, "plain ░▒▓ ╔═╦╗ ▚", FG, BG);
	let flagged = [
		(2, "Bold", CellFlags::BOLD),
		(
			3,
			"UNDERLINE Italic",
			CellFlags::UNDERLINE | CellFlags::ITALIC,
		),
		(4, "BOLD UNDERLINE", CellFlags::BOLD | CellFlags::UNDERLINE),
	];
	for (row, text, flags) in flagged {
		for (i, ch) in text.chars().enumerate() {
			screen.set_cell(i, row, ch, Color::GREEN, Color::BLACK, flags);
		}
	}
	screen.write_row(5, "app cursor>", Color::CYAN, Color::BLACK);
	for (i, ch) in "BoldIt".chars().enumerate() {
		screen.set_cell(
			13 + i,
			5,
			ch,
			Color::CYAN,
			Color::BLACK,
			CellFlags::BOLD | CellFlags::ITALIC,
		);
	}
	screen
}

/// Фон буфера golden-сцены — прежний цвет рамки (ADR-0013).
pub fn golden_background() -> Color {
	BORDER
}

/// Фон буфера сцены «крупный шрифт»: другой цвет поля. Размер окна и растеризации — параметр сцены, а не
/// масштаб (ADR-0005).
pub fn big_background() -> Color {
	Color::from_rgb(0x40, 0x00, 0x40)
}

/// Отрисовка с заданным фоном буфера: фон — свойство буфера (ADR-0013), а тестам удобно задавать
/// его вместе с вызовом, как раньше задавался вид.
pub trait RenderWith {
	fn render_with(
		&mut self,
		screen: &mut ScreenBuffer,
		background: &Color,
	) -> Result<(), RenderError>;

	/// То же с наложением (ADR-0014).
	fn render_overlaid_with(
		&mut self,
		screen: &mut ScreenBuffer,
		background: &Color,
		overlay: &Overlay,
	) -> Result<(), RenderError>;
}

impl<R: Renderer> RenderWith for R {
	fn render_with(
		&mut self,
		screen: &mut ScreenBuffer,
		background: &Color,
	) -> Result<(), RenderError> {
		screen.set_background(*background);
		self.render(screen)
	}

	fn render_overlaid_with(
		&mut self,
		screen: &mut ScreenBuffer,
		background: &Color,
		overlay: &Overlay,
	) -> Result<(), RenderError> {
		screen.set_background(*background);
		self.render_overlaid(screen, overlay)
	}
}
