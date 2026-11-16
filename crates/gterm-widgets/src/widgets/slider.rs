//! Ползунок (ADR-0036): горизонтальная дорожка на ширину своей области с заполненной частью и
//! ползунком; целое значение в границах `min..=max`, шаг для клавиш и колеса. Под фокусом `←`/`→`
//! меняют значение на шаг, `Home`/`End` ставят границы; левое нажатие на дорожке ставит значение по
//! положению указателя и берёт захват, движение под захватом ведёт значение до отпускания, уход
//! указателя из окна захват снимает (значение остаётся); колесо — на шаг. Новое значение дерево
//! превращает в сообщение привязки `on_value`; программная установка события не даёт — как у
//! флажка.

use gterm::buffer::ScreenBuffer;
use winit::keyboard::NamedKey;

use crate::buffer::BufferExt;
use crate::control::{Control, NodeState, WidgetEvent, WidgetEvents};
use crate::geometry::Rect;
use crate::input::{Handled, KeyMsg, Messages, MouseKind, MouseMsg};
use crate::style::Style;
use crate::widgets::StatefulWidget;

/// Заполненная часть дорожки — левее ползунка.
const HEAVY_HORIZONTAL: char = '━';
/// Ползунок — ячейка текущего значения.
const FULL_BLOCK: char = '█';
/// Незаполненная часть — правее ползунка. Все три знака рисуются процедурно (ADR-0022).
const LIGHT_HORIZONTAL: char = '─';

/// Состояние ползунка: значение, границы и шаг, активен ли; фокус, область и захват — от дерева.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SliderState {
	pub node: NodeState,
	/// Неактивный ползунок не фокусируется и не реагирует на ввод.
	pub enabled: bool,
	value: i64,
	min: i64,
	max: i64,
	step: i64,
	event: Option<WidgetEvent>,
}

/// Проценты: 0…100, значение 0, шаг 1 — чтобы состояние приложения могло выводить `Default`.
impl Default for SliderState {
	fn default() -> Self {
		Self::new(0, 100)
	}
}

impl SliderState {
	/// Ползунок в границах `min..=max` (при `max < min` верхняя граница поднимается до нижней),
	/// значение — нижняя граница, шаг — 1.
	pub fn new(min: i64, max: i64) -> Self {
		Self {
			node: NodeState::default(),
			enabled: true,
			value: min,
			min,
			max: max.max(min),
			step: 1,
			event: None,
		}
	}

	/// Начальное значение (зажимается в границы).
	pub fn with_value(mut self, value: i64) -> Self {
		self.set_value(value);
		self
	}

	/// Шаг клавиш и колеса (не меньше 1).
	pub fn with_step(mut self, step: i64) -> Self {
		self.set_step(step);
		self
	}

	pub fn value(&self) -> i64 {
		self.value
	}

	pub fn min(&self) -> i64 {
		self.min
	}

	pub fn max(&self) -> i64 {
		self.max
	}

	pub fn step(&self) -> i64 {
		self.step
	}

	/// Задать значение (зажимается в границы). Программная смена — дело приложения, события
	/// `ValueChanged` она не даёт: события отражают ввод пользователя.
	pub fn set_value(&mut self, value: i64) {
		self.value = value.clamp(self.min, self.max);
	}

	/// Сменить границы (при `max < min` верхняя поднимается до нижней); значение зажимается в
	/// новые границы без события.
	pub fn set_range(&mut self, min: i64, max: i64) {
		self.min = min;
		self.max = max.max(min);
		self.set_value(self.value);
	}

	pub fn set_step(&mut self, step: i64) {
		self.step = step.max(1);
	}

	/// Смена пользователем: новое значение уходит событием, если оно изменилось.
	fn change_by_user(&mut self, value: i64) {
		let value = value.clamp(self.min, self.max);
		if value != self.value {
			self.value = value;
			self.event = Some(WidgetEvent::ValueChanged(value));
		}
	}

	/// Значение на `steps` шагов от текущего (отрицательное — вниз).
	fn stepped(&self, steps: i64) -> i64 {
		self.value.saturating_add(steps.saturating_mul(self.step))
	}

	/// Значение под колонкой `col` дорожки области узла: левая ячейка — `min`, правая — `max`,
	/// между ними — пропорционально с округлением; указатель за краями — граница. У дорожки в
	/// одну ячейку положения нет — значение прежнее.
	fn value_at(&self, col: i32) -> i64 {
		let area = self.node.area;
		let last = i128::from(area.width) - 1;
		if last <= 0 {
			return self.value;
		}
		let offset = (i128::from(col) - i128::from(area.x)).clamp(0, last);
		let range = i128::from(self.max) - i128::from(self.min);
		// Округление к ближайшему: все величины неотрицательны.
		let value = i128::from(self.min) + (2 * offset * range + last) / (2 * last);
		i64::try_from(value).unwrap_or(self.max)
	}

	/// Колонка ползунка от левого края дорожки шириной `width` — обратное к [`Self::value_at`].
	fn thumb_offset(&self, width: u16) -> u16 {
		let last = i128::from(width) - 1;
		let range = i128::from(self.max) - i128::from(self.min);
		if last <= 0 || range == 0 {
			return 0;
		}
		let shift = i128::from(self.value) - i128::from(self.min);
		let offset = (2 * shift * last + range) / (2 * range);
		u16::try_from(offset).unwrap_or(width - 1)
	}
}

impl<M> Control<M> for SliderState {
	fn node(&self) -> &NodeState {
		&self.node
	}

	fn node_mut(&mut self) -> &mut NodeState {
		&mut self.node
	}

	fn focusable(&self) -> bool {
		self.enabled
	}

	/// `←`/`→` — на шаг, `Home`/`End` — к границам; в границе клавиша принята, но значение и
	/// событий нет.
	fn key(&mut self, msg: &KeyMsg<'_>, _out: &mut Messages<M>) -> Handled {
		if !self.enabled {
			return Handled::No;
		}
		let target = if msg.is_named(NamedKey::ArrowLeft) {
			self.stepped(-1)
		} else if msg.is_named(NamedKey::ArrowRight) {
			self.stepped(1)
		} else if msg.is_named(NamedKey::Home) {
			self.min
		} else if msg.is_named(NamedKey::End) {
			self.max
		} else {
			return Handled::No;
		};
		self.change_by_user(target);
		Handled::Yes
	}

	fn mouse(&mut self, msg: &MouseMsg, _out: &mut Messages<M>) -> Handled {
		if !self.enabled {
			return Handled::No;
		}
		match msg.kind {
			MouseKind::Down if msg.is_left_down() => {
				self.change_by_user(self.value_at(msg.col));
				Handled::Yes
			}
			// Протяжка: нажатие принято, ползунок держит захват до отпускания или ухода указателя.
			MouseKind::Move if self.node.captured => {
				self.change_by_user(self.value_at(msg.col));
				Handled::Yes
			}
			MouseKind::Up | MouseKind::Cancel if self.node.captured => Handled::Yes,
			// Колесо вверх (положительные строки, соглашение `winit`) увеличивает значение.
			MouseKind::Wheel { lines } => {
				self.change_by_user(self.stepped(lines.round() as i64));
				Handled::Yes
			}
			_ => Handled::No,
		}
	}

	/// Неактивный ползунок ввода не принимает — и двойного клика тоже (ADR-0035).
	fn accepts_double_click(&self, _msg: &MouseMsg) -> bool {
		self.enabled
	}
}

impl WidgetEvents for SliderState {
	fn take_event(&mut self) -> Option<WidgetEvent> {
		self.event.take()
	}
}

/// Ползунок: дорожка в средней строке области — заполненная часть, ползунок, незаполненная часть;
/// стиль состояния (неактивный либо под фокусом) накладывается поверх стилей частей.
#[derive(Clone, Copy, Debug)]
pub struct Slider {
	style: Style,
	filled_style: Style,
	thumb_style: Style,
	unfilled_style: Style,
	focus_style: Style,
	disabled_style: Style,
	filled_symbol: char,
	thumb_symbol: char,
	unfilled_symbol: char,
}

impl Default for Slider {
	fn default() -> Self {
		Self {
			style: Style::new(),
			filled_style: Style::new(),
			thumb_style: Style::new(),
			unfilled_style: Style::new(),
			focus_style: Style::new(),
			disabled_style: Style::new(),
			filled_symbol: HEAVY_HORIZONTAL,
			thumb_symbol: FULL_BLOCK,
			unfilled_symbol: LIGHT_HORIZONTAL,
		}
	}
}

impl Slider {
	pub fn new() -> Self {
		Self::default()
	}

	/// Стиль всей области.
	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	/// Стиль заполненной части — левее ползунка.
	pub fn filled_style(mut self, style: Style) -> Self {
		self.filled_style = style;
		self
	}

	pub fn thumb_style(mut self, style: Style) -> Self {
		self.thumb_style = style;
		self
	}

	/// Стиль незаполненной части — правее ползунка.
	pub fn unfilled_style(mut self, style: Style) -> Self {
		self.unfilled_style = style;
		self
	}

	/// Накладывается на всю область, пока ползунок под фокусом.
	pub fn focus_style(mut self, style: Style) -> Self {
		self.focus_style = style;
		self
	}

	/// Накладывается у неактивного ползунка вместо стиля фокуса.
	pub fn disabled_style(mut self, style: Style) -> Self {
		self.disabled_style = style;
		self
	}

	pub fn filled_symbol(mut self, symbol: char) -> Self {
		self.filled_symbol = symbol;
		self
	}

	pub fn thumb_symbol(mut self, symbol: char) -> Self {
		self.thumb_symbol = symbol;
		self
	}

	pub fn unfilled_symbol(mut self, symbol: char) -> Self {
		self.unfilled_symbol = symbol;
		self
	}
}

impl StatefulWidget for Slider {
	type State = SliderState;

	fn render(self, area: Rect, buf: &mut ScreenBuffer, state: &mut SliderState) {
		let area = area.intersection(buf.area());
		if area.is_empty() {
			return;
		}
		let state_style = if !state.enabled {
			self.disabled_style
		} else if state.node.focused {
			self.focus_style
		} else {
			Style::new()
		};
		buf.set_style(area, self.style.patch(state_style));
		let row = area.y + area.height / 2;
		let thumb = state.thumb_offset(area.width);
		for cell in 0..area.width {
			let (symbol, part) = if cell < thumb {
				(self.filled_symbol, self.filled_style)
			} else if cell == thumb {
				(self.thumb_symbol, self.thumb_style)
			} else {
				(self.unfilled_symbol, self.unfilled_style)
			};
			let style = self.style.patch(part).patch(state_style);
			buf.set_styled(area.x + cell, row, symbol, style, area);
		}
	}
}
