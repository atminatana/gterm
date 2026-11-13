//! Список: пункты из многострочного текста, выделение с символом слева, смещение прокрутки
//! подгоняется так, чтобы выделенный пункт был виден. Состояние — [`ListState`] у приложения.
//! Полоса прокрутки (`List::scrollbar`, ADR-0032) принадлежит списку: он рисует её в последней
//! колонке с точными значениями своего кадра и сам обрабатывает мышь на ней — стрелки листают по
//! пункту, дорожка — по странице, ползунок тянется; колесо при полосе прокручивает содержимое
//! (без полосы — двигает выделение, ADR-0026). Прокрутка полосой выделение не меняет;
//! к выделенному пункту смещение возвращается, когда меняется выделение, высота или число
//! пунктов.

use std::time::Instant;

use gterm::buffer::ScreenBuffer;

use crate::buffer::BufferExt;
use crate::control::{Control, NodeState, WidgetEvent, WidgetEvents};
use crate::geometry::Rect;
use crate::input::{Handled, KeyMsg, Messages, MouseKind, MouseMsg};
use crate::style::Style;
use crate::text::{StyledLine, Text};
use crate::widgets::scrollbar::BarState;
use crate::widgets::selection::{self, RowMap};
use crate::widgets::{Block, Scrollbar, ScrollbarState, StatefulWidget, Widget};

#[derive(Clone, Debug, Default)]
pub struct ListItem<'a> {
	content: Text<'a>,
	style: Style,
}

impl<'a> ListItem<'a> {
	pub fn new<T: Into<Text<'a>>>(content: T) -> Self {
		Self {
			content: content.into(),
			style: Style::new(),
		}
	}

	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	pub fn height(&self) -> usize {
		self.content.height().max(1)
	}
}

impl<'a, T: Into<Text<'a>>> From<T> for ListItem<'a> {
	fn from(content: T) -> Self {
		Self::new(content)
	}
}

/// Выделенный пункт и смещение первого видимого; как контрол (ADR-0026) — фокусируется,
/// стрелки/`Home`/`End`/`PgUp`/`PgDn` и клик меняют выделение, колесо листает его.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListState {
	pub offset: usize,
	pub selected: Option<usize>,
	/// Фокус, область и захват — пишет дерево описания.
	pub node: NodeState,
	/// Видимые строки последней отрисовки: по ним клик находит пункт, `PgUp`/`PgDn` — страницу.
	rows: RowMap,
	/// Полоса прокрутки: разметка последней отрисовки, протяжка, правило подгонки смещения.
	scroll: BarState,
	event: Option<WidgetEvent>,
}

impl ListState {
	pub fn with_selected(selected: Option<usize>) -> Self {
		Self {
			selected,
			..Self::default()
		}
	}

	/// Сменить выделение и запомнить событие, если оно изменилось.
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

	/// Следующий пункт; с последнего — остаётся; без выделения — первый.
	pub fn select_next(&mut self) {
		self.selected = Some(self.selected.map_or(0, |i| i.saturating_add(1)));
	}

	/// Предыдущий пункт; с первого — остаётся; без выделения — первый.
	pub fn select_previous(&mut self) {
		self.selected = Some(self.selected.map_or(0, |i| i.saturating_sub(1)));
	}

	pub fn select_first(&mut self) {
		self.selected = Some(0);
	}

	pub fn select_last(&mut self) {
		self.selected = Some(usize::MAX);
	}

	/// Прокрутить на `delta` пунктов (отрицательное — назад), не меняя выделения. Дальше конца
	/// смещение не уходит: предел известен по последней отрисовке с полосой, иначе его
	/// подрежет следующая отрисовка.
	pub fn scroll_by(&mut self, delta: isize) {
		self.scroll_to(self.offset.saturating_add_signed(delta));
	}

	/// Прокрутить к пункту `offset` как к первому видимому, не меняя выделения.
	pub fn scroll_to(&mut self, offset: usize) {
		self.offset = self.scroll.clamp(offset);
	}
}

impl<M> Control<M> for ListState {
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
				// Полоса прокрутки раньше пунктов: нажатие на ней выделение не трогает.
				match self.scroll.press(msg, self.offset) {
					Some(offset) => self.offset = offset,
					None => {
						let hit = self.rows.item_at(self.node.area, msg.col, msg.row);
						self.apply(hit);
					}
				}
				Handled::Yes
			}
			// Протяжка ползунка: нажатие принято, список держит захват до отпускания.
			MouseKind::Move if self.node.captured => match self.scroll.drag(msg) {
				Some(offset) => {
					self.offset = offset;
					Handled::Yes
				}
				None => Handled::No,
			},
			MouseKind::Wheel { lines } => {
				// С полосой колесо прокручивает содержимое, без неё — двигает выделение.
				match self.scroll.wheel(self.offset, lines) {
					Some(offset) => self.offset = offset,
					None => {
						let next = selection::wheel(self.selected, self.rows.len(), lines);
						self.apply(next);
					}
				}
				Handled::Yes
			}
			// Отпускание после принятого нажатия приходит по захвату.
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

impl WidgetEvents for ListState {
	fn take_event(&mut self) -> Option<WidgetEvent> {
		self.event.take()
	}
}

#[derive(Clone, Debug, Default)]
pub struct List<'a> {
	items: Vec<ListItem<'a>>,
	block: Option<Block<'a>>,
	style: Style,
	highlight_style: Style,
	highlight_symbol: Option<&'a str>,
	focus_style: Option<Style>,
	scrollbar: Option<Scrollbar>,
}

impl<'a> List<'a> {
	pub fn new<I>(items: I) -> Self
	where
		I: IntoIterator,
		I::Item: Into<ListItem<'a>>,
	{
		Self {
			items: items.into_iter().map(Into::into).collect(),
			..Self::default()
		}
	}

	pub fn block(mut self, block: Block<'a>) -> Self {
		self.block = Some(block);
		self
	}

	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	pub fn highlight_style(mut self, style: Style) -> Self {
		self.highlight_style = style;
		self
	}

	/// Символ перед выделенным пунктом; остальные пункты сдвигаются на его ширину.
	pub fn highlight_symbol(mut self, symbol: &'a str) -> Self {
		self.highlight_symbol = Some(symbol);
		self
	}

	/// Стиль, накладываемый на рамку блока, пока список под фокусом (ADR-0026).
	pub fn focus_style(mut self, style: Style) -> Self {
		self.focus_style = Some(style);
		self
	}

	/// Полоса прокрутки в последней колонке списка (ADR-0032): значения подставляет сам список,
	/// мышь на полосе обрабатывает его состояние. Ориентация полосы — вертикальная, заданное в
	/// ней состояние не используется.
	pub fn scrollbar(mut self, scrollbar: Scrollbar) -> Self {
		self.scrollbar = Some(scrollbar);
		self
	}

	pub fn len(&self) -> usize {
		self.items.len()
	}

	pub fn is_empty(&self) -> bool {
		self.items.is_empty()
	}

	/// Наибольшее смещение: первый пункт последней страницы — все пункты от него до конца
	/// помещаются в `height` строк.
	fn last_offset(&self, height: usize) -> usize {
		let mut used = 0;
		let mut first = self.items.len();
		while first > 0 {
			used += self.items[first - 1].height();
			if used > height {
				break;
			}
			first -= 1;
		}
		// Пункт выше области целиком всё равно показывается, хотя бы частично.
		first.min(self.items.len().saturating_sub(1))
	}

	/// Диапазон видимых пунктов `[first, last)` при высоте `height`. При `is_fitting` смещение
	/// сдвигается так, чтобы выделенный пункт был виден целиком (или хотя бы начинался в
	/// области); иначе остаётся как есть, но не дальше последней страницы.
	fn visible_range(&self, state: &ListState, height: usize, is_fitting: bool) -> (usize, usize) {
		if self.items.is_empty() || height == 0 {
			return (0, 0);
		}
		let last_index = self.items.len() - 1;
		let selected = state.selected.map(|s| s.min(last_index));
		let mut first = state.offset.min(last_index);
		if !is_fitting {
			first = first.min(self.last_offset(height));
		} else if let Some(selected) = selected {
			if selected < first {
				first = selected;
			}
			// Сдвигаем вперёд, пока выделенный не влезет.
			loop {
				let mut used = 0;
				let mut fits = false;
				for item in &self.items[first..=selected] {
					used += item.height();
					if used > height {
						break;
					}
					fits = true;
				}
				if fits && used <= height || first == selected {
					break;
				}
				first += 1;
			}
		}
		let mut used = 0;
		let mut last = first;
		while last < self.items.len() {
			used += self.items[last].height();
			if used > height {
				break;
			}
			last += 1;
		}
		(first, last.max(first + 1).min(self.items.len()))
	}
}

impl StatefulWidget for List<'_> {
	type State = ListState;

	fn render(self, area: Rect, buf: &mut ScreenBuffer, state: &mut ListState) {
		let area = area.intersection(buf.area());
		if area.is_empty() {
			return;
		}
		buf.set_style(area, self.style);
		state.rows.reset(area.height, self.items.len());
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
		if inner.is_empty() || self.items.is_empty() {
			return;
		}
		if let Some(selected) = state.selected {
			state.selected = Some(selected.min(self.items.len() - 1));
		}
		// Под выделение смещение подгоняется, только когда изменилось то, от чего оно зависит;
		// иначе его оставляют там, куда прокрутила полоса.
		let is_fitting = state
			.scroll
			.take_fit(state.selected, inner.height, self.items.len());
		let (first, last) = self.visible_range(state, usize::from(inner.height), is_fitting);
		state.offset = first;
		// Полоса — в последней колонке; пунктам остаётся ширина без неё.
		let inner = match &self.scrollbar {
			Some(scrollbar) if inner.width >= 2 => {
				let bar_area = Rect::new(inner.right() - 1, inner.y, 1, inner.height);
				let scroll = ScrollbarState::new(self.items.len())
					.position(first)
					.viewport_length(last - first);
				state
					.scroll
					.set_layout(scrollbar.draw(bar_area, buf, &scroll));
				Rect::new(inner.x, inner.y, inner.width - 1, inner.height)
			}
			_ => inner,
		};
		let symbol = self.highlight_symbol.unwrap_or("");
		let symbol_width = symbol.chars().count().min(usize::from(inner.width)) as u16;
		let text_area = Rect::new(
			inner.x + symbol_width,
			inner.y,
			inner.width - symbol_width,
			inner.height,
		);
		let mut y = inner.y;
		for (index, item) in self.items.iter().enumerate().take(last).skip(first) {
			let is_selected = state.selected == Some(index);
			let item_style = self.style.patch(item.style);
			let item_style = if is_selected {
				item_style.patch(self.highlight_style)
			} else {
				item_style
			};
			let height = item.height().min(usize::from(inner.bottom() - y)) as u16;
			let item_area = Rect::new(inner.x, y, inner.width, height);
			for row in y..y + height {
				state.rows.mark(row - area.y, index);
			}
			buf.set_style(item_area, item_style);
			if is_selected && symbol_width > 0 {
				buf.set_string(inner.x, y, symbol, item_style, item_area);
			}
			for (i, line) in item
				.content
				.lines
				.iter()
				.take(usize::from(height))
				.enumerate()
			{
				let row = Rect::new(text_area.x, y + i as u16, text_area.width, 1);
				let styled = StyledLine {
					chars: line
						.styled_chars(item_style.patch(item.content.style))
						.collect(),
					alignment: line
						.alignment
						.or(item.content.alignment)
						.unwrap_or_default(),
				};
				buf.set_line(row, &styled, 0);
			}
			y += height;
			if y >= inner.bottom() {
				break;
			}
		}
	}
}

impl Widget for List<'_> {
	fn render(self, area: Rect, buf: &mut ScreenBuffer) {
		let mut state = ListState::default();
		StatefulWidget::render(self, area, buf, &mut state);
	}
}
