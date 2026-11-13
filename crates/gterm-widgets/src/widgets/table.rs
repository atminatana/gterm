//! Таблица: строки из ячеек-текстов, ширины колонок через раскладку по ограничениям,
//! необязательный заголовок, выделение строки с символом слева. Состояние — [`TableState`].
//! Полоса прокрутки (`Table::scrollbar`, ADR-0032) — как у списка: в последней колонке рядом со
//! строками (не с заголовком), с точными значениями своего кадра; мышь на ней и колесо
//! прокручивают строки, не меняя выделения.

use std::time::Instant;

use gterm::buffer::ScreenBuffer;

use crate::buffer::BufferExt;
use crate::control::{Control, NodeState, WidgetEvent, WidgetEvents};
use crate::geometry::Rect;
use crate::input::{Handled, KeyMsg, Messages, MouseKind, MouseMsg};
use crate::layout::{Constraint, Layout};
use crate::style::Style;
use crate::text::{StyledLine, Text};
use crate::widgets::scrollbar::BarState;
use crate::widgets::selection::{self, RowMap};
use crate::widgets::{Block, Scrollbar, ScrollbarState, StatefulWidget, Widget};

/// Строка таблицы: ячейки — тексты, высота — максимум строк в ячейках (не меньше 1).
#[derive(Clone, Debug, Default)]
pub struct Row<'a> {
	cells: Vec<Text<'a>>,
	style: Style,
	height: u16,
}

impl<'a> Row<'a> {
	pub fn new<I>(cells: I) -> Self
	where
		I: IntoIterator,
		I::Item: Into<Text<'a>>,
	{
		let cells: Vec<Text<'a>> = cells.into_iter().map(Into::into).collect();
		let height = cells.iter().map(Text::height).max().unwrap_or(1).max(1) as u16;
		Self {
			cells,
			style: Style::new(),
			height,
		}
	}

	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	/// Явная высота строки (не меньше 1).
	pub fn height(mut self, height: u16) -> Self {
		self.height = height.max(1);
		self
	}
}

/// Выделенная строка и смещение первой видимой; как контрол (ADR-0026) — то же поведение, что
/// у [`ListState`](super::ListState): клавиши навигации, клик по строке, колесо; заголовок не
/// выделяется.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TableState {
	pub offset: usize,
	pub selected: Option<usize>,
	/// Фокус, область и захват — пишет дерево описания.
	pub node: NodeState,
	rows: RowMap,
	/// Полоса прокрутки: разметка последней отрисовки, протяжка, правило подгонки смещения.
	scroll: BarState,
	event: Option<WidgetEvent>,
}

impl TableState {
	pub fn with_selected(selected: Option<usize>) -> Self {
		Self {
			selected,
			..Self::default()
		}
	}

	fn apply(&mut self, next: Option<usize>) {
		if next.is_some() && next != self.selected {
			self.selected = next;
			self.event = next.map(WidgetEvent::Selected);
		}
	}

	pub fn selected(&self) -> Option<usize> {
		self.selected
	}

	pub fn select(&mut self, index: Option<usize>) {
		self.selected = index;
		if index.is_none() {
			self.offset = 0;
		}
	}

	pub fn select_next(&mut self) {
		self.selected = Some(self.selected.map_or(0, |i| i.saturating_add(1)));
	}

	pub fn select_previous(&mut self) {
		self.selected = Some(self.selected.map_or(0, |i| i.saturating_sub(1)));
	}

	/// Прокрутить на `delta` строк (отрицательное — назад), не меняя выделения.
	pub fn scroll_by(&mut self, delta: isize) {
		self.scroll_to(self.offset.saturating_add_signed(delta));
	}

	/// Прокрутить к строке `offset` как к первой видимой, не меняя выделения.
	pub fn scroll_to(&mut self, offset: usize) {
		self.offset = self.scroll.clamp(offset);
	}
}

impl<M> Control<M> for TableState {
	fn node(&self) -> &NodeState {
		&self.node
	}

	fn node_mut(&mut self) -> &mut NodeState {
		&mut self.node
	}

	fn focusable(&self) -> bool {
		true
	}

	fn key(&mut self, msg: &KeyMsg<'_>, _out: &mut Messages<M>) -> Handled {
		match selection::navigate(msg, self.selected, self.rows.len(), self.rows.page()) {
			Some(next) => {
				self.apply(next);
				Handled::Yes
			}
			None => Handled::No,
		}
	}

	fn mouse(&mut self, msg: &MouseMsg, _out: &mut Messages<M>) -> Handled {
		match msg.kind {
			MouseKind::Down if msg.is_left_down() => {
				// Полоса прокрутки раньше строк: нажатие на ней выделение не трогает.
				match self.scroll.press(msg, self.offset) {
					Some(offset) => self.offset = offset,
					None => {
						let hit = self.rows.item_at(self.node.area, msg.col, msg.row);
						self.apply(hit);
					}
				}
				Handled::Yes
			}
			// Протяжка ползунка: нажатие принято, таблица держит захват до отпускания.
			MouseKind::Move if self.node.captured => match self.scroll.drag(msg) {
				Some(offset) => {
					self.offset = offset;
					Handled::Yes
				}
				None => Handled::No,
			},
			MouseKind::Wheel { lines } => {
				// С полосой колесо прокручивает строки, без неё — двигает выделение.
				match self.scroll.wheel(self.offset, lines) {
					Some(offset) => self.offset = offset,
					None => {
						let next = selection::wheel(self.selected, self.rows.len(), lines);
						self.apply(next);
					}
				}
				Handled::Yes
			}
			MouseKind::Up | MouseKind::Cancel => {
				self.scroll.release();
				Handled::Yes
			}
			MouseKind::Down | MouseKind::Move => Handled::No,
		}
	}

	/// Кнопка удержана на стрелке или дорожке полосы прокрутки: листание повторяется.
	fn tick(&mut self, now: Instant, _out: &mut Messages<M>) -> Handled {
		match self.scroll.tick(now, self.offset) {
			Some(offset) => {
				self.offset = offset;
				Handled::Yes
			}
			None => Handled::No,
		}
	}
}

impl WidgetEvents for TableState {
	fn take_event(&mut self) -> Option<WidgetEvent> {
		self.event.take()
	}
}

#[derive(Clone, Debug, Default)]
pub struct Table<'a> {
	rows: Vec<Row<'a>>,
	header: Option<Row<'a>>,
	widths: Vec<Constraint>,
	block: Option<Block<'a>>,
	style: Style,
	highlight_style: Style,
	highlight_symbol: Option<&'a str>,
	column_spacing: u16,
	focus_style: Option<Style>,
	scrollbar: Option<Scrollbar>,
}

impl<'a> Table<'a> {
	/// Строки и ограничения ширин колонок (по одному на колонку).
	pub fn new<R, W>(rows: R, widths: W) -> Self
	where
		R: IntoIterator<Item = Row<'a>>,
		W: IntoIterator<Item = Constraint>,
	{
		Self {
			rows: rows.into_iter().collect(),
			widths: widths.into_iter().collect(),
			column_spacing: 1,
			..Self::default()
		}
	}

	pub fn header(mut self, header: Row<'a>) -> Self {
		self.header = Some(header);
		self
	}

	pub fn block(mut self, block: Block<'a>) -> Self {
		self.block = Some(block);
		self
	}

	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	pub fn row_highlight_style(mut self, style: Style) -> Self {
		self.highlight_style = style;
		self
	}

	pub fn highlight_symbol(mut self, symbol: &'a str) -> Self {
		self.highlight_symbol = Some(symbol);
		self
	}

	pub fn column_spacing(mut self, spacing: u16) -> Self {
		self.column_spacing = spacing;
		self
	}

	/// Стиль, накладываемый на рамку блока, пока таблица под фокусом (ADR-0026).
	pub fn focus_style(mut self, style: Style) -> Self {
		self.focus_style = Some(style);
		self
	}

	/// Полоса прокрутки в последней колонке таблицы рядом со строками (ADR-0032): значения
	/// подставляет сама таблица, мышь на полосе обрабатывает её состояние. Ориентация —
	/// вертикальная, заданное в полосе состояние не используется.
	pub fn scrollbar(mut self, scrollbar: Scrollbar) -> Self {
		self.scrollbar = Some(scrollbar);
		self
	}

	pub fn len(&self) -> usize {
		self.rows.len()
	}

	pub fn is_empty(&self) -> bool {
		self.rows.is_empty()
	}

	/// Колонки в области строк: раскладка по ограничениям с зазором.
	fn columns(&self, area: Rect) -> Vec<Rect> {
		Layout::horizontal(self.widths.iter().copied())
			.spacing(self.column_spacing)
			.split(area)
	}

	/// Наибольшее смещение: первая строка последней страницы — все строки от неё до конца
	/// помещаются в `height`.
	fn last_offset(&self, height: usize) -> usize {
		let mut used = 0;
		let mut first = self.rows.len();
		while first > 0 {
			used += usize::from(self.rows[first - 1].height);
			if used > height {
				break;
			}
			first -= 1;
		}
		first.min(self.rows.len().saturating_sub(1))
	}

	/// Сколько строк, начиная с `first`, помещается в `height` целиком (не меньше одной).
	fn visible_count(&self, first: usize, height: usize) -> usize {
		let mut used = 0;
		let mut count = 0;
		for row in &self.rows[first..] {
			used += usize::from(row.height);
			if used > height {
				break;
			}
			count += 1;
		}
		count.max(1)
	}

	/// Первая видимая строка. При `is_fitting` смещение сдвигается, чтобы выделенная строка
	/// была видна; иначе остаётся как есть, но не дальше последней страницы.
	fn first_visible(&self, state: &TableState, height: usize, is_fitting: bool) -> usize {
		if self.rows.is_empty() || height == 0 {
			return 0;
		}
		let last_index = self.rows.len() - 1;
		let mut first = state.offset.min(last_index);
		if !is_fitting {
			return first.min(self.last_offset(height));
		}
		let Some(selected) = state.selected.map(|s| s.min(last_index)) else {
			return first;
		};
		if selected < first {
			return selected;
		}
		loop {
			let used: usize = self.rows[first..=selected]
				.iter()
				.map(|r| usize::from(r.height))
				.sum();
			if used <= height || first == selected {
				return first;
			}
			first += 1;
		}
	}

	fn render_row(
		row: &Row<'a>,
		area: Rect,
		columns: &[Rect],
		style: Style,
		buf: &mut ScreenBuffer,
	) {
		buf.set_style(area, style);
		for (cell, column) in row.cells.iter().zip(columns) {
			let column = Rect::new(column.x, area.y, column.width, area.height);
			for (i, line) in cell.lines.iter().take(usize::from(area.height)).enumerate() {
				let line_area = Rect::new(column.x, column.y + i as u16, column.width, 1);
				let styled = StyledLine {
					chars: line.styled_chars(style.patch(cell.style)).collect(),
					alignment: line.alignment.or(cell.alignment).unwrap_or_default(),
				};
				buf.set_line(line_area, &styled, 0);
			}
		}
	}
}

impl StatefulWidget for Table<'_> {
	type State = TableState;

	fn render(self, area: Rect, buf: &mut ScreenBuffer, state: &mut TableState) {
		let area = area.intersection(buf.area());
		if area.is_empty() {
			return;
		}
		buf.set_style(area, self.style);
		state.rows.reset(area.height, self.rows.len());
		state.scroll.begin_frame();
		let inner = match &self.block {
			Some(block) => {
				let inner = block.inner(area);
				let mut block = block.clone();
				if let (true, Some(style)) = (state.node.focused, self.focus_style) {
					block.patch_border_style(style);
				}
				block.render(area, buf);
				inner
			}
			None => area,
		};
		if inner.is_empty() {
			return;
		}
		// Полосе — последняя колонка; заголовку и строкам остаётся ширина без неё.
		let bar_column = match &self.scrollbar {
			Some(_) if inner.width >= 2 => Some(inner.right() - 1),
			_ => None,
		};
		let inner = match bar_column {
			Some(_) => Rect::new(inner.x, inner.y, inner.width - 1, inner.height),
			None => inner,
		};
		let symbol = self.highlight_symbol.unwrap_or("");
		let symbol_width = symbol.chars().count().min(usize::from(inner.width)) as u16;
		let columns_area = Rect::new(
			inner.x + symbol_width,
			inner.y,
			inner.width - symbol_width,
			inner.height,
		);
		let columns = self.columns(columns_area);
		let mut y = inner.y;
		if let Some(header) = &self.header {
			let height = header.height.min(inner.bottom() - y);
			let header_area = Rect::new(inner.x, y, inner.width, height);
			Self::render_row(
				header,
				header_area,
				&columns,
				self.style.patch(header.style),
				buf,
			);
			y += height;
		}
		if self.rows.is_empty() || y >= inner.bottom() {
			return;
		}
		if let Some(selected) = state.selected {
			state.selected = Some(selected.min(self.rows.len() - 1));
		}
		// Под выделение смещение подгоняется, только когда изменилось то, от чего оно зависит;
		// иначе его оставляют там, куда прокрутила полоса.
		let rows_height = inner.bottom() - y;
		let is_fitting = state
			.scroll
			.take_fit(state.selected, rows_height, self.rows.len());
		let first = self.first_visible(state, usize::from(rows_height), is_fitting);
		state.offset = first;
		if let (Some(scrollbar), Some(column)) = (&self.scrollbar, bar_column) {
			let bar_area = Rect::new(column, y, 1, rows_height);
			let scroll = ScrollbarState::new(self.rows.len())
				.position(first)
				.viewport_length(self.visible_count(first, usize::from(rows_height)));
			state
				.scroll
				.set_layout(scrollbar.draw(bar_area, buf, &scroll));
		}
		for (index, row) in self.rows.iter().enumerate().skip(first) {
			if y >= inner.bottom() {
				break;
			}
			let is_selected = state.selected == Some(index);
			let mut style = self.style.patch(row.style);
			if is_selected {
				style = style.patch(self.highlight_style);
			}
			let height = row.height.min(inner.bottom() - y);
			let row_area = Rect::new(inner.x, y, inner.width, height);
			for line in y..y + height {
				state.rows.mark(line - area.y, index);
			}
			Self::render_row(row, row_area, &columns, style, buf);
			if is_selected && symbol_width > 0 {
				buf.set_string(inner.x, y, symbol, style, row_area);
			}
			y += height;
		}
	}
}

impl Widget for Table<'_> {
	fn render(self, area: Rect, buf: &mut ScreenBuffer) {
		let mut state = TableState::default();
		StatefulWidget::render(self, area, buf, &mut state);
	}
}
