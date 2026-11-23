//! Вкладки (ADR-0032): ряд заголовков в одну строку с разделителем, одна вкладка выбрана.
//! Контрол: под фокусом `←`/`→`/`Home`/`End` меняют выбор, левое нажатие по заголовку выбирает
//! его; смену дерево превращает в сообщение привязки `on_select`. Число вкладок и их колонки
//! состояние узнаёт на отрисовке, поэтому до первого кадра клавиши и мышь выбор не меняют.
//! Заголовки, не поместившиеся по ширине, обрезаются.

use gterm::buffer::ScreenBuffer;
use winit::keyboard::NamedKey;

use crate::buffer::BufferExt;
use crate::control::{Control, NodeState, WidgetEvent, WidgetEvents};
use crate::geometry::Rect;
use crate::input::{Handled, KeyMsg, Messages, MouseKind, MouseMsg};
use crate::style::Style;
use crate::text::Line;
use crate::theme::{Theme, scope};
use crate::widgets::{Block, StatefulWidget, Widget};

/// Состояние вкладок: выбранная, активность, фокус и область — от дерева.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabsState {
	pub node: NodeState,
	/// Неактивные вкладки не фокусируются и не реагируют на ввод.
	pub enabled: bool,
	pub selected: usize,
	/// Колонки вкладок на последней отрисовке (`начало..конец`, с отступами) — для клика;
	/// длина — число вкладок.
	spans: Vec<(u16, u16)>,
	/// Строка заголовков на последней отрисовке.
	row: u16,
	event: Option<WidgetEvent>,
}

impl Default for TabsState {
	fn default() -> Self {
		Self {
			node: NodeState::default(),
			enabled: true,
			selected: 0,
			spans: Vec::new(),
			row: 0,
			event: None,
		}
	}
}

impl TabsState {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn with_selected(selected: usize) -> Self {
		Self {
			selected,
			..Self::default()
		}
	}

	pub fn selected(&self) -> usize {
		self.selected
	}

	/// Выбрать вкладку программно, без события; индекс за последней вкладкой подрежет отрисовка.
	pub fn select(&mut self, index: usize) {
		self.selected = index;
	}

	/// Выбор пользователем: событие — только при смене и только на существующую вкладку.
	fn choose(&mut self, index: usize) {
		if index < self.spans.len() && index != self.selected {
			self.selected = index;
			self.event = Some(WidgetEvent::Selected(index));
		}
	}

	fn last(&self) -> usize {
		self.spans.len().saturating_sub(1)
	}

	/// Вкладка под ячейкой по последней отрисовке.
	fn tab_at(&self, col: i32, row: i32) -> Option<usize> {
		let (Ok(col), Ok(row)) = (u16::try_from(col), u16::try_from(row)) else {
			return None;
		};
		if row != self.row {
			return None;
		}
		self.spans
			.iter()
			.position(|&(start, end)| (start..end).contains(&col))
	}
}

impl<M> Control<M> for TabsState {
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
		let target = if msg.is_named(NamedKey::ArrowLeft) {
			self.selected.saturating_sub(1)
		} else if msg.is_named(NamedKey::ArrowRight) {
			(self.selected + 1).min(self.last())
		} else if msg.is_named(NamedKey::Home) {
			0
		} else if msg.is_named(NamedKey::End) {
			self.last()
		} else {
			return Handled::No;
		};
		self.choose(target);
		Handled::Yes
	}

	fn mouse(&mut self, msg: &MouseMsg, _out: &mut Messages<M>) -> Handled {
		if !self.enabled {
			return Handled::No;
		}
		match msg.kind {
			MouseKind::Down if msg.is_left_down() => match self.tab_at(msg.col, msg.row) {
				Some(index) => {
					self.choose(index);
					Handled::Yes
				}
				// Мимо заголовков (разделитель, пустое место) — не наше нажатие.
				None => Handled::No,
			},
			MouseKind::Up | MouseKind::Cancel if self.node.captured => Handled::Yes,
			_ => Handled::No,
		}
	}
}

impl WidgetEvents for TabsState {
	fn take_event(&mut self) -> Option<WidgetEvent> {
		self.event.take()
	}
}

/// Ряд вкладок: заголовки с отступами по краям и разделителем между ними.
#[derive(Clone, Debug)]
pub struct Tabs<'a> {
	titles: Vec<Line<'a>>,
	block: Option<Block<'a>>,
	style: Style,
	highlight_style: Style,
	focus_style: Style,
	divider: &'a str,
	padding: (&'a str, &'a str),
}

impl<'a> Tabs<'a> {
	pub fn new<I, T>(titles: I) -> Self
	where
		I: IntoIterator<Item = T>,
		T: Into<Line<'a>>,
	{
		Self {
			titles: titles.into_iter().map(Into::into).collect(),
			block: None,
			style: Style::new(),
			highlight_style: Style::new().bold().underlined(),
			focus_style: Style::new(),
			divider: "│",
			padding: (" ", " "),
		}
	}

	pub fn block(mut self, block: Block<'a>) -> Self {
		self.block = Some(block);
		self
	}

	/// Стиль всей области вкладок.
	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	/// Стиль выбранной вкладки (по умолчанию жирный с подчёркиванием).
	pub fn highlight_style(mut self, style: Style) -> Self {
		self.highlight_style = style;
		self
	}

	/// Накладывается на выбранную вкладку и рамку блока, пока вкладки под фокусом.
	pub fn focus_style(mut self, style: Style) -> Self {
		self.focus_style = style;
		self
	}

	/// Разделитель между вкладками (по умолчанию `│`).
	pub fn divider(mut self, divider: &'a str) -> Self {
		self.divider = divider;
		self
	}

	/// Отступы слева и справа от заголовка (по умолчанию по пробелу); входят в область клика.
	pub fn padding(mut self, left: &'a str, right: &'a str) -> Self {
		self.padding = (left, right);
		self
	}

	/// Подставить стили темы под заданные явно (ADR-0037) — для виджета вне дерева `Ui`; в
	/// дереве то же делает [`Ui::theme`](crate::ui::Ui::theme).
	pub fn themed(mut self, theme: &Theme) -> Self {
		self.set_theme(theme);
		self
	}

	/// Области `ui.tabs.*`; в теме без них — строка буферов helix (`ui.bufferline`).
	pub(crate) fn set_theme(&mut self, theme: &Theme) {
		self.style = theme.first(&["ui.tabs", "ui.bufferline"]).patch(self.style);
		self.highlight_style = theme
			.first(&[
				"ui.tabs.selected",
				"ui.bufferline.active",
				scope::MENU_SELECTED,
			])
			.patch(self.highlight_style);
		self.focus_style = theme
			.first(&["ui.tabs.focus", scope::TEXT_FOCUS])
			.patch(self.focus_style);
		if let Some(block) = &mut self.block {
			block.set_theme(theme);
		}
	}
}

impl StatefulWidget for Tabs<'_> {
	type State = TabsState;

	fn apply_theme(&mut self, theme: &Theme) {
		self.set_theme(theme);
	}

	fn render(self, area: Rect, buf: &mut ScreenBuffer, state: &mut TabsState) {
		state.spans.clear();
		let area = area.intersection(buf.area());
		if area.is_empty() {
			return;
		}
		let focused = state.node.focused && state.enabled;
		buf.set_style(area, self.style);
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
		state.selected = state.selected.min(self.titles.len().saturating_sub(1));
		let row = Rect::new(inner.x, inner.y, inner.width, 1);
		state.row = row.y;
		let mut x = row.x;
		for (index, title) in self.titles.iter().enumerate() {
			if index > 0 {
				x = x.saturating_add(buf.set_string(x, row.y, self.divider, Style::new(), row));
			}
			let tab_style = if index != state.selected {
				Style::new()
			} else if focused {
				self.highlight_style.patch(self.focus_style)
			} else {
				self.highlight_style
			};
			let start = x.min(row.right());
			x = x.saturating_add(buf.set_string(x, row.y, self.padding.0, tab_style, row));
			for (ch, style) in title.styled_chars(tab_style) {
				if x >= row.right() {
					break;
				}
				buf.set_styled(x, row.y, ch, style, row);
				x += 1;
			}
			x = x.saturating_add(buf.set_string(x, row.y, self.padding.1, tab_style, row));
			// Вкладка, не поместившаяся целиком или вовсе, получает обрезанную либо пустую
			// область — индексы областей совпадают с индексами заголовков.
			state.spans.push((start, x.min(row.right())));
		}
	}
}
