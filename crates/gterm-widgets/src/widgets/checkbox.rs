//! Флажок (ADR-0031): метка и подпись слева в одну строку, стили обычный/фокус/нажат/
//! неактивный — как у кнопки. Переключается пробелом, `Enter` и мышью — отпусканием левой
//! кнопки внутри области после нажатия в ней; отпускание снаружи или уход указателя — отмена.
//! Новое значение дерево превращает в сообщение привязки `on_toggle`.

use gterm::buffer::ScreenBuffer;
use winit::keyboard::NamedKey;

use crate::buffer::BufferExt;
use crate::control::{Control, NodeState, WidgetEvent, WidgetEvents};
use crate::geometry::Rect;
use crate::input::{Handled, KeyMsg, Messages, MouseKind, MouseMsg};
use crate::style::Style;
use crate::text::{Alignment, Line, StyledLine};
use crate::widgets::StatefulWidget;

/// Состояние флажка: отмечен ли, активен ли, нажат ли мышью; фокус и область — от дерева.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckboxState {
	pub node: NodeState,
	/// Неактивный флажок не фокусируется и не реагирует на ввод.
	pub enabled: bool,
	pub checked: bool,
	/// Нажат мышью: от левого нажатия в области до отпускания или отмены.
	pub pressed: bool,
	event: Option<WidgetEvent>,
}

impl Default for CheckboxState {
	fn default() -> Self {
		Self {
			node: NodeState::default(),
			enabled: true,
			checked: false,
			pressed: false,
			event: None,
		}
	}
}

impl CheckboxState {
	pub fn new() -> Self {
		Self::default()
	}

	/// Флажок с начальным значением.
	pub fn with_checked(checked: bool) -> Self {
		Self {
			checked,
			..Self::default()
		}
	}

	pub fn disabled() -> Self {
		Self {
			enabled: false,
			..Self::default()
		}
	}

	pub fn is_checked(&self) -> bool {
		self.checked
	}

	/// Задать значение. Программная смена — дело приложения, события `Toggled` она не даёт:
	/// события отражают ввод пользователя.
	pub fn set_checked(&mut self, checked: bool) {
		self.checked = checked;
	}

	/// Переключить значение программно, без события.
	pub fn toggle(&mut self) {
		self.checked = !self.checked;
	}

	/// Переключение пользователем: новое значение уходит событием.
	fn toggle_by_user(&mut self) {
		self.toggle();
		self.event = Some(WidgetEvent::Toggled(self.checked));
	}

	fn is_inside(&self, msg: &MouseMsg) -> bool {
		let (Ok(col), Ok(row)) = (u16::try_from(msg.col), u16::try_from(msg.row)) else {
			return false;
		};
		self.node.area.contains(col, row)
	}
}

impl<M> Control<M> for CheckboxState {
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
		if msg.is_named(NamedKey::Space) || msg.is_named(NamedKey::Enter) || msg.is_char(" ") {
			self.toggle_by_user();
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
					self.toggle_by_user();
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
}

impl WidgetEvents for CheckboxState {
	fn take_event(&mut self) -> Option<WidgetEvent> {
		self.event.take()
	}
}

/// Флажок: метка состояния, пробел и подпись; стиль по состоянию — неактивный, нажат, под
/// фокусом, обычный.
#[derive(Clone, Debug)]
pub struct Checkbox<'a> {
	label: Line<'a>,
	checked_mark: &'a str,
	unchecked_mark: &'a str,
	style: Style,
	focus_style: Style,
	pressed_style: Style,
	disabled_style: Style,
}

impl<'a> Checkbox<'a> {
	pub fn new<T: Into<Line<'a>>>(label: T) -> Self {
		Self {
			label: label.into(),
			checked_mark: "[x]",
			unchecked_mark: "[ ]",
			style: Style::default(),
			focus_style: Style::default(),
			pressed_style: Style::default(),
			disabled_style: Style::default(),
		}
	}

	/// Свои метки отмеченного и неотмеченного состояния (например, `"☑"` и `"☐"` — если они
	/// есть в шрифте).
	pub fn marks(mut self, checked: &'a str, unchecked: &'a str) -> Self {
		self.checked_mark = checked;
		self.unchecked_mark = unchecked;
		self
	}

	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	/// Накладывается на основной стиль, пока флажок под фокусом.
	pub fn focus_style(mut self, style: Style) -> Self {
		self.focus_style = style;
		self
	}

	/// Накладывается, пока флажок нажат мышью (сильнее стиля фокуса).
	pub fn pressed_style(mut self, style: Style) -> Self {
		self.pressed_style = style;
		self
	}

	/// Накладывается у неактивного флажка вместо стилей фокуса и нажатия.
	pub fn disabled_style(mut self, style: Style) -> Self {
		self.disabled_style = style;
		self
	}
}

impl StatefulWidget for Checkbox<'_> {
	type State = CheckboxState;

	fn render(self, area: Rect, buf: &mut ScreenBuffer, state: &mut CheckboxState) {
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
		let mark = if state.checked {
			self.checked_mark
		} else {
			self.unchecked_mark
		};
		buf.set_string(row.x, row.y, mark, style, row);
		// Подпись — через одну ячейку после метки, в оставшейся ширине.
		let offset = u16::try_from(mark.chars().count() + 1).unwrap_or(u16::MAX);
		if offset >= row.width {
			return;
		}
		let label_area = Rect::new(row.x + offset, row.y, row.width - offset, 1);
		let styled = StyledLine {
			chars: self.label.styled_chars(style).collect(),
			alignment: self.label.alignment.unwrap_or(Alignment::Left),
		};
		buf.set_line(label_area, &styled, 0);
	}
}
