//! Элементы дерева: свой контрол, встроенный контрол с привязками, лист без состояния. Единый
//! объектный трейт [`Element`] — только внутри временного вектора узлов (ADR-0026).

use std::time::Instant;

use gterm::buffer::ScreenBuffer;

use crate::clipboard::{Clipboard, EditCommand};
use crate::control::{Control, Drawable, NodeState, WidgetEvent, WidgetEvents};
use crate::geometry::Rect;
use crate::input::{Handled, KeyMsg, Messages, MouseMsg};
use crate::theme::Theme;
use crate::widgets::{StatefulWidget, Widget};

/// `'a` — время жизни описания: столько живут ссылки на состояния и замыкания привязок.
pub(super) trait Element<'a, M> {
	/// Состояние узла; `None` у листа без состояния.
	fn node(&self) -> Option<&NodeState>;
	fn node_mut(&mut self) -> Option<&mut NodeState>;
	fn focusable(&self) -> bool;
	fn key(&mut self, msg: &KeyMsg<'_>, out: &mut Messages<M>) -> Handled;
	fn mouse(&mut self, msg: &MouseMsg, out: &mut Messages<M>) -> Handled;
	/// Принимает ли контрол двойной клик в месте нажатия (ADR-0035); у листа без состояния —
	/// нет.
	fn accepts_double_click(&self, msg: &MouseMsg) -> bool {
		let _ = msg;
		false
	}
	fn text(&mut self, text: &str, out: &mut Messages<M>) -> Handled;
	/// Команда правки контролу (ADR-0030); у листа без состояния не действует.
	fn edit(
		&mut self,
		command: EditCommand,
		clipboard: &mut dyn Clipboard,
		out: &mut Messages<M>,
	) -> Handled {
		let _ = (command, clipboard, out);
		Handled::No
	}
	/// Такт времени контролу под захватом; у листа без состояния не действует.
	fn tick(&mut self, now: Instant, out: &mut Messages<M>) -> Handled {
		let _ = (now, out);
		Handled::No
	}
	/// Нарисовать узел; тема описания (ADR-0037) передаётся виджету или контролу перед
	/// отрисовкой — свои области он берёт сам.
	fn render(&mut self, area: Rect, buf: &mut ScreenBuffer, theme: Option<&Theme>);
	/// Слой закрывают кликом мимо; у листов без состояния не действует.
	fn dismiss(&mut self) {}
	/// Привязки встроенных контролов; у остальных элементов не действуют.
	fn bind_select(&mut self, to_message: Box<dyn Fn(usize) -> M + 'a>) {
		let _ = to_message;
	}
	fn bind_activate(&mut self, message: M) {
		let _ = message;
	}
	fn bind_change(&mut self, to_message: Box<dyn Fn(String) -> M + 'a>) {
		let _ = to_message;
	}
	fn bind_submit(&mut self, to_message: Box<dyn Fn(String) -> M + 'a>) {
		let _ = to_message;
	}
	fn bind_toggle(&mut self, to_message: Box<dyn Fn(bool) -> M + 'a>) {
		let _ = to_message;
	}
	fn bind_value(&mut self, to_message: Box<dyn Fn(i64) -> M + 'a>) {
		let _ = to_message;
	}
}

/// Лист без состояния: виджет потребляется отрисовкой.
pub(super) struct Stateless<W> {
	pub(super) widget: Option<W>,
}

impl<M, W: Widget> Element<'_, M> for Stateless<W> {
	fn node(&self) -> Option<&NodeState> {
		None
	}

	fn node_mut(&mut self) -> Option<&mut NodeState> {
		None
	}

	fn focusable(&self) -> bool {
		false
	}

	fn key(&mut self, _msg: &KeyMsg<'_>, _out: &mut Messages<M>) -> Handled {
		Handled::No
	}

	fn mouse(&mut self, _msg: &MouseMsg, _out: &mut Messages<M>) -> Handled {
		Handled::No
	}

	fn text(&mut self, _text: &str, _out: &mut Messages<M>) -> Handled {
		Handled::No
	}

	fn render(&mut self, area: Rect, buf: &mut ScreenBuffer, theme: Option<&Theme>) {
		if let Some(mut widget) = self.widget.take() {
			if let Some(theme) = theme {
				widget.apply_theme(theme);
			}
			widget.render(area, buf);
		}
	}
}

/// Свой контрол приложения.
pub(super) struct Custom<'a, C> {
	pub(super) control: &'a mut C,
}

impl<M, C: Control<M> + Drawable> Element<'_, M> for Custom<'_, C> {
	fn node(&self) -> Option<&NodeState> {
		Some(self.control.node())
	}

	fn node_mut(&mut self) -> Option<&mut NodeState> {
		Some(self.control.node_mut())
	}

	fn focusable(&self) -> bool {
		self.control.focusable()
	}

	fn key(&mut self, msg: &KeyMsg<'_>, out: &mut Messages<M>) -> Handled {
		self.control.key(msg, out)
	}

	fn mouse(&mut self, msg: &MouseMsg, out: &mut Messages<M>) -> Handled {
		self.control.mouse(msg, out)
	}

	fn accepts_double_click(&self, msg: &MouseMsg) -> bool {
		self.control.accepts_double_click(msg)
	}

	fn text(&mut self, text: &str, out: &mut Messages<M>) -> Handled {
		self.control.text(text, out)
	}

	fn edit(
		&mut self,
		command: EditCommand,
		clipboard: &mut dyn Clipboard,
		out: &mut Messages<M>,
	) -> Handled {
		self.control.edit(command, clipboard, out)
	}

	fn tick(&mut self, now: Instant, out: &mut Messages<M>) -> Handled {
		self.control.tick(now, out)
	}

	fn render(&mut self, area: Rect, buf: &mut ScreenBuffer, theme: Option<&Theme>) {
		if let Some(theme) = theme {
			self.control.apply_theme(theme);
		}
		self.control.draw(area, buf);
	}

	fn dismiss(&mut self) {
		self.control.dismiss();
	}
}

/// Временный контрол, которым владеет само описание (содержимое выпадающего списка, ADR-0027):
/// флаги фокуса и области он обязан хранить в состоянии приложения, на которое ссылается, иначе
/// они пропадут вместе с описанием.
pub(super) struct Owned<C> {
	pub(super) control: C,
}

impl<M, C: Control<M> + Drawable> Element<'_, M> for Owned<C> {
	fn node(&self) -> Option<&NodeState> {
		Some(self.control.node())
	}

	fn node_mut(&mut self) -> Option<&mut NodeState> {
		Some(self.control.node_mut())
	}

	fn focusable(&self) -> bool {
		self.control.focusable()
	}

	fn key(&mut self, msg: &KeyMsg<'_>, out: &mut Messages<M>) -> Handled {
		self.control.key(msg, out)
	}

	fn mouse(&mut self, msg: &MouseMsg, out: &mut Messages<M>) -> Handled {
		self.control.mouse(msg, out)
	}

	fn accepts_double_click(&self, msg: &MouseMsg) -> bool {
		self.control.accepts_double_click(msg)
	}

	fn text(&mut self, text: &str, out: &mut Messages<M>) -> Handled {
		self.control.text(text, out)
	}

	fn edit(
		&mut self,
		command: EditCommand,
		clipboard: &mut dyn Clipboard,
		out: &mut Messages<M>,
	) -> Handled {
		self.control.edit(command, clipboard, out)
	}

	fn tick(&mut self, now: Instant, out: &mut Messages<M>) -> Handled {
		self.control.tick(now, out)
	}

	fn render(&mut self, area: Rect, buf: &mut ScreenBuffer, theme: Option<&Theme>) {
		if let Some(theme) = theme {
			self.control.apply_theme(theme);
		}
		self.control.draw(area, buf);
	}

	fn dismiss(&mut self) {
		self.control.dismiss();
	}
}

/// Встроенный контрол: виджет-значение плюс состояние с поведением и привязки сообщений.
pub(super) struct Stateful<'a, M, W, S> {
	widget: Option<W>,
	state: &'a mut S,
	on_select: Option<Box<dyn Fn(usize) -> M + 'a>>,
	on_activate: Option<M>,
	on_change: Option<Box<dyn Fn(String) -> M + 'a>>,
	on_submit: Option<Box<dyn Fn(String) -> M + 'a>>,
	on_toggle: Option<Box<dyn Fn(bool) -> M + 'a>>,
	on_value: Option<Box<dyn Fn(i64) -> M + 'a>>,
}

impl<'a, M, W, S> Stateful<'a, M, W, S> {
	pub(super) fn new(widget: W, state: &'a mut S) -> Self {
		Self {
			widget: Some(widget),
			state,
			on_select: None,
			on_activate: None,
			on_change: None,
			on_submit: None,
			on_toggle: None,
			on_value: None,
		}
	}
}

impl<'a, M, W, S> Stateful<'a, M, W, S>
where
	S: WidgetEvents,
{
	/// Событие состояния после обработки → сообщение по привязке. Активация публикуется один
	/// раз: описание живёт одно событие, сообщение переносится, а не клонируется.
	fn emit(&mut self, out: &mut Messages<M>) {
		match self.state.take_event() {
			Some(WidgetEvent::Selected(index)) => {
				if let Some(to_message) = &self.on_select {
					out.push(to_message(index));
				}
			}
			Some(WidgetEvent::Activated) => {
				if let Some(message) = self.on_activate.take() {
					out.push(message);
				}
			}
			Some(WidgetEvent::Changed(value)) => {
				if let Some(to_message) = &self.on_change {
					out.push(to_message(value));
				}
			}
			Some(WidgetEvent::Submitted(value)) => {
				if let Some(to_message) = &self.on_submit {
					out.push(to_message(value));
				}
			}
			Some(WidgetEvent::Toggled(checked)) => {
				if let Some(to_message) = &self.on_toggle {
					out.push(to_message(checked));
				}
			}
			Some(WidgetEvent::ValueChanged(value)) => {
				if let Some(to_message) = &self.on_value {
					out.push(to_message(value));
				}
			}
			None => {}
		}
	}
}

impl<'a, M, W, S> Element<'a, M> for Stateful<'a, M, W, S>
where
	W: StatefulWidget<State = S>,
	S: Control<M> + WidgetEvents,
{
	fn node(&self) -> Option<&NodeState> {
		Some(self.state.node())
	}

	fn node_mut(&mut self) -> Option<&mut NodeState> {
		Some(self.state.node_mut())
	}

	fn focusable(&self) -> bool {
		self.state.focusable()
	}

	fn key(&mut self, msg: &KeyMsg<'_>, out: &mut Messages<M>) -> Handled {
		let handled = self.state.key(msg, out);
		self.emit(out);
		handled
	}

	fn mouse(&mut self, msg: &MouseMsg, out: &mut Messages<M>) -> Handled {
		let handled = self.state.mouse(msg, out);
		self.emit(out);
		handled
	}

	fn accepts_double_click(&self, msg: &MouseMsg) -> bool {
		self.state.accepts_double_click(msg)
	}

	fn text(&mut self, text: &str, out: &mut Messages<M>) -> Handled {
		let handled = self.state.text(text, out);
		self.emit(out);
		handled
	}

	fn edit(
		&mut self,
		command: EditCommand,
		clipboard: &mut dyn Clipboard,
		out: &mut Messages<M>,
	) -> Handled {
		let handled = self.state.edit(command, clipboard, out);
		self.emit(out);
		handled
	}

	fn tick(&mut self, now: Instant, out: &mut Messages<M>) -> Handled {
		let handled = self.state.tick(now, out);
		self.emit(out);
		handled
	}

	fn render(&mut self, area: Rect, buf: &mut ScreenBuffer, theme: Option<&Theme>) {
		if let Some(mut widget) = self.widget.take() {
			if let Some(theme) = theme {
				widget.apply_theme(theme);
			}
			widget.render(area, buf, self.state);
		}
	}

	fn dismiss(&mut self) {
		self.state.dismiss();
	}

	fn bind_select(&mut self, to_message: Box<dyn Fn(usize) -> M + 'a>) {
		self.on_select = Some(to_message);
	}

	fn bind_activate(&mut self, message: M) {
		self.on_activate = Some(message);
	}

	fn bind_change(&mut self, to_message: Box<dyn Fn(String) -> M + 'a>) {
		self.on_change = Some(to_message);
	}

	fn bind_submit(&mut self, to_message: Box<dyn Fn(String) -> M + 'a>) {
		self.on_submit = Some(to_message);
	}

	fn bind_toggle(&mut self, to_message: Box<dyn Fn(bool) -> M + 'a>) {
		self.on_toggle = Some(to_message);
	}

	fn bind_value(&mut self, to_message: Box<dyn Fn(i64) -> M + 'a>) {
		self.on_value = Some(to_message);
	}
}
