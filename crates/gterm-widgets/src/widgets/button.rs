//! Кнопка (ADR-0026): подпись по центру области, стили обычный/фокус/нажата/неактивная.
//! Активируется `Enter`, пробелом и мышью — отпусканием левой кнопки внутри области после
//! нажатия в ней (решение О4); отпускание снаружи или уход указателя — отмена. Событие
//! активации дерево превращает в сообщение привязки `on_activate`.

use gterm::buffer::ScreenBuffer;
use winit::keyboard::NamedKey;

use crate::buffer::BufferExt;
use crate::control::{Control, NodeState, WidgetEvent, WidgetEvents};
use crate::geometry::Rect;
use crate::input::{Handled, KeyMsg, Messages, MouseKind, MouseMsg};
use crate::style::Style;
use crate::text::{Alignment, Line, StyledLine};
use crate::widgets::StatefulWidget;

/// Состояние кнопки: активна ли, нажата ли мышью, фокус и область — от дерева.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ButtonState {
	pub node: NodeState,
	/// Неактивная кнопка не фокусируется и не реагирует на ввод.
	pub enabled: bool,
	/// Нажата мышью: от левого нажатия в области до отпускания или отмены.
	pub pressed: bool,
	event: Option<WidgetEvent>,
}

impl Default for ButtonState {
	fn default() -> Self {
		Self {
			node: NodeState::default(),
			enabled: true,
			pressed: false,
			event: None,
		}
	}
}

impl ButtonState {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn disabled() -> Self {
		Self {
			enabled: false,
			..Self::default()
		}
	}

	fn activate(&mut self) {
		self.event = Some(WidgetEvent::Activated);
	}

	fn is_inside(&self, msg: &MouseMsg) -> bool {
		let (Ok(col), Ok(row)) = (u16::try_from(msg.col), u16::try_from(msg.row)) else {
			return false;
		};
		self.node.area.contains(col, row)
	}
}

impl<M> Control<M> for ButtonState {
	fn node(&self) -> &NodeState {
		&self.node
	}

	fn node_mut(&mut self) -> &mut NodeState {
		&mut self.node
	}

	fn focusable(&self) -> bool {
		self.enabled
	}

	fn key(&mut self, msg: &KeyMsg<'_>, _out: &mut Messages<M>) -> Handled {
		if !self.enabled {
			return Handled::No;
		}
		if msg.is_named(NamedKey::Enter) || msg.is_named(NamedKey::Space) || msg.is_char(" ") {
			self.activate();
			return Handled::Yes;
		}
		Handled::No
	}

	fn mouse(&mut self, msg: &MouseMsg, _out: &mut Messages<M>) -> Handled {
		if !self.enabled {
			return Handled::No;
		}
		match msg.kind {
			MouseKind::Down if msg.is_left_down() => {
				self.pressed = true;
				Handled::Yes
			}
			MouseKind::Up if self.pressed => {
				self.pressed = false;
				if self.is_inside(msg) {
					self.activate();
				}
				Handled::Yes
			}
			MouseKind::Cancel => {
				self.pressed = false;
				Handled::Yes
			}
			MouseKind::Move if self.pressed => Handled::Yes,
			_ => Handled::No,
		}
	}

	/// Неактивная кнопка ввода не принимает — и двойного клика тоже (ADR-0035).
	fn accepts_double_click(&self, _msg: &MouseMsg) -> bool {
		self.enabled
	}
}

impl WidgetEvents for ButtonState {
	fn take_event(&mut self) -> Option<WidgetEvent> {
		self.event.take()
	}
}

/// Кнопка: подпись по центру; стиль по состоянию — неактивная, нажата, под фокусом, обычная.
#[derive(Clone, Debug, Default)]
pub struct Button<'a> {
	label: Line<'a>,
	style: Style,
	focus_style: Style,
	pressed_style: Style,
	disabled_style: Style,
}

impl<'a> Button<'a> {
	pub fn new<T: Into<Line<'a>>>(label: T) -> Self {
		Self {
			label: label.into(),
			..Self::default()
		}
	}

	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	/// Накладывается на основной стиль, пока кнопка под фокусом.
	pub fn focus_style(mut self, style: Style) -> Self {
		self.focus_style = style;
		self
	}

	/// Накладывается, пока кнопка нажата мышью (сильнее стиля фокуса).
	pub fn pressed_style(mut self, style: Style) -> Self {
		self.pressed_style = style;
		self
	}

	/// Накладывается у неактивной кнопки вместо стилей фокуса и нажатия.
	pub fn disabled_style(mut self, style: Style) -> Self {
		self.disabled_style = style;
		self
	}
}

impl StatefulWidget for Button<'_> {
	type State = ButtonState;

	fn render(self, area: Rect, buf: &mut ScreenBuffer, state: &mut ButtonState) {
		let area = area.intersection(buf.area());
		if area.is_empty() {
			return;
		}
		let style = if !state.enabled {
			self.style.patch(self.disabled_style)
		} else if state.pressed {
			self.style.patch(self.pressed_style)
		} else if state.node.focused {
			self.style.patch(self.focus_style)
		} else {
			self.style
		};
		buf.set_style(area, style);
		let row = Rect::new(area.x, area.y + area.height / 2, area.width, 1);
		let styled = StyledLine {
			chars: self.label.styled_chars(style).collect(),
			alignment: self.label.alignment.unwrap_or(Alignment::Center),
		};
		buf.set_line(row, &styled, 0);
	}
}
