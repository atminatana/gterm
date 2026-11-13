//! Всплывающее окно (ADR-0023, К4): прозрачный буфер с фоном по альфе; поверх экрана через
//! наложение Software-рендерера пиксель под окном — смесь фона экрана и цвета окна.

mod common;

use std::path::{Path, PathBuf};

use common::{BG, screen};
use gterm::atlas::{Atlas, PAGE_PX_MIN};
use gterm::cell::Color;
use gterm::font::FontFamily;
use gterm::overlay::Overlay;
use gterm::rasterizer::Rasterizer;
use gterm::renderer::Renderer;
use gterm::renderer::soft_backend::SoftBackend;
use gterm_widgets::widgets::{Block, Paragraph, Popup};
use gterm_widgets::{Rect, Style, Widget};

const POPUP_BG: Color = Color::from_rgba(0x30, 0x20, 0x50, 0xC0);

/// Шрифт репозитория (пакет `gterm-demo`), как у тестов библиотеки.
fn repo_rasterizer() -> Rasterizer {
	let fonts: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../gterm-demo/assets/fonts");
	let family = FontFamily::load(&fonts, "RobotoMono Nerd Font Mono").expect("шрифт репозитория");
	Rasterizer::new(family).expect("растеризатор")
}

#[test]
fn popup_buffer_is_transparent_with_alpha_background() {
	let popup = Popup::centered(40, 10, 20, 4).style(Style::new().bg(POPUP_BG));
	assert_eq!(popup.area, Rect::new(10, 3, 20, 4));
	assert_eq!(popup.local_area(), Rect::new(0, 0, 20, 4));
	let buffer = popup.buffer();
	assert_eq!((buffer.cols(), buffer.rows()), (20, 4));
	let cell = buffer.cell(0, 0);
	assert_eq!(cell.ch, '\0');
	assert_eq!(cell.bg, POPUP_BG);
	assert_eq!(cell.bg.a(), 0xC0);
	assert_eq!(
		buffer.background(),
		Color::TRANSPARENT,
		"подложка буфера окна прозрачна"
	);
	let plain = Popup::new(Rect::new(0, 0, 3, 2)).buffer();
	assert_eq!(plain.cell(1, 1).bg, Color::TRANSPARENT);
}

#[test]
fn popup_blends_over_the_screen_in_software() {
	let mut soft = SoftBackend::new(Atlas::new(repo_rasterizer(), PAGE_PX_MIN));
	let (gw, gh) = (soft.atlas().glyph_w() as u32, soft.atlas().glyph_h() as u32);
	let (cols, rows) = (20usize, 6usize);
	soft.resize(cols as u32 * gw, rows as u32 * gh).unwrap();
	let mut main = screen(cols, rows);
	main.set_background(BG);

	// Кадр без окна: пиксель пустой ячейки — фон экрана.
	let probe = |soft: &SoftBackend, col: u32, row: u32| -> u32 {
		let x = col * gw + gw / 2;
		let y = row * gh + gh / 2;
		soft.frame()[(y * cols as u32 * gw + x) as usize]
	};
	soft.render_overlaid(&mut main, &Overlay::new()).unwrap();
	let without = probe(&soft, 5, 2);
	assert_eq!(without, BG.0 & 0x00FF_FFFF);

	let popup = Popup::centered(cols as u16, rows as u16, 10, 4).style(Style::new().bg(POPUP_BG));
	let mut buffer = popup.buffer();
	Paragraph::new("hi")
		.block(Block::bordered())
		.render(popup.local_area(), &mut buffer);
	let mut overlay = Overlay::new();
	overlay.push(&buffer, i32::from(popup.area.x), i32::from(popup.area.y));
	soft.render_overlaid(&mut main, &overlay).unwrap();
	// Пустая ячейка внутри окна (не рамка и не текст): смесь, а не чистый цвет.
	let inside = probe(
		&soft,
		u32::from(popup.area.x) + 3,
		u32::from(popup.area.y) + 2,
	);
	let popup_rgb = POPUP_BG.0 & 0x00FF_FFFF;
	assert_ne!(inside, without, "окно должно изменить пиксель под собой");
	assert_ne!(inside, popup_rgb, "альфа 0xC0 — не чистый цвет окна");
	let channel = |v: u32, shift: u32| (v >> shift) & 0xFF;
	for shift in [16, 8, 0] {
		let (lo, hi) = (
			channel(without, shift).min(channel(popup_rgb, shift)),
			channel(without, shift).max(channel(popup_rgb, shift)),
		);
		assert!(
			(lo..=hi).contains(&channel(inside, shift)),
			"канал {shift}: {:#x} вне [{lo:#x}, {hi:#x}]",
			channel(inside, shift)
		);
	}
	// За пределами окна пиксель прежний.
	assert_eq!(probe(&soft, 0, 0), without);
}
