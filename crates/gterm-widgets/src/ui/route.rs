//! Маршрутизация ввода (ADR-0026): при модальном слое всё идёт ему; иначе клавиша — контролу
//! под фокусом, `Tab`/`Shift+Tab` берёт дерево; мышь — контролу под захватом, иначе верхнему под
//! ячейкой; левое нажатие по фокусируемому контролу переводит фокус до доставки; контрол,
//! принявший нажатие, держит захват до отпускания или ухода указателя. Непринятое возвращается
//! приложению без сообщений от дерева. Карта сочетаний (ADR-0029): каждое событие клавиши
//! кормит трекер, непринятое нажатие сопоставляется с картой слоя либо экрана. Команды правки
//! (ADR-0030) распознаются по физической клавише и идут контролу под фокусом раньше клавиши.

use std::time::Instant;

use gterm::app::{Ctx, PointerEvent};
use gterm::hotkey::Key;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::NamedKey;

use super::Ui;
use crate::clipboard::{Clipboard, EditCommand, NoClipboard};
use crate::geometry::Rect;
use crate::input::{Handled, KeyMsg, Messages, MouseKind, MouseMsg, Routed};

impl<'a, M: 'a> Ui<'a, M> {
	/// Клавиша из `AppHandler::on_key`: модификаторы — из `Ctx`.
	pub fn key(&mut self, event: &KeyEvent, ctx: &Ctx<'_>) -> Routed<M> {
		self.route_key_event(
			Key::from_event(event),
			event.state,
			event.repeat,
			&KeyMsg::from_event(event, ctx.modifiers),
		)
	}

	/// Событие клавиши по частям (точка входа тестов без окна: `KeyEvent` не сконструировать):
	/// физическая клавиша, состояние, признак автоповтора и сообщение для контролов. Трекер
	/// подключённой карты получает каждое событие; контролам идут только нажатия (`on_key`
	/// отдаёт и отпускания, ADR-0028); нажатие, не принятое деревом и учтённое трекером,
	/// сопоставляется с картой сочетаний (ADR-0029). Нажатие — команда правки (ADR-0030) сначала
	/// предлагается контролу под фокусом как команда; принятое дальше не идёт.
	pub fn route_key_event(
		&mut self,
		key: Option<Key>,
		state: ElementState,
		repeat: bool,
		msg: &KeyMsg<'_>,
	) -> Routed<M> {
		let is_new_press = self.feed_tracker(key, state, repeat);
		if state == ElementState::Released {
			return Routed::ignored();
		}
		if let Some(command) = key.and_then(|key| EditCommand::from_key(key, msg.modifiers)) {
			let routed = self.route_edit(command);
			if routed.handled.is_yes() {
				return routed;
			}
		}
		let mut routed = self.route_key(msg);
		if routed.handled == Handled::No && is_new_press {
			if let Some(message) = self.hotkey_message() {
				routed.messages.push(message);
				routed.handled = Handled::Yes;
			}
		}
		routed
	}

	/// Команда правки контролу под фокусом с буфером приложения; без подключённого буфера —
	/// с пустым: поле ввода команду всё равно поглощает.
	fn route_edit(&mut self, command: EditCommand) -> Routed<M> {
		match self.clipboard.take() {
			Some(clipboard) => {
				let routed = self.edit_focused(command, &mut *clipboard);
				self.clipboard = Some(clipboard);
				routed
			}
			None => self.edit_focused(command, &mut NoClipboard),
		}
	}

	/// Команда правки фокусу модального слоя, если он есть, иначе фокусу экрана.
	pub(super) fn edit_focused(
		&mut self,
		command: EditCommand,
		clipboard: &mut dyn Clipboard,
	) -> Routed<M> {
		if let Some(modal) = &mut self.modal {
			return modal.route_edit(command, clipboard);
		}
		self.ensure_focus();
		let Some(target) = self.focused() else {
			return Routed::ignored();
		};
		let mut out = Messages::new();
		let handled = self.element_mut(target).edit(command, clipboard, &mut out);
		Routed::new(handled, out)
	}

	/// Скормить событие трекеру карты; ответ — учёл ли он его как новое нажатие (не автоповтор
	/// и не повторное нажатие удержанной клавиши): только такое нажатие вызывает привязку.
	fn feed_tracker(&mut self, key: Option<Key>, state: ElementState, repeat: bool) -> bool {
		let (Some(hotkeys), Some(key)) = (&mut self.hotkeys, key) else {
			return false;
		};
		let is_new_press = state == ElementState::Pressed
			&& !repeat
			&& !hotkeys.tracker.current().keys().contains(&key);
		hotkeys.tracker.feed_key(key, state, repeat);
		is_new_press
	}

	/// Сообщение привязки для только что учтённого нажатия: при модальном слое — из его карты
	/// (нет карты — ничего), иначе из карты экрана.
	fn hotkey_message(&self) -> Option<M> {
		let hotkeys = self.hotkeys.as_ref()?;
		let map = match &self.modal {
			Some(modal) => modal.hotkey_map()?,
			None => hotkeys.map,
		};
		map.message_for(&hotkeys.tracker.full(), &hotkeys.tracker.current())
	}

	/// Клавиша как сообщение (точка входа тестов без окна). Клавиша, не принятая контролом под
	/// фокусом, но несущая набираемый текст (`KeyMsg::typed_text` — обычный набор, который
	/// `winit` отдаёт в `KeyEvent.text`, а не через IME), доставляется ему как текст (ADR-0027).
	pub fn route_key(&mut self, msg: &KeyMsg<'_>) -> Routed<M> {
		if let Some(modal) = &mut self.modal {
			return modal.route_key(msg);
		}
		self.ensure_focus();
		let mut out = Messages::new();
		if msg.is_named(NamedKey::Tab) {
			if msg.modifiers.shift_key() {
				self.focus_prev();
			} else {
				self.focus_next();
			}
			return Routed::new(Handled::Yes, out);
		}
		let Some(target) = self.focused() else {
			return Routed::ignored();
		};
		let mut handled = self.element_mut(target).key(msg, &mut out);
		if handled == Handled::No {
			if let Some(text) = msg.typed_text() {
				handled = self.element_mut(target).text(text, &mut out);
			}
		}
		Routed::new(handled, out)
	}

	/// Текст из `AppHandler::on_text` (ADR-0027): контролу под фокусом слоя или экрана; контрол
	/// без ввода текста отвечает «не обработано», и текст возвращается приложению.
	pub fn text(&mut self, text: &str) -> Routed<M> {
		if let Some(modal) = &mut self.modal {
			return modal.route_text(text);
		}
		self.ensure_focus();
		let Some(target) = self.focused() else {
			return Routed::ignored();
		};
		let mut out = Messages::new();
		let handled = self.element_mut(target).text(text, &mut out);
		Routed::new(handled, out)
	}

	/// Мышь из `AppHandler::on_pointer`: ячейка — из события, модификаторы — из `Ctx`; сетка
	/// экрана нужна для области модального слоя.
	pub fn pointer(&mut self, event: &PointerEvent<'_>, ctx: &Ctx<'_>) -> Routed<M> {
		self.route_mouse(
			&MouseMsg::from_pointer(event, ctx),
			Rect::of_screen(ctx.screen),
		)
	}

	/// Мышь как сообщение в ячейках дерева (точка входа тестов без окна); `screen` — область
	/// всей сетки, по ней центрируется модальный слой.
	pub fn route_mouse(&mut self, msg: &MouseMsg, screen: Rect) -> Routed<M> {
		if let Some(modal) = &mut self.modal {
			return modal.route_mouse(msg, screen);
		}
		self.ensure_focus();
		let mut out = Messages::new();
		if let Some(owner) = self.captured() {
			let handled = self.element_mut(owner).mouse(msg, &mut out);
			if matches!(msg.kind, MouseKind::Up | MouseKind::Cancel) {
				self.set_captured(owner, false);
			}
			return Routed::new(handled, out);
		}
		let Some(target) = self.hit(msg.col, msg.row) else {
			return Routed::ignored();
		};
		if msg.is_left_down() && self.element_mut(target).focusable() {
			self.focus_at(target);
		}
		let handled = self.element_mut(target).mouse(msg, &mut out);
		if msg.kind == MouseKind::Down && handled.is_yes() {
			self.set_captured(target, true);
		}
		Routed::new(handled, out)
	}

	/// Такт времени (поправка ADR-0032): из `AppHandler::on_frame` с `Instant::now()`. Идёт
	/// контролу, держащему захват мыши, — на экране или в модальном слое; без захвата — «не
	/// обработано». Ответ `Yes` значит «контролу нужно время»: приложение перерисовывает экран и
	/// запрашивает следующий кадр (`ctx.redraw()`), так удержанная на стрелке полосы прокрутки
	/// кнопка листает повторно. Сам кадр после нажатия приложение и так запрашивает.
	pub fn tick(&mut self, now: Instant) -> Routed<M> {
		if let Some(modal) = &mut self.modal {
			return modal.route_tick(now);
		}
		let Some(owner) = self.captured() else {
			return Routed::ignored();
		};
		let mut out = Messages::new();
		let handled = self.element_mut(owner).tick(now, &mut out);
		Routed::new(handled, out)
	}

	/// Держит ли какой-то контрол описания захват указателя.
	pub(super) fn has_capture(&self) -> bool {
		self.captured().is_some()
	}

	/// Контрол под захватом, если есть.
	fn captured(&self) -> Option<usize> {
		self.elements()
			.find(|(_, element)| element.node().is_some_and(|node| node.captured))
			.map(|(index, _)| index)
	}

	fn set_captured(&mut self, index: usize, captured: bool) {
		if let Some(state) = self.element_state_mut(index) {
			state.captured = captured;
		}
	}

	/// Верхний контрол под ячейкой — последний в порядке обхода, чья область её содержит.
	/// Листы без состояния не адресуются.
	fn hit(&self, col: i32, row: i32) -> Option<usize> {
		let (Ok(col), Ok(row)) = (u16::try_from(col), u16::try_from(row)) else {
			return None;
		};
		self.elements()
			.filter(|(_, element)| {
				element
					.node()
					.is_some_and(|node| node.area.contains(col, row))
			})
			.map(|(index, _)| index)
			.last()
	}

	fn focus_at(&mut self, index: usize) {
		for other in 0..self.nodes.len() {
			if let Some(state) = self.element_state_mut(other) {
				state.focused = other == index;
			}
		}
	}
}
