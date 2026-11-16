//! Экран из виджетов `gterm-widgets` (ADR-0023, ADR-0026, ADR-0029) на системном моноширинном шрифте, без
//! `assets/`: сверху вкладки (ADR-0032; `←`/`→` под фокусом или клик), на первой — слева список
//! в группе с подсветкой рамки и полосой прокрутки (мышь: стрелки и дорожка с повтором при удержании, ползунок, колесо;
//! двойной клик по пункту — ADR-0035 — пишет его в строку позиции внизу), справа таблица с такой же полосой, абзац с переносом и кнопка,
//! на второй — индикаторы `Gauge`, `LineGauge` и `Meter` (ADR-0034); снизу индикатор позиции в списке и подсказка.
//! Экран описан один раз — `Demo::ui()`: дерево раскладывает, рисует,
//! водит фокус по `Tab`/`Shift+Tab`, доставляет клавиши и мышь контролам и отдаёт сообщения
//! `Msg`, которые приложение разбирает в `update`. `P` и кнопка открывают модальное окно с
//! непрозрачным фоном: `Tab` ходит внутри, `Esc`, `P` и «OK» закрывают, клик вне окна
//! поглощается. `P` — привязки карт сочетаний (ADR-0029) по физической клавише, раскладка не
//! важна: у экрана `P` открывает окно, у слоя своя карта — `P` закрывает.
//! `Esc` на главном экране — выход (клавиша, не принятая деревом). Сетка `AutoGrid` следует за
//! размером окна, содержимое пересобирается в `on_grid_resized`.
//! Запуск: `cargo run -p gterm-widgets --example widgets`.

use std::error::Error;
use std::process::ExitCode;
use std::time::Instant;

use gterm::app::{App, AppHandler, AppOptions, Ctx, DrawCtx, Grid, GridMode, PointerEvent};
use gterm::buffer::ScreenBuffer;
use gterm::cell::{Color, Style as FontStyle};
use gterm::config::{BACKEND_ENV, Config};
use gterm::font::{FONT_SCALE, FontFamily, glyph_px_for};
use gterm::hotkey::HotkeyTracker;
use gterm::rasterizer::Rasterizer;
use gterm_widgets::widgets::{
	Block, BorderType, Button, ButtonState, Gauge, LineGauge, List, ListState, Meter, Padding,
	Paragraph, Row, Scrollbar, ScrollbarOrientation, Table, TableState, Tabs, TabsState, Wrap,
};
use gterm_widgets::{Constraint, HotkeyMap, Layout, Line, Modal, Rect, Span, Style, Text, Ui};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, NamedKey};

const FONT_SIZE_PX: u16 = 14;
const COLS: usize = 90;
const ROWS: usize = 28;

const FG: Color = Color::from_rgb(0xD8, 0xD8, 0xD8);
const BG: Color = Color::from_rgb(0x10, 0x14, 0x1C);
const ACCENT: Color = Color::from_rgb(0x80, 0xC0, 0xFF);
const DIM: Color = Color::from_rgb(0x80, 0x80, 0x80);
const SELECTED_BG: Color = Color::from_rgb(0x2A, 0x4A, 0x7A);
/// Фон модального окна — непрозрачный: окно закрывает экран под собой (ТЗ 2026-10-30 п. 9).
const DIALOG_BG: Color = Color::from_rgb(0x30, 0x20, 0x50);
/// Зоны индикатора уровня (ADR-0034): норма, предупреждение, перегрузка.
const METER_ZONES: [(f64, Style); 3] = [
	(0.0, Style::new().fg(Color::from_rgb(0x30, 0xC0, 0x50))),
	(0.7, Style::new().fg(Color::from_rgb(0xF0, 0xC0, 0x30))),
	(0.9, Style::new().fg(Color::from_rgb(0xE0, 0x40, 0x40))),
];
/// Насколько отметка пика в примере опережает уровень.
const METER_PEAK_LEAD: f64 = 0.15;

const ITEMS: [&str; 14] = [
	"Квадранты",
	"Секстанты",
	"Октанты",
	"Рамки одиночные",
	"Рамки двойные",
	"Рамки жирные",
	"Круги",
	"Треугольники",
	"Диагонали",
	"SVG-тайлы",
	"Наложение буферов",
	"Альфа цветов",
	"Системный шрифт",
	"Виджеты",
];

const LOREM: &str = "Виджеты рисуют себя в ScreenBuffer внутри прямоугольника: без второго буфера \
и диффа — частичную перерисовку делает gterm. Экран описан один раз: дерево раскладывает, рисует \
и маршрутизирует ввод, а приложение получает сообщения. Tab и Shift+Tab ходят по списку, таблице \
и кнопке, стрелки и колесо двигают выделение, клик даёт фокус. Кнопка или P открывают окно.";

const INDICATORS: &str = "Gauge заливает полосу на долю с точностью до восьмой части ячейки — \
блоки-восьмушки рисуются процедурно и от шрифта не зависят. LineGauge — та же доля линией в одну \
строку. Meter — уровень с точностью до половины ячейки: цвет ячейки задаёт её место на шкале, \
отдельная ячейка — отметка пика. Все три показывают позицию выделения в списке с первой \
вкладки; полоса прокрутки рядом со \
списком — его смещение. Вернитесь на первую вкладку (← под фокусом на вкладках) и подвигайте \
выделение.";

fn main() -> ExitCode {
	let env_backend = std::env::var(BACKEND_ENV).ok();
	let config = match Config::parse(std::env::args().skip(1), env_backend.as_deref()) {
		Ok(config) if config.help => {
			println!("{}", Config::usage("widgets"));
			return ExitCode::SUCCESS;
		}
		Ok(config) => config,
		Err(e) => {
			eprintln!("widgets: {e}");
			return ExitCode::FAILURE;
		}
	};
	match run(config) {
		Ok(()) => ExitCode::SUCCESS,
		Err(e) => {
			eprintln!("widgets: {e}");
			ExitCode::FAILURE
		}
	}
}

fn run(config: Config) -> Result<(), Box<dyn Error>> {
	println!("widgets: searching for system monospace font (this may take a moment)...");
	let family = FontFamily::system_monospace()?;
	eprintln!(
		"widgets: font '{}' — {}",
		family.family(),
		family.face(FontStyle::Regular).path.display()
	);
	let glyph_px = glyph_px_for(FONT_SIZE_PX, FONT_SCALE, 1.0);
	let rasterizer = Rasterizer::new_at(family, glyph_px)?;
	let screen = ScreenBuffer::with_default_colors(COLS, ROWS, FG, BG);
	let options = AppOptions {
		grid_mode: GridMode::AutoGrid,
		font_size_px: FONT_SIZE_PX,
		..AppOptions::default()
	};
	let demo = Demo {
		keys: HotkeyMap::new().bind_str("P", Msg::OpenDialog)?,
		dialog_keys: HotkeyMap::new().bind_str("P", Msg::CloseDialog)?,
		..Demo::default()
	};
	App::new("gterm widgets", screen, rasterizer, config, options).run(demo)?;
	Ok(())
}

/// Сообщения приложения: их публикуют привязки описания, разбирает `Demo::update`.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Msg {
	/// Выделение в списке изменилось.
	Picked(usize),
	/// Выбрана вкладка.
	Tab(usize),
	/// Двойной клик по пункту списка (ADR-0035): к этому моменту пункт уже выделен.
	Opened,
	OpenDialog,
	CloseDialog,
}

#[derive(Default)]
struct Demo {
	backend: &'static str,
	tabs: TabsState,
	list: ListState,
	table: TableState,
	open: ButtonState,
	ok: ButtonState,
	dialog: bool,
	/// Пункт, открытый двойным кликом, — для строки подсказки.
	opened: Option<usize>,
	/// Комбинация, которую сейчас набирает пользователь; кормится из `on_key` через описание.
	tracker: HotkeyTracker,
	/// Карта сочетаний экрана (ADR-0029).
	keys: HotkeyMap<Msg>,
	/// Карта сочетаний модального окна: пока оно открыто, действует только она.
	dialog_keys: HotkeyMap<Msg>,
}

impl Demo {
	/// Описание экрана: строится на каждое событие и кадр из ссылок на состояния (ADR-0026).
	fn ui(&mut self) -> Ui<'_, Msg> {
		let selected = self
			.list
			.selected()
			.map_or("—", |i| ITEMS[i.min(ITEMS.len() - 1)]);
		let dim_border = Style::new().fg(DIM);
		let focus = Style::new().fg(ACCENT);
		// Значения из состояний — до того, как описание возьмёт их по `&mut`.
		let page = self.tabs.selected();
		let position = self.list.selected().map_or(0, |i| i + 1);
		let ratio = position as f64 / ITEMS.len() as f64;

		let list = List::new(ITEMS)
			.highlight_symbol("▶ ")
			.highlight_style(Style::new().bg(SELECTED_BG).bold())
			// Полоса принадлежит списку (ADR-0032): он рисует её с точным смещением своего кадра
			// и сам обрабатывает мышь на ней — стрелки, дорожку, протяжку ползунка.
			.scrollbar(
				Scrollbar::new(ScrollbarOrientation::Vertical)
					.style(dim_border)
					.thumb_style(focus),
			);
		let left = Ui::group(
			Block::bordered()
				.title(" Возможности gterm ")
				.border_type(BorderType::Rounded)
				.border_style(dim_border),
			Layout::vertical([Constraint::Fill(1)]),
			vec![
				Ui::stateful(list, &mut self.list)
					.on_select(Msg::Picked)
					.on_double_click(Msg::Opened),
			],
		)
		.focus_style(focus);

		let rows = (1..=12).map(|i| {
			Row::new([
				format!("{i:02}"),
				format!("задача-{i}"),
				["ok", "wait", "run"][i % 3].to_owned(),
				format!("{:>5}", i * 37 % 1000),
			])
		});
		let table = Table::new(
			rows,
			[
				Constraint::Length(3),
				Constraint::Fill(2),
				Constraint::Length(6),
				Constraint::Min(5),
			],
		)
		.header(Row::new(["№", "имя", "статус", "кадры"]).style(Style::new().fg(ACCENT).bold()))
		.block(
			Block::bordered()
				.title(" Таблица ")
				.title(Line::from("двойная рамка").right_aligned())
				.border_type(BorderType::Double)
				.border_style(dim_border),
		)
		.focus_style(focus)
		.highlight_symbol("» ")
		.row_highlight_style(Style::new().bg(SELECTED_BG))
		.scrollbar(
			Scrollbar::new(ScrollbarOrientation::Vertical)
				.style(dim_border)
				.thumb_style(focus),
		);

		let paragraph = Paragraph::new(Text::raw(LOREM))
			.block(
				Block::bordered()
					.title(" Абзац с переносом ")
					.border_type(BorderType::Thick)
					.padding(Padding::horizontal(1)),
			)
			.wrap(Wrap { trim: true });

		let open = Button::new("Открыть окно")
			.style(Style::new().fg(BG).bg(DIM))
			.focus_style(Style::new().bg(ACCENT))
			.pressed_style(Style::new().bg(SELECTED_BG).fg(FG));
		let right = Ui::split(
			Layout::vertical([
				Constraint::Length(11),
				Constraint::Fill(1),
				Constraint::Length(1),
			]),
			vec![
				Ui::stateful(table, &mut self.table),
				Ui::widget(paragraph),
				Ui::split(
					Layout::horizontal([Constraint::Fill(1), Constraint::Length(16)]),
					vec![
						Ui::widget(Paragraph::new("")),
						Ui::stateful(open, &mut self.open).on_activate(Msg::OpenDialog),
					],
				),
			],
		);

		let hint = Paragraph::new(Line::from(vec![
			Span::styled(
				format!(" {} ", self.backend),
				Style::new().fg(BG).bg(ACCENT),
			),
			Span::styled(
				"  Tab фокус  ←→ вкладки  ↑↓ выделение  клик/колесо  P — окно  Esc выход",
				Style::new().fg(DIM),
			),
		]));

		let tabs = Tabs::new(["Список и таблица", "Индикаторы"])
			.style(dim_border)
			.highlight_style(Style::new().fg(FG).bold().underlined())
			.focus_style(focus);
		let body = if page == 0 {
			Ui::split(
				Layout::horizontal([Constraint::Percentage(35), Constraint::Fill(1)]),
				vec![left, right],
			)
		} else {
			Ui::split(
				Layout::vertical([
					Constraint::Length(3),
					Constraint::Length(1),
					Constraint::Length(1),
					Constraint::Length(1),
					Constraint::Length(1),
					Constraint::Length(1),
					Constraint::Fill(1),
				]),
				vec![
					Ui::widget(
						Gauge::new()
							.ratio(ratio)
							.gauge_style(Style::new().fg(ACCENT).bg(SELECTED_BG))
							.block(
								Block::bordered()
									.title(" Gauge: позиция в списке ")
									.border_style(dim_border),
							),
					),
					Ui::widget(Paragraph::new("")),
					Ui::widget(
						LineGauge::new()
							.ratio(ratio)
							.label(format!(" LineGauge — {selected}:"))
							.filled_style(focus)
							.unfilled_style(dim_border),
					),
					Ui::widget(Paragraph::new("")),
					Ui::widget(
						Meter::new()
							.ratio(ratio)
							.peak(Some(ratio + METER_PEAK_LEAD))
							.zones(&METER_ZONES)
							.unfilled_style(dim_border),
					),
					Ui::widget(Paragraph::new("")),
					Ui::widget(
						Paragraph::new(Text::raw(INDICATORS))
							.block(
								Block::bordered()
									.title(" Индикаторы ")
									.border_style(dim_border)
									.padding(Padding::horizontal(1)),
							)
							.wrap(Wrap { trim: true }),
					),
				],
			)
		};
		let opened = match self.opened {
			Some(index) => format!("открыт «{}»", ITEMS[index.min(ITEMS.len() - 1)]),
			None => "двойной клик — открыть".to_owned(),
		};
		let progress = LineGauge::new()
			.ratio(ratio)
			.label(format!(" выделено {position} из {}, {opened}", ITEMS.len()))
			.filled_style(focus)
			.unfilled_style(dim_border);

		let screen = Ui::split(
			Layout::vertical([
				Constraint::Length(1),
				Constraint::Fill(1),
				Constraint::Length(1),
				Constraint::Length(1),
			]),
			vec![
				Ui::stateful(tabs, &mut self.tabs).on_select(Msg::Tab),
				body,
				Ui::widget(progress),
				Ui::widget(hint),
			],
		)
		.hotkeys(&mut self.tracker, &self.keys);
		if !self.dialog {
			return screen;
		}

		let ok = Button::new("OK")
			.style(Style::new().fg(Color::WHITE).bg(SELECTED_BG))
			.focus_style(Style::new().fg(BG).bg(ACCENT))
			.pressed_style(Style::new().bg(DIM));
		let content = Ui::group(
			Block::bordered()
				.title(" Окно ")
				.border_type(BorderType::Rounded)
				.border_style(Style::new().fg(ACCENT)),
			Layout::vertical([
				Constraint::Length(1),
				Constraint::Length(1),
				Constraint::Length(1),
				Constraint::Fill(1),
				Constraint::Length(1),
			]),
			vec![
				Ui::widget(Paragraph::new(
					Line::from(vec![
						Span::raw("Выбрано в списке: "),
						Span::styled(selected, Style::new().fg(ACCENT).bold()),
					])
					.centered(),
				)),
				Ui::widget(Paragraph::new(
					Line::styled(
						"Окно модальное: ввод идёт только ему, клик мимо поглощается.",
						Style::new().fg(DIM),
					)
					.centered(),
				)),
				Ui::widget(Paragraph::new(
					Line::raw("Esc, P или OK — закрыть.").centered(),
				)),
				Ui::widget(Paragraph::new("")),
				Ui::split(
					Layout::horizontal([
						Constraint::Fill(1),
						Constraint::Length(8),
						Constraint::Fill(1),
					]),
					vec![
						Ui::widget(Paragraph::new("")),
						Ui::stateful(ok, &mut self.ok).on_activate(Msg::CloseDialog),
					],
				),
			],
		);
		screen.modal(
			Modal::centered(64, 9)
				.style(Style::new().fg(Color::WHITE).bg(DIALOG_BG))
				.on_escape(Msg::CloseDialog)
				.hotkeys(&self.dialog_keys)
				.content(content),
		)
	}

	fn update(&mut self, message: Msg) {
		match message {
			// Выделение и вкладка уже в состояниях контролов; описание читает их при построении.
			Msg::Picked(_) | Msg::Tab(_) => {}
			// Первое нажатие пары уже выделило пункт под указателем.
			Msg::Opened => self.opened = self.list.selected(),
			Msg::OpenDialog => self.dialog = true,
			Msg::CloseDialog => self.dialog = false,
		}
	}

	fn draw(&mut self, screen: &mut ScreenBuffer) {
		screen.clear_all();
		let area = Rect::of_screen(screen);
		self.ui().render(area, screen);
	}
}

impl AppHandler for Demo {
	fn on_backend_changed(&mut self, name: &'static str, ctx: &mut Ctx<'_>) {
		self.backend = name;
		if self.list.selected().is_none() {
			self.list.select_first();
			self.table.select(Some(0));
		}
		self.draw(ctx.screen);
		ctx.redraw();
	}

	fn on_grid_resized(&mut self, _old: Grid, ctx: &mut Ctx<'_>) {
		// FR-23: содержимое сохранилось в пересекающейся области, достраиваем под новый размер.
		self.draw(ctx.screen);
		ctx.redraw();
	}

	fn on_key(&mut self, event: &KeyEvent, ctx: &mut Ctx<'_>) {
		// Порядок ADR-0026: маршрутизация → описание сброшено → разбор сообщений → отрисовка.
		// Описание получает и отпускания: ими кормится трекер карты сочетаний (ADR-0029).
		let routed = self.ui().key(event, ctx);
		if event.state != ElementState::Pressed {
			return;
		}
		let handled = routed.handled;
		for message in routed {
			self.update(message);
		}
		if !handled.is_yes() && matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
			ctx.exit();
		}
		self.draw(ctx.screen);
		ctx.redraw();
	}

	fn on_focus(&mut self, focused: bool, _ctx: &mut Ctx<'_>) {
		// Отпускания вне окна не придут — без сброса клавиша «зависла» бы в трекере.
		if !focused {
			self.tracker.reset();
		}
	}

	fn on_pointer(&mut self, event: &PointerEvent<'_>, ctx: &mut Ctx<'_>) {
		let routed = self.ui().pointer(event, ctx);
		// Движение мимо контролов не принято и сообщений не даёт — кадр не трогаем.
		let changed = routed.handled.is_yes() || !routed.messages.is_empty();
		for message in routed {
			self.update(message);
		}
		if changed {
			self.draw(ctx.screen);
			ctx.redraw();
		}
	}

	fn on_frame(&mut self, ctx: &mut Ctx<'_>) {
		// Кнопка удержана на стрелке или дорожке полосы прокрутки (поправка ADR-0032): контрол
		// листает по времени и просит следующий кадр. Без удержания такт ничего не делает, и
		// кадры в простое не идут.
		let routed = self.ui().tick(Instant::now());
		if routed.handled.is_yes() {
			for message in routed {
				self.update(message);
			}
			self.draw(ctx.screen);
			ctx.redraw();
		}
	}

	fn on_redraw(&mut self, ctx: &mut DrawCtx<'_>) {
		self.ui().draw_modal(ctx);
	}
}
