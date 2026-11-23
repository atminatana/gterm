//! Полоса прокрутки (ADR-0032). Сама по себе — только отображение: положение и размеры задаёт
//! значение [`ScrollbarState`], фокуса и ввода у полосы нет. Прокручиваемый контрол
//! (`List::scrollbar`, `Table::scrollbar`) рисует полосу сам, с точными значениями своего кадра,
//! и сам обрабатывает мышь на ней ([`BarState`] по разметке [`BarLayout`]): стрелки, дорожка,
//! протяжка ползунка, колесо, повтор листания при удержанной кнопке (по тактам `Ui::tick`). Вертикальная
//! полоса рисуется в последней колонке области, горизонтальная — в последней строке. Стрелки,
//! дорожка и ползунок — процедурные знаки (ADR-0002, ADR-0022), от шрифта не зависят.

use std::time::{Duration, Instant};

use gterm::buffer::ScreenBuffer;

use crate::buffer::BufferExt;
use crate::geometry::Rect;
use crate::input::MouseMsg;
use crate::style::Style;
use crate::theme::{Theme, fg_only, scope};
use crate::widgets::Widget;

/// Что прокручивается: длина содержимого, смещение первого видимого элемента и сколько
/// элементов видно. Единицы — любые, одни и те же для всех трёх (строки, символы).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScrollbarState {
	pub content_length: usize,
	pub position: usize,
	pub viewport_length: usize,
}

impl ScrollbarState {
	pub fn new(content_length: usize) -> Self {
		Self {
			content_length,
			..Self::default()
		}
	}

	/// Смещение первого видимого элемента; больше наибольшего возможного — подрезается.
	pub fn position(mut self, position: usize) -> Self {
		self.position = position;
		self
	}

	/// Сколько элементов видно; ноль считается единицей.
	pub fn viewport_length(mut self, viewport_length: usize) -> Self {
		self.viewport_length = viewport_length;
		self
	}

	/// Наибольшее смещение: при нём виден конец содержимого.
	fn last_position(&self) -> usize {
		self.content_length
			.saturating_sub(self.viewport_length.max(1))
	}

	/// Ползунок на дорожке длиной `track`: `(начало, длина)` в ячейках. Содержимое, которое
	/// помещается целиком, даёт ползунок во всю дорожку.
	fn thumb(&self, track: u16) -> (u16, u16) {
		let track = usize::from(track);
		let viewport = self.viewport_length.max(1);
		let last_position = self.last_position();
		if track == 0 || last_position == 0 {
			return (0, track as u16);
		}
		let length = rounded_div(track * viewport, self.content_length).clamp(1, track);
		let position = self.position.min(last_position);
		let start = rounded_div(position * (track - length), last_position);
		(start as u16, length as u16)
	}
}

/// Деление с округлением к ближайшему.
fn rounded_div(numerator: usize, denominator: usize) -> usize {
	(numerator + denominator / 2) / denominator
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollbarOrientation {
	/// В последней колонке области.
	#[default]
	Vertical,
	/// В последней строке области.
	Horizontal,
}

/// Разметка нарисованной полосы — по ней контрол-владелец разбирает мышь. Координаты «вдоль» —
/// по оси полосы (строки у вертикальной, колонки у горизонтальной), «поперёк» — вторая ось.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct BarLayout {
	is_vertical: bool,
	/// Координата полосы поперёк оси (колонка вертикальной полосы).
	cross: u16,
	/// Первая ячейка полосы вдоль оси и её длина вместе со стрелками.
	start: u16,
	length: u16,
	has_arrows: bool,
	/// Дорожка: первая ячейка вдоль оси и длина.
	track_start: u16,
	track_length: u16,
	/// Ползунок: смещение от начала дорожки и длина.
	thumb_start: u16,
	thumb_length: u16,
	/// Наибольшее смещение содержимого и размер страницы (сколько элементов видно).
	pub(crate) last_position: usize,
	pub(crate) page: usize,
}

/// Часть полосы под указателем.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BarPart {
	/// Стрелка в начале: на элемент назад.
	Begin,
	/// Стрелка в конце: на элемент вперёд.
	End,
	/// Дорожка перед ползунком: на страницу назад.
	Before,
	/// Дорожка за ползунком: на страницу вперёд.
	After,
	/// Ползунок; `grab` — за какую его ячейку взялись (от начала ползунка).
	Thumb { grab: u16 },
}

impl BarLayout {
	/// Координата ячейки `(col, row)` вдоль оси, если ячейка лежит на линии полосы (поперёк).
	fn along(&self, col: i32, row: i32) -> Option<i32> {
		let (along, cross) = if self.is_vertical {
			(row, col)
		} else {
			(col, row)
		};
		(cross == i32::from(self.cross)).then_some(along)
	}

	/// Часть полосы под ячейкой; `None` — ячейка не на полосе.
	pub(crate) fn part_at(&self, col: i32, row: i32) -> Option<BarPart> {
		let along = u16::try_from(self.along(col, row)?).ok()?;
		let end = self.start + self.length;
		if !(self.start..end).contains(&along) {
			return None;
		}
		if self.has_arrows && along == self.start {
			return Some(BarPart::Begin);
		}
		if self.has_arrows && along == end - 1 {
			return Some(BarPart::End);
		}
		let on_track = along - self.track_start;
		Some(if on_track < self.thumb_start {
			BarPart::Before
		} else if on_track < self.thumb_start + self.thumb_length {
			BarPart::Thumb {
				grab: on_track - self.thumb_start,
			}
		} else {
			BarPart::After
		})
	}

	/// Смещение содержимого, при котором ячейка ползунка `grab` окажется под указателем
	/// `(col, row)`: протяжка. Указатель в стороне от полосы или за её краями — тоже годится,
	/// смещение подрезается.
	pub(crate) fn position_for_drag(&self, col: i32, row: i32, grab: u16) -> usize {
		let along = if self.is_vertical { row } else { col };
		let travel = usize::from(self.track_length - self.thumb_length);
		if travel == 0 {
			return 0;
		}
		let thumb_start = along - i32::from(self.track_start) - i32::from(grab);
		let thumb_start = usize::try_from(thumb_start).unwrap_or(0).min(travel);
		rounded_div(thumb_start * self.last_position, travel)
	}
}

/// Полоса в состоянии прокручиваемого контрола (список, таблица): разметка последней отрисовки,
/// протяжка ползунка и правило подгонки смещения под выделение. Одна реализация на оба
/// контрола, чтобы поведение мыши и колеса не расходилось.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct BarState {
	/// Разметка полосы последней отрисовки; `None` — полосы нет.
	layout: Option<BarLayout>,
	/// Ползунок тянут мышью: за какую его ячейку взялись.
	drag_grab: Option<u16>,
	/// При каких выделении, высоте и числе элементов смещение подгонялось под выделение: пока
	/// они те же, смещение свободно (его двигает полоса).
	fitted: Option<(Option<usize>, u16, usize)>,
	/// Кнопка удержана на стрелке или дорожке: листание повторяется по времени.
	hold: Option<Hold>,
}

/// Удержание кнопки на стрелке или дорожке полосы.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Hold {
	/// Что нажато: повтор идёт, пока под указателем та же часть полосы.
	part: BarPart,
	/// Где нажато (ячейка системы координат дерева).
	col: i32,
	row: i32,
	/// Когда листать в следующий раз; `None` — время ещё не приходило, отсчёт начнёт первый такт.
	next: Option<Instant>,
}

impl BarState {
	/// Начало отрисовки: полосы в этом кадре пока нет.
	pub(crate) fn begin_frame(&mut self) {
		self.layout = None;
	}

	pub(crate) fn set_layout(&mut self, layout: Option<BarLayout>) {
		self.layout = layout;
	}

	/// Подгонять ли в этом кадре смещение под выделение: да, если с прошлого кадра изменилось
	/// выделение, высота области или число элементов.
	pub(crate) fn take_fit(&mut self, selected: Option<usize>, height: u16, len: usize) -> bool {
		let key = (selected, height, len);
		let is_fitting = self.fitted != Some(key);
		self.fitted = Some(key);
		is_fitting
	}

	/// Смещение не дальше последней страницы; предел известен по последней отрисовке с
	/// полосой, без неё смещение подрежет следующая отрисовка.
	pub(crate) fn clamp(&self, offset: usize) -> usize {
		offset.min(
			self.layout
				.map_or(usize::MAX, |layout| layout.last_position),
		)
	}

	/// Лежит ли ячейка сообщения на полосе последней отрисовки.
	pub(crate) fn contains(&self, msg: &MouseMsg) -> bool {
		self.layout
			.is_some_and(|layout| layout.part_at(msg.col, msg.row).is_some())
	}

	/// Левое нажатие: новое смещение, если нажатие пришлось на полосу (стрелка — на элемент,
	/// дорожка — на страницу, ползунок — начало протяжки, смещение прежнее); `None` — мимо.
	pub(crate) fn press(&mut self, msg: &MouseMsg, offset: usize) -> Option<usize> {
		let layout = self.layout?;
		let part = layout.part_at(msg.col, msg.row)?;
		match part {
			BarPart::Thumb { grab } => self.drag_grab = Some(grab),
			_ => {
				self.hold = Some(Hold {
					part,
					col: msg.col,
					row: msg.row,
					next: None,
				});
			}
		}
		Some(self.clamp(Self::step(part, layout, offset)))
	}

	/// Смещение после одного шага по части полосы: стрелка — элемент, дорожка — страница.
	fn step(part: BarPart, layout: BarLayout, offset: usize) -> usize {
		match part {
			BarPart::Begin => offset.saturating_sub(1),
			BarPart::End => offset.saturating_add(1),
			BarPart::Before => offset.saturating_sub(layout.page),
			BarPart::After => offset.saturating_add(layout.page),
			BarPart::Thumb { .. } => offset,
		}
	}

	/// Такт времени при удержанной кнопке: смещение после повтора (или прежнее, если срок не
	/// подошёл); `None` — кнопка на стрелке или дорожке не удержана, время контролу не нужно.
	/// Первый такт после нажатия только заводит отсчёт: первый повтор — через
	/// [`Scrollbar::REPEAT_DELAY`], следующие — каждые [`Scrollbar::REPEAT_INTERVAL`]. Листание по
	/// дорожке останавливается, когда ползунок доходит до указателя.
	pub(crate) fn tick(&mut self, now: Instant, offset: usize) -> Option<usize> {
		let layout = self.layout?;
		let hold = self.hold.as_mut()?;
		let Some(next) = hold.next else {
			hold.next = Some(now + Scrollbar::REPEAT_DELAY);
			return Some(offset);
		};
		if now < next || layout.part_at(hold.col, hold.row) != Some(hold.part) {
			return Some(offset);
		}
		hold.next = Some(now + Scrollbar::REPEAT_INTERVAL);
		let part = hold.part;
		Some(self.clamp(Self::step(part, layout, offset)))
	}

	/// Движение при протяжке ползунка: новое смещение; `None` — ползунок не тянут.
	pub(crate) fn drag(&self, msg: &MouseMsg) -> Option<usize> {
		let layout = self.layout?;
		let grab = self.drag_grab?;
		Some(layout.position_for_drag(msg.col, msg.row, grab))
	}

	/// Отпускание или отмена: протяжка и удержание закончены.
	pub(crate) fn release(&mut self) {
		self.drag_grab = None;
		self.hold = None;
	}

	/// Колесо над контролом с полосой прокручивает содержимое: положительные строки — вверх
	/// (соглашение `winit`), на `round(lines)` элементов. `None` — полосы нет, колесо остаётся
	/// за выделением.
	pub(crate) fn wheel(&self, offset: usize, lines: f32) -> Option<usize> {
		self.layout?;
		let step = lines.round() as i64;
		let target = (offset as i64).saturating_sub(step).max(0) as usize;
		Some(self.clamp(target))
	}
}

/// Полоса прокрутки: стрелки по краям, дорожка и ползунок.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Scrollbar {
	orientation: ScrollbarOrientation,
	state: ScrollbarState,
	style: Style,
	thumb_style: Style,
	track_symbol: Option<char>,
	thumb_symbol: Option<char>,
	is_without_arrows: bool,
}

impl Scrollbar {
	/// Сколько кнопку держат на стрелке или дорожке до первого повтора листания.
	pub const REPEAT_DELAY: Duration = Duration::from_millis(400);
	/// Промежуток между повторами листания при удержанной кнопке.
	pub const REPEAT_INTERVAL: Duration = Duration::from_millis(50);

	pub fn new(orientation: ScrollbarOrientation) -> Self {
		Self {
			orientation,
			..Self::default()
		}
	}

	/// Что показывать. Полоса — виджет без отдельного состояния ([`Widget`], а не
	/// `StatefulWidget`): значение она несёт в себе. Полосе, отданной контролу
	/// (`List::scrollbar`), состояние не нужно — его подставит контрол.
	pub fn state(mut self, state: ScrollbarState) -> Self {
		self.state = state;
		self
	}

	/// Стиль стрелок и дорожки.
	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	/// Стиль ползунка поверх стиля полосы.
	pub fn thumb_style(mut self, style: Style) -> Self {
		self.thumb_style = style;
		self
	}

	/// Знак дорожки (по умолчанию `│` или `─` по ориентации).
	pub fn track_symbol(mut self, symbol: char) -> Self {
		self.track_symbol = Some(symbol);
		self
	}

	/// Знак ползунка (по умолчанию `█`).
	pub fn thumb_symbol(mut self, symbol: char) -> Self {
		self.thumb_symbol = Some(symbol);
		self
	}

	/// Рисовать ли стрелки по краям (по умолчанию да; на полосе короче трёх ячеек их нет).
	pub fn arrows(mut self, arrows: bool) -> Self {
		self.is_without_arrows = !arrows;
		self
	}

	/// Подставить стили темы под заданные явно (ADR-0037) — для виджета вне дерева `Ui`; в
	/// дереве то же делает [`Ui::theme`](crate::ui::Ui::theme).
	pub fn themed(mut self, theme: &Theme) -> Self {
		self.set_theme(theme);
		self
	}

	/// Области `ui.scrollbar` и `ui.scrollbar.thumb`; в теме без них дорожка — как рамка,
	/// ползунок — цвет символа полосы прокрутки меню helix (`ui.menu.scroll`).
	pub(crate) fn set_theme(&mut self, theme: &Theme) {
		let track = theme.get("ui.scrollbar").unwrap_or_else(|| theme.border());
		let thumb = theme
			.get("ui.scrollbar.thumb")
			.unwrap_or_else(|| fg_only(theme.first(&[scope::MENU_SCROLL])));
		self.style = track.patch(self.style);
		self.thumb_style = thumb.patch(self.thumb_style);
	}

	/// Разметка полосы в области `area` для состояния `state`; `None` — рисовать негде.
	fn layout(&self, area: Rect, state: &ScrollbarState) -> Option<BarLayout> {
		if area.is_empty() {
			return None;
		}
		let is_vertical = self.orientation == ScrollbarOrientation::Vertical;
		// Полоса — последняя колонка либо последняя строка области.
		let (cross, start, length) = if is_vertical {
			(area.right() - 1, area.y, area.height)
		} else {
			(area.bottom() - 1, area.x, area.width)
		};
		let has_arrows = !self.is_without_arrows && length >= 3;
		let (track_start, track_length) = if has_arrows {
			(start + 1, length - 2)
		} else {
			(start, length)
		};
		let (thumb_start, thumb_length) = state.thumb(track_length);
		Some(BarLayout {
			is_vertical,
			cross,
			start,
			length,
			has_arrows,
			track_start,
			track_length,
			thumb_start,
			thumb_length,
			last_position: state.last_position(),
			page: state.viewport_length.max(1),
		})
	}

	/// Нарисовать полосу и вернуть её разметку (контролу-владельцу — для мыши).
	pub(crate) fn draw(
		&self,
		area: Rect,
		buf: &mut ScreenBuffer,
		state: &ScrollbarState,
	) -> Option<BarLayout> {
		let area = area.intersection(buf.area());
		let layout = self.layout(area, state)?;
		let cell = |along: u16| {
			if layout.is_vertical {
				(layout.cross, along)
			} else {
				(along, layout.cross)
			}
		};
		let (begin, end, track_default) = if layout.is_vertical {
			('▲', '▼', '│')
		} else {
			('◀', '▶', '─')
		};
		if layout.has_arrows {
			let (x, y) = cell(layout.start);
			buf.set_styled(x, y, begin, self.style, area);
			let (x, y) = cell(layout.start + layout.length - 1);
			buf.set_styled(x, y, end, self.style, area);
		}
		let track_symbol = self.track_symbol.unwrap_or(track_default);
		let thumb_symbol = self.thumb_symbol.unwrap_or('█');
		let thumb = layout.thumb_start..layout.thumb_start + layout.thumb_length;
		for index in 0..layout.track_length {
			let (x, y) = cell(layout.track_start + index);
			if thumb.contains(&index) {
				let style = self.style.patch(self.thumb_style);
				buf.set_styled(x, y, thumb_symbol, style, area);
			} else {
				buf.set_styled(x, y, track_symbol, self.style, area);
			}
		}
		Some(layout)
	}
}

impl Widget for Scrollbar {
	fn apply_theme(&mut self, theme: &Theme) {
		self.set_theme(theme);
	}

	fn render(self, area: Rect, buf: &mut ScreenBuffer) {
		self.draw(area, buf, &self.state);
	}
}
