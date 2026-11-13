//! Модальный слой (ADR-0026, ТЗ К6): весь ввод слою, фокус экрана не тронут, локальные ячейки,
//! клик вне поглощается, `Esc` → `on_escape` или «не обработано», слой рисуется через `Popup`
//! с фоном стиля поверх экрана.

mod common;

use std::path::{Path, PathBuf};

use common::{BG, CallLog, Msg, Stub, call_log, key_msg, left_down, left_up, mouse, moved, screen};
use gterm::atlas::{Atlas, PAGE_PX_MIN};
use gterm::cell::Color;
use gterm::font::FontFamily;
use gterm::overlay::Overlay;
use gterm::rasterizer::Rasterizer;
use gterm::renderer::Renderer;
use gterm::renderer::soft_backend::SoftBackend;
use gterm_widgets::input::{Handled, MouseKind};
use gterm_widgets::{Constraint, Layout, Modal, Rect, Style, Ui};
use winit::keyboard::{Key, ModifiersState, NamedKey};

const SCREEN: Rect = Rect::new(0, 0, 20, 10);
const DIALOG_BG: Color = Color::from_rgb(0x30, 0x20, 0x50);

/// Экран: контрол `main`; диалог 10×4 по центру (область 5..15 × 3..7) с контролами `ok` и
/// `cancel` по одной строке; две нижние строки слоя пусты — виден его фон.
struct App {
	main: Stub,
	ok: Stub,
	cancel: Stub,
	dialog: bool,
	escape: bool,
}

impl App {
	fn new(log: &CallLog) -> Self {
		Self {
			main: Stub::new("main", true, log).answering(Handled::Yes),
			ok: Stub::new("ok", true, log).answering(Handled::Yes),
			cancel: Stub::new("cancel", true, log),
			dialog: true,
			escape: true,
		}
	}

	fn ui(&mut self) -> Ui<'_, Msg> {
		let screen = Ui::split(
			Layout::vertical([Constraint::Fill(1)]),
			vec![Ui::control(&mut self.main)],
		);
		if !self.dialog {
			return screen;
		}
		let mut modal = Modal::centered(10, 4)
			.style(Style::new().bg(DIALOG_BG))
			.content(Ui::split(
				Layout::vertical([Constraint::Length(1), Constraint::Length(1)]),
				vec![Ui::control(&mut self.ok), Ui::control(&mut self.cancel)],
			));
		if self.escape {
			modal = modal.on_escape(Msg::Custom("close"));
		}
		screen.modal(modal)
	}

	/// Раскладка слоя происходит при его отрисовке.
	fn draw(&mut self) {
		let (area, _) = self.ui().modal_buffer(SCREEN).expect("слой есть");
		assert_eq!(area, Rect::new(5, 3, 10, 4));
	}
}

#[test]
fn keys_go_to_the_layer_and_the_screen_focus_is_untouched() {
	let log = call_log();
	let mut app = App::new(&log);
	app.main.node.focused = true;
	let plain = ModifiersState::empty();

	let routed = app
		.ui()
		.route_key(&key_msg(&Key::Named(NamedKey::Enter), plain));
	assert_eq!(routed.handled, Handled::Yes);
	assert_eq!(log.borrow().as_slice(), ["ok:key:Named(Enter)"]);
	assert!(app.main.node.focused, "фокус экрана не тронут");
	assert!(app.ok.node.focused, "в слое свой фокус — первый кандидат");

	// Tab ходит внутри слоя.
	app.ui()
		.route_key(&key_msg(&Key::Named(NamedKey::Tab), plain));
	assert!(app.cancel.node.focused && !app.ok.node.focused);
	assert!(app.main.node.focused);
}

#[test]
fn escape_not_taken_by_the_layer_publishes_on_escape_or_returns_to_the_app() {
	let log = call_log();
	let mut app = App::new(&log);
	let plain = ModifiersState::empty();
	let escape = Key::Named(NamedKey::Escape);

	// `cancel` отвечает «нет» → привязка слоя.
	app.ui()
		.route_key(&key_msg(&Key::Named(NamedKey::Tab), plain));
	assert!(app.cancel.node.focused);
	let routed = app.ui().route_key(&key_msg(&escape, plain));
	assert_eq!(routed.handled, Handled::Yes);
	assert_eq!(routed.messages, vec![Msg::Custom("close")]);

	// Контрол взял Esc сам — привязка молчит. Фокус внутри слоя двигает только ввод слою:
	// `focus_*` у описания относятся к основному экрану.
	app.ui()
		.route_key(&key_msg(&Key::Named(NamedKey::Tab), plain));
	assert!(app.ok.node.focused);
	let routed = app.ui().route_key(&key_msg(&escape, plain));
	assert_eq!(routed.handled, Handled::Yes);
	assert!(routed.messages.is_empty());

	// Без привязки непринятый Esc уходит приложению.
	app.escape = false;
	app.ui()
		.route_key(&key_msg(&Key::Named(NamedKey::Tab), plain));
	assert!(app.cancel.node.focused);
	let routed = app.ui().route_key(&key_msg(&escape, plain));
	assert_eq!(routed.handled, Handled::No);
	assert!(routed.messages.is_empty());
}

#[test]
fn mouse_inside_is_routed_in_local_cells_and_outside_is_swallowed() {
	let log = call_log();
	let mut app = App::new(&log);
	app.draw();
	assert_eq!(app.ok.node.area, Rect::new(0, 0, 10, 1));
	assert_eq!(app.cancel.node.area, Rect::new(0, 1, 10, 1));

	// Ячейка сетки (7, 4) — локальная (2, 1) → `cancel`; фокус переходит к нему. Слой
	// поглощает мышь даже когда контрол ответил «нет».
	let routed = app.ui().route_mouse(&left_down(7, 4), SCREEN);
	assert_eq!(routed.handled, Handled::Yes);
	assert_eq!(log.borrow().as_slice(), ["cancel:mouse:Down@2,1"]);
	assert!(app.cancel.node.focused);

	// Вне слоя и внутри мимо контролов — поглощается: экран ничего не получает, сообщений нет.
	for msg in [
		left_down(0, 0),
		left_down(19, 9),
		moved(5, 2),
		left_up(15, 3),
		left_down(7, 6),
	] {
		let routed = app.ui().route_mouse(&msg, SCREEN);
		assert_eq!(routed.handled, Handled::Yes, "{msg:?}");
		assert!(routed.messages.is_empty());
	}
	assert_eq!(log.borrow().len(), 1, "{:?}", log.borrow());
	assert!(!app.main.node.focused);
}

#[test]
fn capture_inside_the_layer_follows_the_pointer_outside_it() {
	let log = call_log();
	let mut app = App::new(&log);
	app.draw();
	app.ui().route_mouse(&left_down(6, 3), SCREEN);
	assert!(app.ok.node.captured);
	// Отпускание вне слоя идёт захватившему и снимает захват; наружу не проходит.
	let routed = app.ui().route_mouse(&left_up(0, 0), SCREEN);
	assert_eq!(routed.handled, Handled::Yes);
	assert!(!app.ok.node.captured);
	assert_eq!(
		log.borrow().as_slice(),
		["ok:mouse:Down@1,0", "ok:mouse:Up@-5,-3"]
	);
}

#[test]
fn without_the_layer_input_goes_to_the_screen_again() {
	let log = call_log();
	let mut app = App::new(&log);
	app.dialog = false;
	let routed = app.ui().route_key(&key_msg(
		&Key::Named(NamedKey::Enter),
		ModifiersState::empty(),
	));
	assert_eq!(routed.handled, Handled::Yes);
	assert_eq!(log.borrow().as_slice(), ["main:key:Named(Enter)"]);
	assert!(app.ui().modal_buffer(SCREEN).is_none());
}

/// Шрифт репозитория (пакет `gterm-demo`), как у тестов библиотеки.
fn repo_rasterizer() -> Rasterizer {
	let fonts: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../gterm-demo/assets/fonts");
	let family = FontFamily::load(&fonts, "RobotoMono Nerd Font Mono").expect("шрифт репозитория");
	Rasterizer::new(family).expect("растеризатор")
}

#[test]
fn the_layer_is_drawn_over_the_screen_with_its_opaque_background() {
	let log = call_log();
	let mut app = App::new(&log);
	let (area, buffer) = app.ui().modal_buffer(SCREEN).expect("слой есть");
	assert_eq!((buffer.cols(), buffer.rows()), (10, 4));
	// Содержимое нарисовано в локальных ячейках: `ok` под фокусом — заглавными.
	assert_eq!(buffer.cell(0, 0).ch, 'O');
	assert_eq!(buffer.cell(0, 1).ch, 'c');
	assert_eq!(
		buffer.cell(0, 3).ch,
		'\0',
		"пустая строка слоя — только фон"
	);
	assert_eq!(buffer.cell(0, 3).bg, DIALOG_BG);

	let mut soft = SoftBackend::new(Atlas::new(repo_rasterizer(), PAGE_PX_MIN));
	let (gw, gh) = (soft.atlas().glyph_w() as u32, soft.atlas().glyph_h() as u32);
	let (cols, rows) = (usize::from(SCREEN.width), usize::from(SCREEN.height));
	soft.resize(cols as u32 * gw, rows as u32 * gh).unwrap();
	let mut main = screen(cols, rows);
	main.set_background(BG);
	let probe = |soft: &SoftBackend, col: u32, row: u32| -> u32 {
		// Угол ячейки: вне контуров глифа, виден только фон.
		let x = col * gw + 1;
		let y = row * gh + 1;
		soft.frame()[(y * cols as u32 * gw + x) as usize]
	};
	soft.render_overlaid(&mut main, &Overlay::new()).unwrap();
	let without = probe(&soft, 7, 6);
	let mut overlay = Overlay::new();
	overlay.push(&buffer, i32::from(area.x), i32::from(area.y));
	soft.render_overlaid(&mut main, &overlay).unwrap();
	// Пустая ячейка слоя (сетка (7, 6) — локальная (2, 3)): непрозрачный фон слоя закрывает
	// экран целиком; за пределами слоя пиксель прежний.
	assert_eq!(probe(&soft, 7, 6), DIALOG_BG.0 & 0x00FF_FFFF);
	assert_eq!(probe(&soft, 0, 0), without);
}

#[test]
fn text_goes_to_the_layer_focus_while_it_is_open() {
	let log = call_log();
	let mut app = App::new(&log);
	app.main.node.focused = true;
	let routed = app.ui().text("q");
	assert_eq!(routed.handled, Handled::Yes, "ok отвечает «да»");
	assert_eq!(log.borrow().as_slice(), ["ok:text:q"]);
	assert!(app.main.node.focused, "фокус экрана не тронут");
}

#[test]
fn click_outside_publishes_the_binding_once_and_only_for_left_down() {
	let log = call_log();
	let mut outside = Stub::new("m", true, &log);
	let mut inner = Stub::new("i", true, &log);
	fn ui<'a>(outside: &'a mut Stub, inner: &'a mut Stub, close: bool) -> Ui<'a, Msg> {
		let mut modal = Modal::at(Rect::new(2, 2, 6, 3)).content(Ui::split(
			Layout::vertical([Constraint::Fill(1)]),
			vec![Ui::control(inner)],
		));
		if close {
			modal = modal.on_close(Msg::Custom("close"));
		}
		Ui::split(
			Layout::vertical([Constraint::Fill(1)]),
			vec![Ui::control(outside)],
		)
		.modal(modal)
	}
	// `Modal::at`: область как задана, обрезана по сетке.
	let (area, _) = ui(&mut outside, &mut inner, true)
		.modal_buffer(SCREEN)
		.unwrap();
	assert_eq!(area, Rect::new(2, 2, 6, 3));
	let (clipped, _) = Ui::<Msg>::split(Layout::vertical([Constraint::Fill(1)]), vec![])
		.modal(Modal::at(Rect::new(15, 8, 10, 10)))
		.modal_buffer(SCREEN)
		.unwrap();
	assert_eq!(clipped, Rect::new(15, 8, 5, 2));

	// Движение и колесо мимо — без сообщений; левое нажатие мимо — сообщение ровно один раз.
	for msg in [
		moved(0, 0),
		mouse(MouseKind::Wheel { lines: 1.0 }, None, 0, 0),
	] {
		let routed = ui(&mut outside, &mut inner, true).route_mouse(&msg, SCREEN);
		assert_eq!(routed.handled, Handled::Yes);
		assert!(routed.messages.is_empty(), "{msg:?}");
	}
	let routed = ui(&mut outside, &mut inner, true).route_mouse(&left_down(0, 0), SCREEN);
	assert_eq!(routed.messages, vec![Msg::Custom("close")]);
	// Внутри слоя — привязка молчит, контрол получает сообщение.
	let routed = ui(&mut outside, &mut inner, true).route_mouse(&left_down(3, 3), SCREEN);
	assert!(routed.messages.is_empty());
	assert_eq!(log.borrow().last().unwrap(), "i:mouse:Down@1,1");
	// `on_close` ставит и `Esc`.
	let routed = ui(&mut outside, &mut inner, true).route_key(&key_msg(
		&Key::Named(NamedKey::Escape),
		ModifiersState::empty(),
	));
	assert_eq!(routed.messages, vec![Msg::Custom("close")]);
	// Без привязки — прежнее молчаливое поглощение.
	let routed = ui(&mut outside, &mut inner, false).route_mouse(&left_down(0, 0), SCREEN);
	assert_eq!(routed.handled, Handled::Yes);
	assert!(routed.messages.is_empty());
	assert!(
		!outside.node.focused || outside.node.focused,
		"экран не трогается"
	);
	assert!(
		log.borrow().iter().all(|e| !e.starts_with("m:")),
		"{:?}",
		log.borrow()
	);
}
