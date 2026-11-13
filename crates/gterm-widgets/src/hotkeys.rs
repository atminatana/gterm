//! Карта «сочетание → сообщение» (ADR-0029): упорядоченный список образцов `gterm::hotkey`
//! с сообщениями приложения. Строится один раз и хранится в состоянии приложения — описание
//! экрана пересобирается на каждое событие, разбирать строки при каждой сборке нельзя.
//! Подключается к описанию через `Ui::hotkeys` (экран) и `Modal::hotkeys` (слой).

use gterm::hotkey::{Hotkey, ParseError};

/// Привязки сочетаний к сообщениям в порядке объявления.
#[derive(Clone, Debug)]
pub struct HotkeyMap<M> {
	bindings: Vec<(Hotkey, M)>,
	/// Копирование сообщения: берётся при создании, чтобы маршрутизация `Ui` не требовала
	/// `M: Clone` от приложений, которые карту не подключают.
	clone_message: fn(&M) -> M,
}

impl<M: Clone> HotkeyMap<M> {
	pub fn new() -> Self {
		Self {
			bindings: Vec::new(),
			clone_message: M::clone,
		}
	}

	/// Добавить привязку; образец может содержать альтернативы (`Ctrl+S|Super+S`).
	pub fn bind(mut self, pattern: Hotkey, message: M) -> Self {
		self.bindings.push((pattern, message));
		self
	}

	/// Добавить привязку по записи образца (`"Ctrl+[K,U]"`); ошибка разбора — с позицией.
	pub fn bind_str(self, pattern: &str, message: M) -> Result<Self, ParseError> {
		Ok(self.bind(pattern.parse()?, message))
	}
}

impl<M: Clone> Default for HotkeyMap<M> {
	fn default() -> Self {
		Self::new()
	}
}

impl<M> HotkeyMap<M> {
	pub fn len(&self) -> usize {
		self.bindings.len()
	}

	pub fn is_empty(&self) -> bool {
		self.bindings.is_empty()
	}

	/// Сообщение для нажатия: сначала полная история комбинации `full` (аккорды —
	/// `Ctrl+[K,U]` в момент нажатия `U`), затем цепочка удержанных `current` (простые
	/// сочетания — `Ctrl+V` после `Ctrl+C` без отпускания `Ctrl`); на каждом шаге — первая
	/// совпавшая привязка в порядке объявления.
	pub fn message_for(&self, full: &Hotkey, current: &Hotkey) -> Option<M> {
		[full, current].into_iter().find_map(|recorded| {
			self.bindings
				.iter()
				.find(|(pattern, _)| recorded.matches(pattern))
				.map(|(_, message)| (self.clone_message)(message))
		})
	}
}
