use super::cell::{Cell, CellFlags, Color};

/// Цвет символа пустых ячеек по умолчанию.
pub const DEFAULT_FG: Color = Color::WHITE;
/// Цвет фона пустых ячеек по умолчанию.
pub const DEFAULT_BG: Color = Color::BLACK;
/// Фон буфера по умолчанию: подложка под ячейками и поле вокруг сетки (FR-2.1, FR-14).
pub const DEFAULT_BACKGROUND: Color = Color::BLACK;

/// Буфер консольной сетки с учётом изменений (dirty rows).
pub struct ScreenBuffer {
	cols: usize,
	rows: usize,
	cells: Vec<Cell>,
	dirty: Vec<bool>,
	/// Ячейка, которой заполняются пустые места: очистка, прокрутка, `resize`.
	/// Несёт цвета по умолчанию, чтобы фон на пустом месте был виден.
	blank: Cell,
	/// Фон буфера: непрозрачная подложка под ячейками и поле вокруг сетки (FR-2.1, ADR-0013).
	/// Альфа не учитывается. Смену замечает рендерер и рисует кадр целиком.
	background: Color,
}

impl ScreenBuffer {
	/// Создать пустой буфер `cols × rows` с цветами `DEFAULT_FG` / `DEFAULT_BG`.
	pub fn new(cols: usize, rows: usize) -> Self {
		Self::with_default_colors(cols, rows, DEFAULT_FG, DEFAULT_BG)
	}

	/// Создать пустой буфер с заданными цветами пустых ячеек.
	pub fn with_default_colors(cols: usize, rows: usize, fg: Color, bg: Color) -> Self {
		let blank = Cell::new('\0', fg, bg, CellFlags::empty());
		Self {
			cols,
			rows,
			cells: vec![blank; cols * rows],
			dirty: vec![false; rows],
			blank,
			background: DEFAULT_BACKGROUND,
		}
	}

	/// Прозрачный буфер `cols × rows`: пустые места не закрывают то, что под ними. Основа
	/// клиентского буфера наложения (ADR-0014); буфер из [`ScreenBuffer::new`] непрозрачен.
	pub fn transparent(cols: usize, rows: usize) -> Self {
		Self::with_default_colors(cols, rows, Color::TRANSPARENT, Color::TRANSPARENT)
	}

	/// Фон буфера (FR-2.1).
	pub fn background(&self) -> Color {
		self.background
	}

	/// Сменить фон буфера; следующий кадр рисуется целиком.
	pub fn set_background(&mut self, color: Color) {
		self.background = color;
	}

	/// Цвета пустых ячеек `(fg, bg)`.
	pub fn default_colors(&self) -> (Color, Color) {
		(self.blank.fg, self.blank.bg)
	}

	/// Количество столбцов.
	pub fn cols(&self) -> usize {
		self.cols
	}

	/// Количество строк.
	pub fn rows(&self) -> usize {
		self.rows
	}

	/// Получить ячейку по (col, row).
	pub fn cell(&self, col: usize, row: usize) -> Cell {
		assert!(col < self.cols && row < self.rows);
		self.cells[row * self.cols + col]
	}

	/// Установить ячейку с отметкой dirty-строки.
	pub fn set_cell(
		&mut self,
		col: usize,
		row: usize,
		ch: char,
		fg: Color,
		bg: Color,
		flags: CellFlags,
	) {
		let idx = row * self.cols + col;
		self.cells[idx] = Cell::new(ch, fg, bg, flags);
		self.dirty[row] = true;
	}

	/// Записать строку текста в строку `row`.
	pub fn write_row(&mut self, row: usize, text: &str, fg: Color, bg: Color) {
		assert!(row < self.rows);
		let start = row * self.cols;
		for (i, c) in text.chars().take(self.cols).enumerate() {
			self.cells[start + i] = Cell::new(c, fg, bg, CellFlags::empty());
		}
		self.dirty[row] = true;
	}

	/// Заполнить строку цветом символа и фона.
	pub fn fill_row(&mut self, row: usize, fg: Color, bg: Color) {
		assert!(row < self.rows);
		let start = row * self.cols;
		for i in 0..self.cols {
			self.cells[start + i] = Cell::new('\0', fg, bg, CellFlags::empty());
		}
		self.dirty[row] = true;
	}

	#[allow(clippy::too_many_arguments)]
	/// Заполнить прямоугольную область.
	pub fn fill_rect(
		&mut self,
		col: usize,
		row: usize,
		w: usize,
		h: usize,
		ch: char,
		fg: Color,
		bg: Color,
		flags: CellFlags,
	) {
		for dy in 0..h {
			let r = row + dy;
			if r >= self.rows {
				break;
			}
			let start = r * self.cols + col;
			for dx in 0..w {
				let idx = start + dx;
				if col + dx >= self.cols {
					break;
				}
				self.cells[idx] = Cell::new(ch, fg, bg, flags);
			}
			self.dirty[r] = true;
		}
	}

	/// Очистить прямоугольную область.
	pub fn clear_region(&mut self, col: usize, row: usize, w: usize, h: usize) {
		let (fg, bg) = self.default_colors();
		self.fill_rect(col, row, w, h, '\0', fg, bg, CellFlags::empty());
	}

	/// Прокрутить вверх на N строк (верхние удаляются, нижние сдвигаются, новые — пустые).
	pub fn scroll_up(&mut self, n: usize) {
		if n == 0 || n >= self.rows {
			self.clear_all();
			return;
		}
		let cols = self.cols;
		let rows = self.rows;
		let new_cells = &mut self.cells;
		for r in 0..(rows - n) {
			let src = (r + n) * cols;
			let dst = r * cols;
			for c in 0..cols {
				new_cells[dst + c] = new_cells[src + c];
			}
		}
		new_cells[(rows - n) * cols..rows * cols].fill(self.blank);
		for r in (rows - n)..rows {
			self.dirty[r] = true;
		}
	}

	/// Прокрутить вниз на N строк.
	pub fn scroll_down(&mut self, n: usize) {
		if n == 0 || n >= self.rows {
			self.clear_all();
			return;
		}
		let cols = self.cols;
		let rows = self.rows;
		let new_cells = &mut self.cells;
		// Process bottom-to-top to avoid overwriting data we still need to read
		for r in (n..rows).rev() {
			let src = (r - n) * cols;
			let dst = r * cols;
			for c in 0..cols {
				new_cells[dst + c] = new_cells[src + c];
			}
		}
		new_cells[..n * cols].fill(self.blank);
		for r in 0..n {
			self.dirty[r] = true;
		}
	}

	/// Копировать область внутри буфера с перемещением.
	pub fn copy_region(
		&mut self,
		src_col: usize,
		src_row: usize,
		w: usize,
		h: usize,
		dst_col: usize,
		dst_row: usize,
	) {
		let cols = self.cols;
		let rows = self.rows;

		// Защита от выхода за границы
		let src_col = src_col.min(cols.saturating_sub(w));
		let dst_col = dst_col.min(cols.saturating_sub(w));
		let src_row = src_row.min(rows.saturating_sub(h));
		let dst_row = dst_row.min(rows.saturating_sub(h));

		// Копирование с конца, чтобы избежать перекрытия
		for dy in (0..h).rev() {
			let src_y = src_row + dy;
			let dst_y = dst_row + dy;
			let src_start = src_y * cols + src_col;
			let dst_start = dst_y * cols + dst_col;
			for dx in 0..w {
				let src_idx = src_start + dx;
				let dst_idx = dst_start + dx;
				self.cells[dst_idx] = self.cells[src_idx];
			}
		}

		// Помечаем affected rows как dirty
		for r in dst_row..(dst_row + h).min(rows) {
			self.dirty[r] = true;
		}
	}

	/// Изменить размер сетки, сохраняя содержимое в пересекающейся области (верхний левый угол).
	pub fn resize(&mut self, new_cols: usize, new_rows: usize) {
		let old_cols = self.cols;
		let old_rows = self.rows;

		self.cols = new_cols;
		self.rows = new_rows;

		let mut new_cells = vec![self.blank; new_cols * new_rows];
		let min_cols = new_cols.min(old_cols);
		let min_rows = new_rows.min(old_rows);
		for r in 0..min_rows {
			for c in 0..min_cols {
				let old_idx = r * old_cols + c;
				let new_idx = r * new_cols + c;
				new_cells[new_idx] = self.cells[old_idx];
			}
		}
		self.cells = new_cells;

		// resize: все строки dirty
		self.dirty = vec![true; new_rows];
	}

	/// Очистить весь буфер.
	pub fn clear_all(&mut self) {
		self.cells.fill(self.blank);
		self.dirty.fill(true);
	}

	/// Индексы строк, которые изменились с прошлого кадра.
	pub fn dirty_rows(&self) -> &[bool] {
		&self.dirty
	}

	/// Сбросить все dirty-флаги.
	pub fn clear_dirty_rows(&mut self) {
		self.dirty.fill(false);
	}

	/// Проверить, изменилась ли строка с прошлого кадра.
	pub fn is_row_dirty(&self, row: usize) -> bool {
		assert!(row < self.rows);
		self.dirty[row]
	}

	/// Сбросить dirty-флаг указанной строки.
	pub fn clear_row_dirty(&mut self, row: usize) {
		assert!(row < self.rows);
		self.dirty[row] = false;
	}
}
