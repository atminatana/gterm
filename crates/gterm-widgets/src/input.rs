//! Сообщения ввода для контролов (ADR-0026): клавиша с модификаторами, мышь в ячейках,
//! результат обработки и очередь сообщений приложения.
//!
//! Типы `winit` (`Key`, `ModifiersState`, `MouseButton`) не переводятся — как у `gterm`
//! (ADR-0004). Сам `KeyEvent` в сообщении не хранится: его нельзя сконструировать в тестах без
//! окна, а `Key` и `ModifiersState` — можно.

use gterm::app::{Ctx, PointerEvent};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta};
use winit::keyboard::{Key, ModifiersState, NamedKey};

/// Нажатие клавиши: логическая клавиша, текст (если есть) и модификаторы на момент нажатия.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyMsg<'a> {
	pub key: &'a Key,
	pub text: Option<&'a str>,
	pub modifiers: ModifiersState,
}

impl<'a> KeyMsg<'a> {
	/// Из события `winit` и модификаторов из [`Ctx::modifiers`].
	pub fn from_event(event: &'a KeyEvent, modifiers: ModifiersState) -> Self {
		Self {
			key: &event.logical_key,
			text: event.text.as_deref(),
			modifiers,
		}
	}

	/// Клавиша из именованных (`Enter`, `Tab`, стрелки …).
	pub fn is_named(&self, named: NamedKey) -> bool {
		matches!(self.key, Key::Named(n) if *n == named)
	}

	/// Символьная клавиша с данным текстом (`"p"`, `" "`).
	pub fn is_char(&self, ch: &str) -> bool {
		matches!(self.key, Key::Character(c) if &**c == ch)
	}

	/// Текст, который нажатие набирает (ADR-0027): `KeyEvent.text` без `Ctrl`/`Alt`/`Super` и с
	/// хотя бы одним печатным символом. `Enter` (`"\r"`), `Esc` (`"\u{1b}"`) и сочетания с
	/// модификаторами текстом не считаются — иначе поле ввода проглотило бы их.
	pub fn typed_text(&self) -> Option<&'a str> {
		let modifiers = self.modifiers;
		if modifiers.control_key() || modifiers.alt_key() || modifiers.super_key() {
			return None;
		}
		self.text
			.filter(|text| text.chars().any(|ch| !ch.is_control()))
	}
}

/// Вид сообщения мыши.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MouseKind {
	/// Кнопка нажата.
	Down,
	/// Кнопка отпущена.
	Up,
	/// Указатель сдвинулся.
	Move,
	/// Колесо: положительное — вверх (соглашение `winit`), в строках; попиксельная прокрутка
	/// переведена в строки по высоте ячейки.
	Wheel { lines: f32 },
	/// Указатель покинул окно: захват снимается, нажатие считается отменённым.
	Cancel,
}

/// Сообщение мыши в ячейках системы координат дерева: главный экран — сетка, модальный слой —
/// локальные ячейки слоя. Отрицательные значения и значения за краем — «вне».
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MouseMsg {
	pub kind: MouseKind,
	/// Кнопка для `Down`/`Up`; у движения, колеса и отмены — `None`.
	pub button: Option<MouseButton>,
	pub col: i32,
	pub row: i32,
	pub modifiers: ModifiersState,
}

impl MouseMsg {
	/// Из события `gterm`: ячейка — из самого события (ADR-0026), модификаторы и размер ячейки
	/// для попиксельной прокрутки — из `Ctx`. У ухода указателя координат нет — `(-1, -1)`.
	pub fn from_pointer(event: &PointerEvent<'_>, ctx: &Ctx<'_>) -> Self {
		let modifiers = ctx.modifiers;
		let (kind, button, (col, row)) = match *event {
			PointerEvent::Moved { cell, .. } => (MouseKind::Move, None, cell),
			PointerEvent::Input {
				state,
				button,
				cell,
				..
			} => {
				let kind = match state {
					ElementState::Pressed => MouseKind::Down,
					ElementState::Released => MouseKind::Up,
				};
				(kind, Some(button), cell)
			}
			PointerEvent::Wheel { delta, cell } => {
				let lines = match *delta {
					MouseScrollDelta::LineDelta(_, lines) => lines,
					MouseScrollDelta::PixelDelta(px) => {
						(px.y / f64::from(ctx.cell_px.1.max(1))) as f32
					}
				};
				(MouseKind::Wheel { lines }, None, cell)
			}
			PointerEvent::Left => (MouseKind::Cancel, None, (-1, -1)),
		};
		Self {
			kind,
			button,
			col,
			row,
			modifiers,
		}
	}

	/// То же сообщение в системе координат, сдвинутой на `(dx, dy)` ячеек (модальный слой).
	pub fn translated(self, dx: i32, dy: i32) -> Self {
		Self {
			col: self.col.saturating_sub(dx),
			row: self.row.saturating_sub(dy),
			..self
		}
	}

	/// Нажатие левой кнопки.
	pub fn is_left_down(&self) -> bool {
		self.kind == MouseKind::Down && self.button == Some(MouseButton::Left)
	}
}

/// Принял ли контрол сообщение. `No` — сообщение идёт дальше (приложению).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handled {
	Yes,
	No,
}

impl Handled {
	pub fn is_yes(self) -> bool {
		self == Handled::Yes
	}
}

/// Очередь сообщений приложения, собираемых за одну маршрутизацию (ADR-0026): контрол
/// публикует через `push` ноль, одно или несколько; привязки встроенных контролов пишут сюда же.
#[derive(Debug)]
pub struct Messages<M> {
	items: Vec<M>,
}

impl<M> Default for Messages<M> {
	fn default() -> Self {
		Self { items: Vec::new() }
	}
}

impl<M> Messages<M> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn push(&mut self, message: M) {
		self.items.push(message);
	}

	pub fn is_empty(&self) -> bool {
		self.items.is_empty()
	}

	pub fn len(&self) -> usize {
		self.items.len()
	}

	pub fn into_vec(self) -> Vec<M> {
		self.items
	}

	pub fn from_vec(items: Vec<M>) -> Self {
		Self { items }
	}
}

/// Результат маршрутизации: принято ли сообщение ввода и что опубликовали контролы.
/// Адрес контрола не передаётся (решение владельца, ADR-0026).
#[derive(Debug)]
pub struct Routed<M> {
	pub handled: Handled,
	pub messages: Vec<M>,
}

impl<M> Routed<M> {
	pub fn new(handled: Handled, messages: Messages<M>) -> Self {
		Self {
			handled,
			messages: messages.into_vec(),
		}
	}

	/// Ничего не принято, сообщений нет.
	pub fn ignored() -> Self {
		Self {
			handled: Handled::No,
			messages: Vec::new(),
		}
	}
}

impl<M> IntoIterator for Routed<M> {
	type Item = M;
	type IntoIter = std::vec::IntoIter<M>;

	fn into_iter(self) -> Self::IntoIter {
		self.messages.into_iter()
	}
}
