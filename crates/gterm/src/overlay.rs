//! Наложение клиентских буферов поверх экрана (ADR-0014, дополнение к ТЗ
//! `docs/spec/2026-10-09-overlay-buffers.md`).
//!
//! Буферы принадлежат приложению и одолжены только на время `on_redraw`, а рисует библиотека
//! после возврата из него — поэтому [`Overlay::push`] копирует ячейки. Массивы переиспользуются
//! между кадрами: после первого кадра того же размера выделений памяти нет (NFR-6).
//! Записи сравнимы: по равенству с прошлым кадром Software не пересобирает текстуру (ADR-0015).

use crate::buffer::ScreenBuffer;
use crate::cell::Cell;

/// Буферы одного кадра в порядке наложения: позже записанный лежит выше.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Overlay {
	cells: Vec<Cell>,
	entries: Vec<Entry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
	col: i32,
	row: i32,
	cols: usize,
	rows: usize,
	/// Начало ячеек записи в `Overlay::cells`.
	start: usize,
}

/// Один записанный буфер: позиция левого верхнего угла в ячейках основной сетки и ячейки.
#[derive(Clone, Copy, Debug)]
pub struct Layer<'a> {
	/// Колонка основной сетки, на которую ложится колонка 0 буфера; может быть отрицательной.
	pub col: i32,
	/// Строка основной сетки, на которую ложится строка 0 буфера; может быть отрицательной.
	pub row: i32,
	pub cols: usize,
	pub rows: usize,
	cells: &'a [Cell],
}

impl Layer<'_> {
	/// Ячейка буфера `(col, row)` в его собственных координатах.
	pub fn cell(&self, col: usize, row: usize) -> Cell {
		self.cells[row * self.cols + col]
	}

	/// Строки основной сетки `rows`, которые запись задевает, — полуинтервал с обрезкой по сетке.
	pub fn grid_rows(&self, grid_rows: usize) -> std::ops::Range<usize> {
		clip(self.row, self.rows, grid_rows)
	}

	/// Колонки основной сетки, которые запись задевает, — с обрезкой по сетке.
	pub fn grid_cols(&self, grid_cols: usize) -> std::ops::Range<usize> {
		clip(self.col, self.cols, grid_cols)
	}
}

/// Полуинтервал `[start, start + len)` с обрезкой по `[0, limit)`; пустой, если не пересекаются.
fn clip(start: i32, len: usize, limit: usize) -> std::ops::Range<usize> {
	let begin = i64::from(start).clamp(0, limit as i64) as usize;
	let end = (i64::from(start) + len as i64).clamp(0, limit as i64) as usize;
	begin..end.max(begin)
}

/// `clone_from` переиспользует память приёмника: так Software хранит записи прошлого кадра без
/// выделений (NFR-6).
impl Clone for Overlay {
	fn clone(&self) -> Self {
		Self {
			cells: self.cells.clone(),
			entries: self.entries.clone(),
		}
	}

	fn clone_from(&mut self, source: &Self) {
		self.cells.clone_from(&source.cells);
		self.entries.clone_from(&source.entries);
	}
}

impl Overlay {
	pub fn new() -> Self {
		Self::default()
	}

	/// Записать буфер поверх уже записанных; `(col, row)` — ячейка основной сетки под его
	/// левым верхним углом. Ячейки копируются.
	pub fn push(&mut self, buffer: &ScreenBuffer, col: i32, row: i32) {
		let start = self.cells.len();
		for r in 0..buffer.rows() {
			for c in 0..buffer.cols() {
				self.cells.push(buffer.cell(c, r));
			}
		}
		self.entries.push(Entry {
			col,
			row,
			cols: buffer.cols(),
			rows: buffer.rows(),
			start,
		});
	}

	/// Забыть все записи; память остаётся для следующего кадра.
	pub fn clear(&mut self) {
		self.cells.clear();
		self.entries.clear();
	}

	pub fn is_empty(&self) -> bool {
		self.entries.is_empty()
	}

	/// Записи в порядке наложения.
	pub fn layers(&self) -> impl Iterator<Item = Layer<'_>> {
		self.entries.iter().map(|e| Layer {
			col: e.col,
			row: e.row,
			cols: e.cols,
			rows: e.rows,
			cells: &self.cells[e.start..e.start + e.cols * e.rows],
		})
	}

	/// Ёмкость массива ячеек — проверка, что повторный кадр не выделяет память.
	pub fn capacity(&self) -> usize {
		self.cells.capacity()
	}
}
