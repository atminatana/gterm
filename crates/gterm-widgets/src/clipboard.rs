//! Буфер обмена и команды правки (ADR-0030). Буфер — трейт [`Clipboard`]: объект живёт в
//! состоянии приложения (описание экрана пересобирается на каждое событие, а владелец
//! системного буфера на X11 обязан жить всё время работы программы) и подключается к описанию
//! через `Ui::clipboard`. Команды правки [`EditCommand`] дерево распознаёт по физической
//! клавише — раскладка не важна — и передаёт контролу под фокусом (`Control::edit`) до обычной
//! доставки клавиши.

use gterm::hotkey::Key;
use winit::keyboard::{KeyCode, ModifiersState};

/// Текстовый буфер обмена.
pub trait Clipboard {
	/// Текст из буфера; `None` — буфер пуст, содержит не текст или недоступен.
	fn get(&mut self) -> Option<String>;

	/// Положить текст в буфер; при недоступном буфере — без действия.
	fn set(&mut self, text: &str);

	/// Примет ли буфер текст. Вырезание при недоступном буфере текст не удаляет — иначе он
	/// пропал бы.
	fn is_available(&mut self) -> bool {
		true
	}
}

/// Буфер в памяти процесса: для тестов и приложений без системного буфера.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryClipboard {
	text: Option<String>,
}

impl MemoryClipboard {
	pub fn new() -> Self {
		Self::default()
	}

	/// Содержимое без изменения буфера.
	pub fn text(&self) -> Option<&str> {
		self.text.as_deref()
	}
}

impl Clipboard for MemoryClipboard {
	fn get(&mut self) -> Option<String> {
		self.text.clone()
	}

	fn set(&mut self, text: &str) {
		self.text = Some(text.to_owned());
	}
}

/// Буфер описания, к которому приложение буфер не подключило: всегда пуст, запись теряется.
/// Поле ввода при этом всё равно поглощает команды правки (ADR-0030).
pub(crate) struct NoClipboard;

impl Clipboard for NoClipboard {
	fn get(&mut self) -> Option<String> {
		None
	}

	fn set(&mut self, _text: &str) {}

	fn is_available(&mut self) -> bool {
		false
	}
}

/// Команда правки для контрола под фокусом.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditCommand {
	/// Выделенное — в буфер.
	Copy,
	/// Выделенное — в буфер и удалить.
	Cut,
	/// Содержимое буфера — в позицию курсора вместо выделенного.
	Paste,
	SelectAll,
}

impl EditCommand {
	/// Команда по физической клавише и модификаторам: `Ctrl+C`/`Ctrl+Insert` — копировать,
	/// `Ctrl+X`/`Shift+Delete` — вырезать, `Ctrl+V`/`Shift+Insert` — вставить, `Ctrl+A` —
	/// выделить всё. Модификаторы — ровно названные: `Ctrl+Shift+C` командой не считается.
	pub fn from_key(key: Key, modifiers: ModifiersState) -> Option<Self> {
		let Key::Code(code) = key else {
			return None;
		};
		if modifiers == ModifiersState::CONTROL {
			match code {
				KeyCode::KeyC | KeyCode::Insert => Some(Self::Copy),
				KeyCode::KeyX => Some(Self::Cut),
				KeyCode::KeyV => Some(Self::Paste),
				KeyCode::KeyA => Some(Self::SelectAll),
				_ => None,
			}
		} else if modifiers == ModifiersState::SHIFT {
			match code {
				KeyCode::Insert => Some(Self::Paste),
				KeyCode::Delete => Some(Self::Cut),
				_ => None,
			}
		} else {
			None
		}
	}
}

/// Системный буфер обмена на `arboard` (только текст). Создаётся при первом обращении; ошибки
/// не паникуют: `get` отдаёт `None`, `set` ничего не делает, причина один раз печатается в
/// stderr.
#[cfg(feature = "system-clipboard")]
#[derive(Default)]
pub struct SystemClipboard {
	inner: Option<arboard::Clipboard>,
	/// Создать буфер не удалось — повторных попыток нет.
	is_unavailable: bool,
	is_failure_reported: bool,
}

#[cfg(feature = "system-clipboard")]
impl SystemClipboard {
	pub fn new() -> Self {
		Self::default()
	}

	fn inner(&mut self) -> Option<&mut arboard::Clipboard> {
		if self.inner.is_none() && !self.is_unavailable {
			match arboard::Clipboard::new() {
				Ok(clipboard) => self.inner = Some(clipboard),
				Err(error) => {
					self.is_unavailable = true;
					self.report(&error);
				}
			}
		}
		self.inner.as_mut()
	}

	fn report(&mut self, error: &arboard::Error) {
		if !self.is_failure_reported {
			eprintln!("gterm-widgets: system clipboard failed: {error}");
			self.is_failure_reported = true;
		}
	}
}

#[cfg(feature = "system-clipboard")]
impl Clipboard for SystemClipboard {
	fn get(&mut self) -> Option<String> {
		let result = self.inner()?.get_text();
		match result {
			Ok(text) => Some(text),
			// Буфер пуст или в нём не текст — обычное состояние, не ошибка.
			Err(arboard::Error::ContentNotAvailable) => None,
			Err(error) => {
				self.report(&error);
				None
			}
		}
	}

	fn set(&mut self, text: &str) {
		let Some(clipboard) = self.inner() else {
			return;
		};
		if let Err(error) = clipboard.set_text(text) {
			self.report(&error);
		}
	}

	fn is_available(&mut self) -> bool {
		self.inner().is_some()
	}
}
