//! Контракт для приложений (ADR-0004, ADR-0006, ADR-0014): трейт [`AppHandler`], [`Ctx`],
//! [`DrawCtx`], [`Request`], [`PointerEvent`], [`FrameStats`].

use std::time::Duration;

use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta};
use winit::keyboard::ModifiersState;

use super::Grid;
use crate::buffer::ScreenBuffer;
use crate::overlay::Overlay;
use crate::rasterizer::BoxDrawingMode;
use crate::renderer::RendererKind;

/// Событие указателя: мышь приложения, сгруппированная без перевода типов (FR-25, ADR-0004).
///
/// `cell` — ячейка сетки под указателем по последнему известному положению (ADR-0026): столбец
/// и строка, деление на размер ячейки с округлением вниз; отрицательные значения и значения за
/// краем сетки означают «вне сетки» (поле вокруг сетки, окно шире сетки).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointerEvent<'a> {
	/// Движение указателя; координаты физические, как их отдаёт `winit`.
	Moved {
		position: PhysicalPosition<f64>,
		delta: PhysicalPosition<f64>,
		cell: (i32, i32),
	},
	/// Кнопка нажата или отпущена.
	Input {
		state: ElementState,
		button: MouseButton,
		cell: (i32, i32),
	},
	/// Колесо.
	Wheel {
		delta: &'a MouseScrollDelta,
		cell: (i32, i32),
	},
	/// Указатель покинул окно.
	Left,
}

/// Команды приложения библиотеке, накопленные за обработчик и применённые после него (FR-25).
#[derive(Debug, Default)]
pub struct Request {
	/// Задать сетку; содержимое сохраняется в пересекающейся области (FR-6).
	///
	/// Действует только в [`GridMode::FixedGrid`](super::GridMode::FixedGrid): в
	/// [`GridMode::AutoGrid`](super::GridMode::AutoGrid) сетка выводится из
	/// окна, и следующая же перерисовка по размеру окна вернула бы заданное значение обратно.
	pub grid: Option<Grid>,
	/// Переключить бэкенд на лету (FR-21, FR-28).
	pub backend: Option<RendererKind>,
	/// Включить или выключить полный экран (FR-22).
	pub fullscreen: Option<bool>,
	/// Сменить источник псевдографики (рамки, мозаики, фигуры) на лету (ADR-0018): атлас сбрасывается,
	/// кадр перерисовывается целиком.
	pub box_drawing: Option<BoxDrawingMode>,
	/// Сменить период тика (ADR-0033): `Some(Some(period))` задаёт его, `Some(None)` выключает
	/// тик; отсчёт начинается с момента применения запроса. Период короче 1 мс считается за 1 мс.
	pub tick_period: Option<Option<Duration>>,
	/// Запросить перерисовку.
	pub redraw: bool,
	/// Завершить цикл событий.
	pub exit: bool,
}

/// Статистика кадров (ADR-0006): считается библиотекой без аллокаций, читается приложением
/// через [`Ctx::stats`]. Основа счётчика FPS в демо (FR-27) и замера `--bench` (NFR-4, NFR-5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameStats {
	/// Отрисованных кадров с запуска.
	pub frames: u64,
	/// CPU-часть последнего кадра: на GL — сборка данных, загрузка текстур и draw call без
	/// ожидания VSync (то, что меряет NFR-4); на Software — растеризация кадра без копирования
	/// в окно.
	pub cpu: Duration,
	/// Весь последний кадр, включая `swap_buffers` (GL) или копирование и `present` (Software).
	pub total: Duration,
}

/// Всё, что приложение получает вместе с событием: экран, команды и статистика
/// (ADR-0004, ADR-0006).
pub struct Ctx<'a> {
	/// Экран приложения — тот же буфер, который рендерится.
	pub screen: &'a mut ScreenBuffer,
	/// Команды библиотеке; применяются после возврата из обработчика.
	pub request: &'a mut Request,
	/// Статистика кадров на момент события.
	pub stats: FrameStats,
	/// Состояние модификаторов (`Shift`, `Ctrl`, `Alt`, `Super`) на момент события
	/// (ADR-0026): `winit` присылает их отдельным событием до клавиши, библиотека хранит
	/// последнее. `KeyEvent` в `on_key` модификаторов не несёт.
	pub modifiers: ModifiersState,
	/// Размер ячейки в физических пикселях — метрики атласа активного бэкенда (ADR-0026);
	/// `(1, 1)` до создания бэкенда.
	pub cell_px: (u32, u32),
}

impl Ctx<'_> {
	/// Запросить перерисовку — нужно после любой записи в экран.
	pub fn redraw(&mut self) {
		self.request.redraw = true;
	}

	/// Завершить цикл событий.
	pub fn exit(&mut self) {
		self.request.exit = true;
	}
}

/// Контекст отрисовки наложения (ADR-0014): живёт только внутри [`AppHandler::on_redraw`].
pub struct DrawCtx<'a> {
	pub(super) overlay: &'a mut Overlay,
	pub(super) grid: Grid,
}

impl DrawCtx<'_> {
	/// Нарисовать `buffer` поверх основного экрана и уже нарисованных в этом кадре буферов;
	/// `(col, row)` — ячейка основной сетки под левым верхним углом буфера, может быть
	/// отрицательной. Всё за пределами сетки обрезается. Буфер копируется — его можно менять
	/// сразу после вызова. Прозрачный буфер — [`ScreenBuffer::transparent`].
	pub fn draw_buffer(&mut self, buffer: &ScreenBuffer, col: i32, row: i32) {
		self.overlay.push(buffer, col, row);
	}

	/// Сетка основного экрана — для расчёта позиции.
	pub fn grid(&self) -> Grid {
		self.grid
	}
}

/// Приложение: ввод и уведомления без интерпретации (FR-25, ADR-0004).
///
/// Все методы имеют тело по умолчанию — реализовывать нужно только то, что нужно приложению.
/// `Esc` приходит сюда как обычное нажатие: решать, что делать, должен вызывающий.
pub trait AppHandler {
	/// Нажатие или отпускание клавиши.
	fn on_key(&mut self, _event: &KeyEvent, _ctx: &mut Ctx<'_>) {}

	/// Текстовый ввод (`Ime::Commit`).
	fn on_text(&mut self, _text: &str, _ctx: &mut Ctx<'_>) {}

	/// Окно получило или потеряло фокус ввода (ADR-0029). Отпускания клавиш, случившиеся вне
	/// окна, сюда не придут: приложение с `HotkeyTracker` сбрасывает его при `focused == false`.
	fn on_focus(&mut self, _focused: bool, _ctx: &mut Ctx<'_>) {}

	/// Мышь: движение, кнопки, колесо, выход за окно.
	fn on_pointer(&mut self, _event: &PointerEvent<'_>, _ctx: &mut Ctx<'_>) {}

	/// Сетка изменилась; содержимое уже сохранено, размеры — новые (FR-23).
	fn on_grid_resized(&mut self, _old: Grid, _ctx: &mut Ctx<'_>) {}

	/// Сменился бэкенд: при запуске и при откате (FR-20, FR-21).
	fn on_backend_changed(&mut self, _name: &'static str, _ctx: &mut Ctx<'_>) {}

	/// Кадр сейчас будет отрисован (ADR-0014): наложение пусто, всё нарисованное через
	/// `ctx.draw_buffer` ляжет поверх основного экрана только в этом кадре. Вызывается на каждом
	/// `RedrawRequested`, в том числе при повторе кадра после ошибки.
	fn on_redraw(&mut self, _ctx: &mut DrawCtx<'_>) {}

	/// Прошёл период тика (ADR-0033), заданный в
	/// [`AppOptions::tick_period`](super::AppOptions::tick_period) или
	/// [`Request::tick_period`]. Кадр тик не запрашивает: записав в экран, приложение зовёт
	/// `ctx.redraw()`. Пропущенные тики не догоняются — после задержки приходит один вызов,
	/// следующий — через период от него. Без заданного периода метод не вызывается.
	fn on_tick(&mut self, _ctx: &mut Ctx<'_>) {}

	/// Кадр отрисован (ADR-0006): `ctx.stats` уже учитывает его. Безусловный `ctx.redraw()`
	/// отсюда означает непрерывную перерисовку — так работает замер `--bench`; обычному
	/// приложению этот метод не нужен, и в простое он не вызывается (FR-26).
	fn on_frame(&mut self, _ctx: &mut Ctx<'_>) {}
}
