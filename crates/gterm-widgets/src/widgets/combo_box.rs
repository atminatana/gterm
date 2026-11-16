//! Выпадающий список (ADR-0027): закрытое поле с текущим значением и маркером `▾`/`▴` (малые треугольники из шрифта: полноразмерные `▼`/`▲` `gterm` рисует процедурно во всю ячейку);
//! раскрытие — слой [`Modal`] под полем (над — при нехватке места) с контролом списка над тем
//! же состоянием. Поле и слой строит один вызов [`Ui::combo_box`]: пока список раскрыт, поле
//! рисуется снимком состояния, а `&mut` состояние держит слой — приложению не нужно занимать
//! его дважды. Выбор (`Enter`, клик по строке) закрывает список и публикует привязку
//! `on_select`; `Esc` и клик мимо список сворачивает сам. В закрытом поле стрелки значения не
//! листают — раскрытие единственный путь.

use gterm::buffer::ScreenBuffer;
use winit::keyboard::NamedKey;

use crate::buffer::BufferExt;
use crate::control::{Control, Drawable, NodeState, WidgetEvent, WidgetEvents};
use crate::geometry::Rect;
use crate::input::{Handled, KeyMsg, Messages, MouseKind, MouseMsg};
use crate::layout::{Constraint, Layout};
use crate::style::Style;
use crate::ui::{Modal, Ui};
use crate::widgets::selection;
use crate::widgets::{Block, List, ListState, StatefulWidget, Widget};

/// Маркеры закрытого и раскрытого поля.
const MARKER_CLOSED: char = '▾';
const MARKER_OPEN: char = '▴';

/// Состояние выпадающего списка: выбранный пункт, раскрыт ли, выделение раскрытого списка.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComboBoxState {
	pub selected: Option<usize>,
	/// Список раскрыт: `Ui::combo_box` добавляет слой, пока флаг стоит.
	pub open: bool,
	/// Выделение в раскрытом списке (стартует с текущего значения).
	pub list: ListState,
	pub node: NodeState,
	pub enabled: bool,
	/// Узел контрола списка внутри слоя (фокус и область).
	list_node: NodeState,
	event: Option<WidgetEvent>,
}

impl Default for ComboBoxState {
	fn default() -> Self {
		Self {
			selected: None,
			open: false,
			list: ListState::default(),
			node: NodeState::default(),
			enabled: true,
			list_node: NodeState::default(),
			event: None,
		}
	}
}

impl ComboBoxState {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn with_selected(selected: Option<usize>) -> Self {
		Self {
			selected,
			..Self::default()
		}
	}

	pub fn selected(&self) -> Option<usize> {
		self.selected
	}

	/// Выбрать пункт; событие `Selected`, если значение изменилось.
	pub fn select(&mut self, index: Option<usize>) {
		if index != self.selected {
			self.selected = index;
			if let Some(index) = index {
				self.event = Some(WidgetEvent::Selected(index));
			}
		}
	}

	/// Раскрыть список: выделение стартует с текущего значения.
	pub fn open_list(&mut self) {
		self.open = true;
		self.list.select(self.selected);
		self.list_node.focused = true;
	}

	pub fn close_list(&mut self) {
		self.open = false;
	}
}

impl<M> Control<M> for ComboBoxState {
	fn node(&self) -> &NodeState {
		&self.node
	}

	fn node_mut(&mut self) -> &mut NodeState {
		&mut self.node
	}

	fn focusable(&self) -> bool {
		self.enabled
	}

	fn key(&mut self, msg: &KeyMsg<'_>, _out: &mut Messages<M>) -> Handled {
		if !self.enabled {
			return Handled::No;
		}
		if msg.is_named(NamedKey::Enter)
			|| msg.is_named(NamedKey::Space)
			|| msg.is_char(" ")
			|| msg.is_named(NamedKey::ArrowDown)
		{
			self.open_list();
			return Handled::Yes;
		}
		Handled::No
	}

	fn mouse(&mut self, msg: &MouseMsg, _out: &mut Messages<M>) -> Handled {
		if !self.enabled {
			return Handled::No;
		}
		match msg.kind {
			MouseKind::Down if msg.is_left_down() => {
				self.open_list();
				Handled::Yes
			}
			MouseKind::Up | MouseKind::Cancel => Handled::Yes,
			_ => Handled::No,
		}
	}
}

impl WidgetEvents for ComboBoxState {
	fn take_event(&mut self) -> Option<WidgetEvent> {
		self.event.take()
	}
}

/// Содержимое слоя: список пунктов над выделением `ComboBoxState::list`; держит ссылки на поля
/// состояния, пока список раскрыт.
struct DropdownList<'a, M> {
	/// Узел списка внутри слоя (`ComboBoxState::list_node`).
	node: &'a mut NodeState,
	selected: &'a mut Option<usize>,
	open: &'a mut bool,
	list: &'a mut ListState,
	items: &'a [&'a str],
	page: usize,
	highlight_style: Style,
	on_select: Box<dyn Fn(usize) -> M + 'a>,
}

impl<M> DropdownList<'_, M> {
	fn choose(&mut self, index: Option<usize>, out: &mut Messages<M>) {
		if let Some(index) = index.filter(|&i| i < self.items.len()) {
			if Some(index) != *self.selected {
				*self.selected = Some(index);
				out.push((self.on_select)(index));
			}
		}
		*self.open = false;
	}
}

impl<M> Control<M> for DropdownList<'_, M> {
	fn node(&self) -> &NodeState {
		self.node
	}

	fn node_mut(&mut self) -> &mut NodeState {
		self.node
	}

	fn focusable(&self) -> bool {
		true
	}

	fn key(&mut self, msg: &KeyMsg<'_>, out: &mut Messages<M>) -> Handled {
		if msg.is_named(NamedKey::Enter) {
			let chosen = self.list.selected.or(Some(0));
			self.choose(chosen, out);
			return Handled::Yes;
		}
		if msg.is_named(NamedKey::Escape) {
			*self.open = false;
			return Handled::Yes;
		}
		match selection::navigate(msg, self.list.selected, self.items.len(), self.page) {
			Some(next) => {
				self.list.selected = next;
				Handled::Yes
			}
			None => Handled::No,
		}
	}

	fn mouse(&mut self, msg: &MouseMsg, out: &mut Messages<M>) -> Handled {
		match msg.kind {
			MouseKind::Down if msg.is_left_down() => {
				let area = self.node.area;
				if let Ok(row) = u16::try_from(msg.row) {
					if (area.y..area.bottom()).contains(&row) {
						let index = self.list.offset + usize::from(row - area.y);
						self.choose(Some(index), out);
					}
				}
				Handled::Yes
			}
			MouseKind::Wheel { lines } => {
				self.list.selected = selection::wheel(self.list.selected, self.items.len(), lines);
				Handled::Yes
			}
			MouseKind::Up | MouseKind::Cancel => Handled::Yes,
			MouseKind::Down | MouseKind::Move => Handled::No,
		}
	}

	fn dismiss(&mut self) {
		*self.open = false;
	}
}

impl<M> Drawable for DropdownList<'_, M> {
	fn draw(&mut self, area: Rect, buf: &mut ScreenBuffer) {
		let list = List::new(self.items.iter().copied()).highlight_style(self.highlight_style);
		StatefulWidget::render(list, area, buf, self.list);
	}
}

/// Закрытое поле списка: текущее значение (или подсказка) и маркер справа.
#[derive(Clone, Debug, Default)]
pub struct ComboBox<'a> {
	items: &'a [&'a str],
	style: Style,
	focus_style: Style,
	highlight_style: Style,
	placeholder: &'a str,
	block: Option<Block<'a>>,
	/// Строк в раскрытом списке (без рамки).
	rows: u16,
}

impl<'a> ComboBox<'a> {
	pub fn new(items: &'a [&'a str]) -> Self {
		Self {
			items,
			rows: 6,
			..Self::default()
		}
	}

	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	/// Накладывается на поле и рамку, пока поле под фокусом.
	pub fn focus_style(mut self, style: Style) -> Self {
		self.focus_style = style;
		self
	}

	/// Стиль выделенной строки раскрытого списка.
	pub fn highlight_style(mut self, style: Style) -> Self {
		self.highlight_style = style;
		self
	}

	/// Текст при отсутствии выбора.
	pub fn placeholder(mut self, placeholder: &'a str) -> Self {
		self.placeholder = placeholder;
		self
	}

	pub fn block(mut self, block: Block<'a>) -> Self {
		self.block = Some(block);
		self
	}

	/// Высота раскрытого списка в строках (без рамки); по умолчанию 6, не больше числа пунктов.
	pub fn rows(mut self, rows: u16) -> Self {
		self.rows = rows.max(1);
		self
	}

	fn draw(
		&self,
		area: Rect,
		buf: &mut ScreenBuffer,
		selected: Option<usize>,
		open: bool,
		focused: bool,
	) {
		let area = area.intersection(buf.area());
		if area.is_empty() {
			return;
		}
		let style = if focused {
			self.style.patch(self.focus_style)
		} else {
			self.style
		};
		buf.set_style(area, style);
		let inner = match &self.block {
			Some(block) => {
				let inner = block.inner(area);
				let mut block = block.clone();
				if focused {
					block.patch_border_style(self.focus_style);
				}
				block.render(area, buf);
				inner
			}
			None => area,
		};
		if inner.is_empty() {
			return;
		}
		let value = selected
			.and_then(|index| self.items.get(index).copied())
			.unwrap_or(self.placeholder);
		let row = Rect::new(inner.x, inner.y, inner.width.saturating_sub(2), 1);
		buf.set_string(inner.x, inner.y, value, style, row);
		let marker = if open { MARKER_OPEN } else { MARKER_CLOSED };
		buf.set_styled(inner.right() - 1, inner.y, marker, style, inner);
	}
}

impl StatefulWidget for ComboBox<'_> {
	type State = ComboBoxState;

	fn render(self, area: Rect, buf: &mut ScreenBuffer, state: &mut ComboBoxState) {
		if let Some(selected) = state.selected {
			state.selected = Some(selected.min(self.items.len().saturating_sub(1)));
		}
		let focused = state.node.focused && state.enabled;
		self.draw(area, buf, state.selected, state.open, focused);
	}
}

/// Снимок поля на время раскрытого списка: рисуется без состояния, ввод не принимает.
struct ComboBoxView<'a> {
	widget: ComboBox<'a>,
	selected: Option<usize>,
	focused: bool,
}

impl Widget for ComboBoxView<'_> {
	fn render(self, area: Rect, buf: &mut ScreenBuffer) {
		self.widget
			.draw(area, buf, self.selected, true, self.focused);
	}
}

impl<'a, M: 'a> Ui<'a, M> {
	/// Выпадающий список: закрытое поле как встроенный контрол, а пока `state.open` — ещё и
	/// слой списка под полем (над — при нехватке места), который поднимается к корню описания.
	/// `on_select` публикуется при выборе нового значения; слой сворачивается сам по выбору,
	/// `Esc` и клику мимо. Фон слоя — `style` виджета.
	pub fn combo_box(
		widget: ComboBox<'a>,
		state: &'a mut ComboBoxState,
		on_select: impl Fn(usize) -> M + 'a,
	) -> Self {
		if !state.open {
			return Self::stateful(widget, state).on_select(on_select);
		}
		// Раскрытие кликом оставляет полю захват указателя: нажатие принято, а парное отпускание
		// уходит уже слою. Пока список раскрыт, поля в дереве нет и держать указатель ему нечем —
		// иначе после закрытия следующее нажатие где угодно досталось бы полю и раскрыло список
		// заново.
		state.node.captured = false;
		let anchor = state.node.area;
		let items = widget.items;
		let rows = widget
			.rows
			.min(items.len().clamp(1, usize::from(u16::MAX)) as u16);
		let view = ComboBoxView {
			selected: state.selected,
			focused: state.node.focused,
			widget: widget.clone(),
		};
		let ComboBoxState {
			selected,
			open,
			list: list_state,
			list_node,
			..
		} = state;
		let list = DropdownList {
			node: list_node,
			selected,
			open,
			list: list_state,
			items,
			page: usize::from(rows),
			highlight_style: widget.highlight_style,
			on_select: Box::new(on_select),
		};
		let dropdown = Modal::below(anchor, rows + 2)
			.style(widget.style)
			.dismiss_on_click_outside()
			.content(Ui::group(
				Block::bordered(),
				Layout::vertical([Constraint::Fill(1)]),
				vec![Ui::owned(list)],
			));
		Self::widget(view).modal(dropdown)
	}
}
