//! Модальный слой (ADR-0026): второе описание поверх экрана в том же `ui()`. Пока слой есть,
//! весь ввод идёт ему: клавиши и текст — его фокусу, мышь — его контролам в локальных ячейках;
//! клик вне области поглощается (решение О2) и, если задана привязка `on_click_outside`
//! (ADR-0027), даёт сообщение; `Esc`, не взятый контролом слоя, публикует привязку `on_escape`
//! (решение О5). Основной экран ввода не получает, его фокус не меняется.
//!
//! Рисуется через [`Popup`] (решение О3): область по центру сетки или заданная (`at`, ADR-0027),
//! буфер наложения с фоном стиля, вывод `DrawCtx::draw_buffer` из `AppHandler::on_redraw`.

use std::time::Instant;

use gterm::app::DrawCtx;
use gterm::buffer::ScreenBuffer;
use winit::keyboard::NamedKey;

use super::Ui;
use crate::clipboard::{Clipboard, EditCommand};
use crate::geometry::Rect;
use crate::hotkeys::HotkeyMap;
use crate::input::{Handled, KeyMsg, Messages, MouseKind, MouseMsg, Routed};
use crate::style::Style;
use crate::widgets::Popup;

/// Где лежит слой на сетке.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Placement {
	/// По центру сетки, размер `width × height`.
	Centered { width: u16, height: u16 },
	/// Заданная область (обрезается по сетке).
	At(Rect),
	/// Под опорной областью той же ширины; над ней — если снизу не хватает строк.
	Below { anchor: Rect, height: u16 },
}

/// Модальный слой: положение, стиль фона, привязки `Esc` и клика мимо, содержимое.
pub struct Modal<'a, M> {
	placement: Placement,
	style: Style,
	on_escape: Option<M>,
	on_click_outside: Option<M>,
	/// Клик мимо закрывает слой силами его контролов (`Control::dismiss`).
	dismiss_on_click_outside: bool,
	/// Карта сочетаний слоя (ADR-0029); карта экрана под слоем не действует.
	hotkeys: Option<&'a HotkeyMap<M>>,
	content: Option<Ui<'a, M>>,
}

impl<'a, M: 'a> Modal<'a, M> {
	fn with_placement(placement: Placement) -> Self {
		Self {
			placement,
			style: Style::new(),
			on_escape: None,
			on_click_outside: None,
			dismiss_on_click_outside: false,
			hotkeys: None,
			content: None,
		}
	}

	/// Слой `width × height`, отцентрованный по сетке (обрезается по ней).
	pub fn centered(width: u16, height: u16) -> Self {
		Self::with_placement(Placement::Centered { width, height })
	}

	/// Слой в заданной области сетки (обрезается по ней) — меню, подсказки (ADR-0027).
	pub fn at(area: Rect) -> Self {
		Self::with_placement(Placement::At(area))
	}

	/// Слой под опорной областью (ширина опоры, `height` строк); если снизу не хватает строк —
	/// над ней. Выпадающие списки (ADR-0027).
	pub fn below(anchor: Rect, height: u16) -> Self {
		Self::with_placement(Placement::Below { anchor, height })
	}

	/// Стиль фона слоя: `bg` с альфой — полупрозрачный, непрозрачный — закрывает экран под собой.
	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	/// Сообщение на `Esc`, не взятый контролом слоя; без привязки `Esc` уходит приложению.
	pub fn on_escape(mut self, message: M) -> Self {
		self.on_escape = Some(message);
		self
	}

	/// Сообщение на левое нажатие вне области слоя (ADR-0027); нажатие поглощается в любом
	/// случае, без привязки — молча.
	pub fn on_click_outside(mut self, message: M) -> Self {
		self.on_click_outside = Some(message);
		self
	}

	/// Левое нажатие вне области слоя вызывает `Control::dismiss` у контролов содержимого —
	/// слой, владеющий своим состоянием (выпадающий список), сворачивается сам (ADR-0027).
	pub fn dismiss_on_click_outside(mut self) -> Self {
		self.dismiss_on_click_outside = true;
		self
	}

	/// Карта сочетаний слоя (ADR-0029): действует вместо карты экрана, пока слой открыт;
	/// сопоставляется нажатие, не принятое контролами слоя и привязкой `on_escape`. Трекер —
	/// тот, что подключён к описанию экрана ([`Ui::hotkeys`]).
	pub fn hotkeys(mut self, map: &'a HotkeyMap<M>) -> Self {
		self.hotkeys = Some(map);
		self
	}

	/// Описание содержимого в локальных ячейках слоя (начало в нуле).
	pub fn content(mut self, content: Ui<'a, M>) -> Self {
		self.content = Some(content);
		self
	}

	/// Область слоя на сетке.
	pub fn area(&self, screen: Rect) -> Rect {
		match self.placement {
			Placement::Centered { width, height } => screen.centered(width, height),
			Placement::At(area) => area.intersection(screen),
			Placement::Below { anchor, height } => {
				let fits_below =
					u32::from(anchor.bottom()) + u32::from(height) <= u32::from(screen.bottom());
				let y = if fits_below {
					anchor.bottom()
				} else {
					anchor.y.saturating_sub(height)
				};
				Rect::new(anchor.x, y, anchor.width, height).intersection(screen)
			}
		}
	}

	pub(super) fn hotkey_map(&self) -> Option<&'a HotkeyMap<M>> {
		self.hotkeys
	}

	fn local_area(&self, screen: Rect) -> Rect {
		let area = self.area(screen);
		Rect::new(0, 0, area.width, area.height)
	}

	pub(super) fn route_key(&mut self, msg: &KeyMsg<'_>) -> Routed<M> {
		let mut routed = match &mut self.content {
			Some(content) => content.route_key(msg),
			None => Routed::ignored(),
		};
		if routed.handled == Handled::No && msg.is_named(NamedKey::Escape) {
			if let Some(message) = self.on_escape.take() {
				routed.messages.push(message);
				routed.handled = Handled::Yes;
			}
		}
		routed
	}

	/// Команда правки — контролу под фокусом слоя; буфер — тот, что подключён к экрану.
	pub(super) fn route_edit(
		&mut self,
		command: EditCommand,
		clipboard: &mut dyn Clipboard,
	) -> Routed<M> {
		match &mut self.content {
			Some(content) => content.edit_focused(command, clipboard),
			None => Routed::ignored(),
		}
	}

	/// Такт времени — контролу слоя, держащему захват.
	pub(super) fn route_tick(&mut self, now: Instant) -> Routed<M> {
		match &mut self.content {
			Some(content) => content.tick(now),
			None => Routed::ignored(),
		}
	}

	pub(super) fn route_text(&mut self, text: &str) -> Routed<M> {
		match &mut self.content {
			Some(content) => content.text(text),
			None => Routed::ignored(),
		}
	}

	pub(super) fn route_mouse(&mut self, msg: &MouseMsg, screen: Rect) -> Routed<M> {
		let area = self.area(screen);
		let local = self.local_area(screen);
		let Some(content) = &mut self.content else {
			return Routed::new(Handled::Yes, Messages::new());
		};
		let translated = msg.translated(i32::from(area.x), i32::from(area.y));
		let inside = u16::try_from(translated.col).is_ok_and(|c| c < local.width)
			&& u16::try_from(translated.row).is_ok_and(|r| r < local.height);
		// Захваченный контрол слоя и отмена получают сообщение независимо от положения. Мышь
		// слой поглощает целиком: и вне области, и внутри мимо контролов — экрану под ним ничего
		// не достаётся, приложение видит только сообщения контролов слоя и привязку клика мимо.
		let mut messages = if inside || content.has_capture() || msg.kind == MouseKind::Cancel {
			Messages::from_vec(content.route_mouse(&translated, local).messages)
		} else {
			Messages::new()
		};
		if !inside && msg.is_left_down() {
			if let Some(message) = self.on_click_outside.take() {
				messages.push(message);
			}
			if self.dismiss_on_click_outside {
				content.dismiss_all();
			}
		}
		Routed::new(Handled::Yes, messages)
	}

	/// Буфер наложения слоя с нарисованным содержимым и его область на сетке.
	pub(super) fn render(self, screen: Rect) -> (Rect, ScreenBuffer) {
		let area = self.area(screen);
		let popup = Popup::new(area).style(self.style);
		let mut buffer = popup.buffer();
		if let Some(content) = self.content {
			content.render(popup.local_area(), &mut buffer);
		}
		(area, buffer)
	}
}

impl<'a, M: Clone + 'a> Modal<'a, M> {
	/// Одно сообщение и на `Esc`, и на клик мимо — «закрыть» для выпадающих списков и меню.
	pub fn on_close(self, message: M) -> Self {
		self.on_escape(message.clone()).on_click_outside(message)
	}
}

impl<'a, M: 'a> Ui<'a, M> {
	/// Положить модальный слой поверх описания: весь ввод — ему, рисуется отдельно
	/// ([`Ui::draw_modal`]).
	pub fn modal(mut self, modal: Modal<'a, M>) -> Self {
		self.modal = Some(Box::new(modal));
		self
	}

	/// Есть ли модальный слой.
	pub fn has_modal(&self) -> bool {
		self.modal.is_some()
	}

	/// Нарисовать модальный слой поверх экрана в этом кадре (из `AppHandler::on_redraw`);
	/// без слоя ничего не делает. Потребляет описание.
	pub fn draw_modal(self, ctx: &mut DrawCtx<'_>) {
		let grid = ctx.grid();
		let screen = Rect::new(
			0,
			0,
			grid.cols.min(usize::from(u16::MAX)) as u16,
			grid.rows.min(usize::from(u16::MAX)) as u16,
		);
		if let Some((area, buffer)) = self.modal_buffer(screen) {
			Popup::new(area).draw(ctx, &buffer);
		}
	}

	/// Буфер наложения слоя и его область на сетке `screen` (точка входа тестов без окна).
	pub fn modal_buffer(self, screen: Rect) -> Option<(Rect, ScreenBuffer)> {
		self.modal.map(|modal| modal.render(screen))
	}
}
