//! Этап 4 (ТЗ §11.2): кадр GL-бэкенда побайтно равен кадру Software (допуск 0).
//!
//! Нужен настоящий GL-контекст: создаётся на скрытом окне через `glutin-winit`. Если GL недоступен
//! (нет дисплея, нет драйвера, `GDI Generic`) — тест печатает причину и завершается успешно: это
//! «пропуск» из §11.2, а не падение. Любая ошибка после успешного создания контекста — падение.

mod common;

use glutin_winit::DisplayBuilder;
use gterm::atlas::{Atlas, page_px_for_limit};
use gterm::buffer::ScreenBuffer;
use gterm::cell::{CellFlags, Color};
use gterm::overlay::Overlay;
use gterm::rasterizer::BoxDrawingMode;
use gterm::renderer::Renderer;
use gterm::renderer::RendererKind;
use gterm::renderer::gl_backend::{self, GlBackend, GlContext, GlOptions};
use gterm::renderer::soft_backend::SoftBackend;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use common::{
	BIG_FONT_PX, GOLDEN_H, GOLDEN_W, RenderWith, big_background, golden_background, golden_scene,
	repo_atlas, repo_atlas_at, repo_rasterizer, repo_rasterizer_at, svg_atlas_at,
	svg_rasterizer_at,
};

/// Результат сравнения, заполняется внутри цикла событий.
enum Outcome {
	Pending,
	/// GL недоступен — причина.
	Unavailable(String),
	/// Кадры совпали для всех сцен.
	Identical {
		api: &'static str,
		scenes: usize,
	},
	/// Расхождение: сцена, первый отличающийся пиксель, значения.
	Mismatch(String),
	/// Ошибка после успешного создания контекста.
	Failed(String),
}

struct GlProbe {
	/// Уровни для проверки и их результаты; заполняется в одном `resumed` — второй цикл событий
	/// winit в процессе не допускает.
	outcomes: Vec<(RendererKind, Outcome)>,
}

impl GlProbe {
	fn compare(&mut self, event_loop: &ActiveEventLoop, kind: RendererKind) -> Outcome {
		let attributes = Window::default_attributes()
			.with_title("gterm gl test")
			.with_visible(false)
			.with_inner_size(PhysicalSize::new(GOLDEN_W, GOLDEN_H));
		let built = DisplayBuilder::new()
			.with_window_attributes(Some(attributes.clone()))
			.build(
				event_loop,
				gl_backend::config_template(),
				gl_backend::pick_config,
			);
		let (window, config) = match built {
			Ok((Some(w), c)) => (w, c),
			Ok((None, c)) => match glutin_winit::finalize_window(event_loop, attributes, &c) {
				Ok(w) => (w, c),
				Err(e) => return Outcome::Unavailable(format!("window: {e}")),
			},
			Err(e) => return Outcome::Unavailable(format!("config: {e}")),
		};
		let ctx = match GlContext::new(
			&config,
			&window,
			GOLDEN_W,
			GOLDEN_H,
			kind,
			&GlOptions::default(),
		) {
			Ok(ctx) => ctx,
			Err(e) => return Outcome::Unavailable(e.to_string()),
		};
		let api = ctx.info().kind.name();
		eprintln!(
			"GL: {} / {} / {} (max texture {})",
			ctx.info().vendor,
			ctx.info().renderer,
			ctx.info().version,
			ctx.info().max_texture_size
		);
		let atlas = Atlas::new(
			repo_rasterizer(),
			page_px_for_limit(Some(ctx.max_texture_size())),
		);
		let mut gl = match GlBackend::new(ctx, atlas) {
			Ok(gl) => gl,
			Err((e, _)) => return Outcome::Failed(format!("GlBackend::new: {e}")),
		};

		// Сцены: обычное окно; окно меньше сетки при крупном шрифте (обрезка, FR-23);
		// крупный шрифт в окне вдвое больше сетки (центрирование и рамка); и переход
		// 16 → 32 px сменой размера на месте — тот путь, который идёт при переносе окна между
		// мониторами (FR-24). Различие сцен — размер растеризации, а не масштаб (ADR-0005).
		struct Scene {
			name: &'static str,
			w: u32,
			h: u32,
			size_px: u16,
			/// Размер достигается `set_size` на месте, а не новым растеризатором.
			via_dpi: bool,
			/// Фон буфера сцены (ADR-0013).
			view: Color,
			/// Экран сцены; строится заново на каждый кадр — `render` сбрасывает dirty.
			screen: fn() -> ScreenBuffer,
			/// Растеризатор с тестовыми SVG-глифами (оба бэкенда).
			svg: bool,
			/// Фон буфера третьего кадра, если он меняется (ADR-0013); иначе — тот же.
			background_after: Option<Color>,
			/// Записи наложения каждого кадра (ADR-0014).
			overlay: fn() -> Overlay,
			/// Источник псевдографики обоих бэкендов (ADR-0018).
			box_mode: BoxDrawingMode,
		}
		let scenes = [
			Scene {
				name: "golden 16px",
				w: GOLDEN_W,
				h: GOLDEN_H,
				size_px: 16,
				via_dpi: false,
				view: golden_background(),
				screen: golden_scene,
				svg: false,
				background_after: None,
				overlay: Overlay::new,
				box_mode: BoxDrawingMode::Procedural,
			},
			// Смена DPI на месте: та же страница атласа и тот же растеризатор, другие хинтеры.
			// Идёт сразу за 16px-сценой, потому что оба бэкенда обязаны сменить размер на
			// месте — ровно как при переносе окна между мониторами (FR-24).
			Scene {
				name: "dpi 16px → 32px",
				w: GOLDEN_W,
				h: GOLDEN_H,
				size_px: BIG_FONT_PX,
				via_dpi: true,
				view: big_background(),
				screen: golden_scene,
				svg: false,
				background_after: None,
				overlay: Overlay::new,
				box_mode: BoxDrawingMode::Procedural,
			},
			Scene {
				name: "32px clipped",
				w: GOLDEN_W,
				h: GOLDEN_H,
				size_px: BIG_FONT_PX,
				via_dpi: false,
				view: big_background(),
				screen: golden_scene,
				svg: false,
				background_after: None,
				overlay: Overlay::new,
				box_mode: BoxDrawingMode::Procedural,
			},
			Scene {
				name: "32px wide",
				w: GOLDEN_W * 2 + 7,
				h: GOLDEN_H * 2 + 5,
				size_px: BIG_FONT_PX,
				via_dpi: false,
				view: big_background(),
				screen: golden_scene,
				svg: false,
				background_after: None,
				overlay: Overlay::new,
				box_mode: BoxDrawingMode::Procedural,
			},
			// Глифы вне BMP (ADR-0009): ключ атласа шире 16 бит, данные GPU — тот же слот.
			Scene {
				name: "outside BMP 16px",
				w: GOLDEN_W,
				h: GOLDEN_H,
				size_px: 16,
				via_dpi: false,
				view: golden_background(),
				screen: outside_bmp_scene,
				svg: false,
				background_after: None,
				overlay: Overlay::new,
				box_mode: BoxDrawingMode::Procedural,
			},
			// Альфа fg и bg над фоном буфера не по умолчанию (ADR-0013); третий кадр меняет фон —
			// прозрачные ячейки показывают новую подложку.
			Scene {
				name: "alpha 16px",
				w: GOLDEN_W,
				h: GOLDEN_H,
				size_px: 16,
				via_dpi: false,
				view: Color::from_rgb(0x20, 0x60, 0x30),
				screen: alpha_scene,
				svg: false,
				background_after: Some(Color::from_rgb(0xC0, 0x40, 0x10)),
				overlay: Overlay::new,
				box_mode: BoxDrawingMode::Procedural,
			},
			// Наложение клиентских буферов (ADR-0014): две записи — с отрицательной позицией и за
			// правым нижним краем сетки; окно больше сетки (поле вокруг не трогается).
			Scene {
				name: "overlay 16px",
				w: GOLDEN_W,
				h: GOLDEN_H,
				size_px: 16,
				via_dpi: false,
				view: golden_background(),
				screen: golden_scene,
				svg: false,
				background_after: None,
				overlay: overlay_scene,
				box_mode: BoxDrawingMode::Procedural,
			},
			// SVG-глифы (ADR-0008) при размере демо: тот же атлас и данные GPU, другой источник
			// растра. Последняя — растеризатор с SVG подменяется только здесь.
			Scene {
				name: "svg 22px",
				w: GOLDEN_W * 2,
				h: GOLDEN_H * 2,
				size_px: 22,
				via_dpi: false,
				view: golden_background(),
				screen: svg_scene,
				svg: true,
				background_after: None,
				overlay: Overlay::new,
				box_mode: BoxDrawingMode::Procedural,
			},
			// Псевдографика из шрифта и комбинированная (ADR-0018): у начертаний свои глифы рамок,
			// режим переключается на лету на обоих бэкендах — страница атласа сбрасывается.
			Scene {
				name: "box font 16px",
				w: GOLDEN_W,
				h: GOLDEN_H,
				size_px: 16,
				via_dpi: false,
				view: golden_background(),
				screen: box_scene,
				svg: false,
				background_after: None,
				overlay: Overlay::new,
				box_mode: BoxDrawingMode::Font,
			},
			Scene {
				name: "box combined 16px",
				w: GOLDEN_W,
				h: GOLDEN_H,
				size_px: 16,
				via_dpi: false,
				view: golden_background(),
				screen: box_scene,
				svg: false,
				background_after: None,
				overlay: Overlay::new,
				box_mode: BoxDrawingMode::Combined,
			},
		];
		let mut gl_size_px = 16u16;
		for scene in &scenes {
			let (name, w, h, view) = (scene.name, &scene.w, &scene.h, &scene.view);
			let w = *w;
			let h = *h;
			// Крупный шрифт — это другой размер растеризации (ADR-0005), а не масштаб.
			// Обычно GL получает новый растеризатор, Software — новый атлас; в сцене смены DPI
			// оба меняют размер на месте, как при переносе окна между мониторами (FR-24).
			let mut soft = SoftBackend::new(if scene.svg {
				svg_atlas_at(scene.size_px)
			} else if scene.via_dpi {
				repo_atlas()
			} else {
				repo_atlas_at(scene.size_px)
			});
			if scene.svg {
				gl.set_rasterizer(svg_rasterizer_at(scene.size_px));
				gl_size_px = scene.size_px;
			} else if gl_size_px != scene.size_px {
				if scene.via_dpi {
					if let Err(e) = gl.set_size(scene.size_px) {
						return Outcome::Failed(format!("{name}: gl set_size: {e}"));
					}
					if let Err(e) = soft.set_size(scene.size_px) {
						return Outcome::Failed(format!("{name}: soft set_size: {e}"));
					}
				} else {
					gl.set_rasterizer(repo_rasterizer_at(scene.size_px));
				}
				gl_size_px = scene.size_px;
			}
			soft.set_box_drawing_mode(scene.box_mode);
			gl.set_box_drawing_mode(scene.box_mode);
			if let Err(e) = soft.resize(w, h) {
				return Outcome::Failed(format!("{name}: soft resize: {e}"));
			}
			let mut soft_screen = (scene.screen)();
			let overlay = (scene.overlay)();
			if let Err(e) = soft.render_overlaid_with(&mut soft_screen, view, &overlay) {
				return Outcome::Failed(format!("{name}: soft render: {e}"));
			}
			if let Err(e) = gl.resize(w, h) {
				return Outcome::Failed(format!("{name}: gl resize: {e}"));
			}
			// Два кадра подряд: второй идёт по dirty-пути (ничего не dirty) и обязан совпасть.
			// Третий — после записи в строку: Software перерисовывает по тому же экрану только
			// её (ADR-0010) и обязан совпасть с полным кадром GL того же содержимого.
			for pass in 0..3 {
				let mut gl_screen = (scene.screen)();
				let mut background = *view;
				if pass == 2 {
					background = scene.background_after.unwrap_or(background);
					change_row(&mut soft_screen);
					change_row(&mut gl_screen);
					if let Err(e) =
						soft.render_overlaid_with(&mut soft_screen, &background, &overlay)
					{
						return Outcome::Failed(format!("{name}: soft partial render: {e}"));
					}
				}
				gl_screen.set_background(background);
				let frame = match gl.render_to_frame_overlaid(&mut gl_screen, &overlay) {
					Ok(f) => f,
					Err(e) => {
						return Outcome::Failed(format!("{name} pass {pass}: gl render: {e}"));
					}
				};
				if frame.len() != soft.frame().len() {
					return Outcome::Mismatch(format!(
						"{name}: frame size {} vs {}",
						frame.len(),
						soft.frame().len()
					));
				}
				if let Some(i) = frame.iter().zip(soft.frame()).position(|(a, b)| a != b) {
					return Outcome::Mismatch(format!(
						"{name} pass {pass}: pixel {i} ({}, {}): gl 0x{:06X} vs soft 0x{:06X}",
						i % w as usize,
						i / w as usize,
						frame[i],
						soft.frame()[i]
					));
				}
			}
		}
		Outcome::Identical {
			api,
			scenes: scenes.len(),
		}
	}
}

/// Золотая сцена и глифы вне BMP в свободном хвосте строки 5: иконки MDI шрифта (U+F0001,
/// U+F0041 — младшие 16 бит совпадают с `A`, U+F1AF0) и U+10FFFD без глифа (слот 0).
/// Золотая сцена и SVG-глифы в хвосте строки 5: квадрат, кольцо, перекрытая иконка шрифта
/// U+F0041, `evenodd` и перекрытая псевдографика U+2500 — с флагами начертаний и `UNDERLINE`.
fn svg_scene() -> ScreenBuffer {
	let mut screen = golden_scene();
	let glyphs = [
		('\u{E000}', CellFlags::empty()),
		('\u{E001}', CellFlags::BOLD),
		('\u{F0041}', CellFlags::ITALIC),
		('\u{10FFFD}', CellFlags::UNDERLINE),
		('\u{2500}', CellFlags::BOLD | CellFlags::ITALIC),
	];
	for (i, (ch, flags)) in glyphs.into_iter().enumerate() {
		screen.set_cell(19 + i, 5, ch, Color::GREEN, Color::BLACK, flags);
	}
	screen
}

/// Золотая сцена и строки с альфой (ADR-0013): строка 1 — `fg.a` 0…255 по колонкам над непрозрачным
/// фоном, строка 3 — `bg.a` 0…255 с непрозрачным символом, строка 4 — обе альфы и `BOLD`/`UNDERLINE`.
/// Рамки трёх видов, тени и блоки, жирная строка — все ветки режима псевдографики (ADR-0018).
fn box_scene() -> ScreenBuffer {
	let mut screen = golden_scene();
	screen.write_row(2, "┌─┬┐├┼┤└┴┘ ╔═╦╗╬ ┏━┯┓┠┼┨", Color::WHITE, Color::BLACK);
	for (i, ch) in "╒═╤╕╞╪╡╘╧╛ ┄┅┆┇ ╭╮╯╰ ░▒▓█".chars().enumerate()
	{
		screen.set_cell(
			i,
			3,
			ch,
			Color::GREEN,
			Color::BLACK,
			CellFlags::BOLD | CellFlags::ITALIC,
		);
	}
	screen
}

fn alpha_scene() -> ScreenBuffer {
	let mut screen = golden_scene();
	let alphas = [0u8, 1, 64, 127, 128, 200, 254, 255];
	for col in 0..screen.cols() {
		let a = alphas[col % alphas.len()];
		let fg = Color::from_rgba(0xF0, 0xE0, 0x10, a);
		let bg = Color::from_rgba(0x30, 0x10, 0xA0, a);
		let opaque_fg = Color::from_rgb(0xF0, 0xE0, 0x10);
		let opaque_bg = Color::from_rgb(0x30, 0x10, 0xA0);
		screen.set_cell(col, 1, 'Ж', fg, opaque_bg, CellFlags::empty());
		screen.set_cell(col, 3, 'g', opaque_fg, bg, CellFlags::ITALIC);
		let flags = if col % 2 == 0 {
			CellFlags::BOLD
		} else {
			CellFlags::UNDERLINE
		};
		screen.set_cell(col, 4, '█', fg, bg, flags);
	}
	screen
}

/// Две записи наложения поверх golden-сцены: полупрозрачные фон и символы с `BOLD`/`UNDERLINE`,
/// прозрачные и непрозрачные ячейки; первая начинается за левым верхним краем, вторая выходит
/// за правый нижний.
fn overlay_scene() -> Overlay {
	let mut first = ScreenBuffer::transparent(6, 3);
	let mut second = ScreenBuffer::transparent(6, 3);
	let alphas = [0u8, 64, 128, 200, 255, 1];
	for (col, &a) in alphas.iter().enumerate() {
		for row in 0..3 {
			let flags = match row {
				0 => CellFlags::BOLD,
				1 => CellFlags::UNDERLINE,
				_ => CellFlags::ITALIC,
			};
			first.set_cell(
				col,
				row,
				'Ж',
				Color::from_rgba(0xF0, 0x40, 0x10, a),
				Color::from_rgba(0x10, 0x60, 0xE0, 255 - a),
				flags,
			);
			second.set_cell(
				col,
				row,
				'█',
				Color::from_rgba(0x20, 0xF0, 0x80, a),
				Color::TRANSPARENT,
				CellFlags::empty(),
			);
		}
	}
	let mut overlay = Overlay::new();
	overlay.push(&first, -2, -1);
	overlay.push(&second, 20, 4);
	overlay.push(&first, 8, 2);
	overlay
}

/// Запись в строку 2 для третьего кадра: частичный кадр Software против полного кадра GL.
fn change_row(screen: &mut ScreenBuffer) {
	screen.write_row(2, "changed row", Color::CYAN, Color::BLACK);
}

fn outside_bmp_scene() -> ScreenBuffer {
	let mut screen = golden_scene();
	for (i, ch) in ['\u{F0001}', '\u{F0041}', '\u{F1AF0}', '\u{10FFFD}']
		.into_iter()
		.enumerate()
	{
		screen.set_cell(
			20 + i,
			5,
			ch,
			Color::MAGENTA,
			Color::BLACK,
			CellFlags::empty(),
		);
	}
	screen
}

impl ApplicationHandler for GlProbe {
	fn resumed(&mut self, event_loop: &ActiveEventLoop) {
		for i in 0..self.outcomes.len() {
			if matches!(self.outcomes[i].1, Outcome::Pending) {
				let kind = self.outcomes[i].0;
				self.outcomes[i].1 = self.compare(event_loop, kind);
			}
		}
		event_loop.exit();
	}

	fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, _event: WindowEvent) {}
}

fn build_event_loop() -> Result<EventLoop<()>, String> {
	let mut builder = EventLoop::builder();
	#[cfg(target_os = "windows")]
	{
		use winit::platform::windows::EventLoopBuilderExtWindows;
		builder.with_any_thread(true);
	}
	#[cfg(all(unix, not(target_os = "macos")))]
	{
		use winit::platform::x11::EventLoopBuilderExtX11;
		builder.with_any_thread(true);
	}
	builder.build().map_err(|e| e.to_string())
}

/// §11.2: GL ↔ Software, допуск 0, на каждом GL-уровне цепочки (один цикл событий на оба);
/// уровень без контекста — печать причины и пропуск.
#[test]
fn gl_frames_match_software_frames_bit_for_bit() {
	let event_loop = match build_event_loop() {
		Ok(el) => el,
		Err(e) => {
			eprintln!("GL test skipped: no event loop ({e})");
			return;
		}
	};
	let mut probe = GlProbe {
		outcomes: vec![
			(RendererKind::OpenGl33, Outcome::Pending),
			(RendererKind::Gles30, Outcome::Pending),
		],
	};
	if let Err(e) = event_loop.run_app(&mut probe) {
		eprintln!("GL test skipped: event loop error ({e})");
		return;
	}
	for (kind, outcome) in probe.outcomes {
		match outcome {
			Outcome::Identical { api, scenes } => {
				eprintln!("{api}: GL ↔ Software identical, {scenes} scenes")
			}
			Outcome::Unavailable(reason) => {
				eprintln!("{}: skipped, unavailable ({reason})", kind.name())
			}
			Outcome::Pending => panic!("{}: event loop never resumed", kind.name()),
			Outcome::Mismatch(m) => panic!("{}: GL and Software frames differ: {m}", kind.name()),
			Outcome::Failed(f) => panic!(
				"{}: GL path failed after context creation: {f}",
				kind.name()
			),
		}
	}
}
