//! Общая навигация по выделению для списка и таблицы (ADR-0026): клавиши, колесо, клик по
//! карте видимых строк. Один алгоритм на оба контрола, чтобы поведение не расходилось.

use winit::keyboard::NamedKey;

use crate::geometry::Rect;
use crate::input::KeyMsg;

/// Карта видимых строк области контрола: строка области → индекс пункта (рамка, заголовок и
/// пустые строки — `None`). Заполняется отрисовкой, переиспользуется между кадрами.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RowMap {
	rows: Vec<Option<usize>>,
	/// Число пунктов на последней отрисовке; `0` — ещё не рисовали.
	len: usize,
}

impl RowMap {
	pub(crate) fn reset(&mut self, height: u16, len: usize) {
		self.rows.clear();
		self.rows.resize(usize::from(height), None);
		self.len = len;
	}

	pub(crate) fn mark(&mut self, row_in_area: u16, index: usize) {
		if let Some(slot) = self.rows.get_mut(usize::from(row_in_area)) {
			*slot = Some(index);
		}
	}

	pub(crate) fn len(&self) -> usize {
		self.len
	}

	/// Высота страницы — число строк области на последней отрисовке (не меньше 1).
	pub(crate) fn page(&self) -> usize {
		self.rows.len().max(1)
	}

	/// Пункт под ячейкой `(col, row)` системы координат дерева, если она в области `area`.
	pub(crate) fn item_at(&self, area: Rect, col: i32, row: i32) -> Option<usize> {
		let (Ok(col), Ok(row)) = (u16::try_from(col), u16::try_from(row)) else {
			return None;
		};
		if !area.contains(col, row) {
			return None;
		}
		self.rows.get(usize::from(row - area.y)).copied().flatten()
	}
}

/// Новое выделение по клавише навигации; `None` — клавиша не навигационная. До первой
/// отрисовки (`len == 0`) верхняя граница неизвестна — выделение зажмёт отрисовка.
pub(crate) fn navigate(
	msg: &KeyMsg<'_>,
	selected: Option<usize>,
	len: usize,
	page: usize,
) -> Option<Option<usize>> {
	let last = if len == 0 { usize::MAX } else { len - 1 };
	let current = selected.unwrap_or(0);
	let next = if msg.is_named(NamedKey::ArrowDown) {
		selected.map_or(0, |i| i.saturating_add(1).min(last))
	} else if msg.is_named(NamedKey::ArrowUp) {
		current.saturating_sub(1)
	} else if msg.is_named(NamedKey::Home) {
		0
	} else if msg.is_named(NamedKey::End) {
		last
	} else if msg.is_named(NamedKey::PageDown) {
		selected.map_or(0, |i| i.saturating_add(page).min(last))
	} else if msg.is_named(NamedKey::PageUp) {
		current.saturating_sub(page)
	} else {
		return None;
	};
	Some(Some(next))
}

/// Новое выделение по колесу: положительные строки — вверх (соглашение `winit`), на
/// `round(lines)` пунктов; без выделения прокрутка вниз даёт первый пункт.
pub(crate) fn wheel(selected: Option<usize>, len: usize, lines: f32) -> Option<usize> {
	let last = if len == 0 { usize::MAX } else { len - 1 };
	let step = lines.round() as i64;
	match selected {
		Some(index) => {
			let moved = (index as i64).saturating_sub(step).max(0) as usize;
			Some(moved.min(last))
		}
		None if step < 0 => Some(0),
		None => None,
	}
}
