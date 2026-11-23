//! Однострочное поле ввода (ADR-0027): значение, курсор и прокрутка в состоянии приложения;
//! текст приходит через `Control::text` (IME), клавиши — стрелки, `Home`/`End`, `Backspace`,
//! `Delete`, `Enter` (`Submitted`); символьные клавиши поле не обрабатывает — иначе ввод
//! удвоился бы. Курсор рисует виджет обменом fg/bg только под фокусом, без мигания (ТЗ §13,
//! ADR-0012). Выделение (ADR-0030) — промежуток между якорем и курсором: `Shift` со стрелками
//! и `Home`/`End`, протяжка мышью, «выделить всё»; набор, вставка, `Backspace` и `Delete`
//! заменяют выделенное. Копирование, вырезание и вставка — команды правки `Control::edit`.

use gterm::buffer::ScreenBuffer;
use winit::keyboard::NamedKey;

use crate::buffer::BufferExt;
use crate::clipboard::{Clipboard, EditCommand};
use crate::control::{Control, NodeState, WidgetEvent, WidgetEvents};
use crate::geometry::Rect;
use crate::input::{Handled, KeyMsg, Messages, MouseKind, MouseMsg};
use crate::style::Style;
use crate::text::Line;
use crate::theme::{Theme, scope};
use crate::widgets::{Block, StatefulWidget, Widget};

/// Состояние поля: значение, курсор (в символах, `0..=len`), первый видимый символ.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextInputState {
	pub value: String,
	pub cursor: usize,
	pub scroll: usize,
	pub node: NodeState,
	/// Неактивное поле не фокусируется и не меняется.
	pub enabled: bool,
	/// Якорь выделения (в символах): выделено между ним и курсором; `None` или равен курсору —
	/// выделения нет.
	anchor: Option<usize>,
	/// Первая колонка текста на последней отрисовке — для клика.
	text_x: u16,
	/// Ширина текста на последней отрисовке — для прокрутки при протяжке за край.
	text_width: u16,
	event: Option<WidgetEvent>,
}

impl Default for TextInputState {
	fn default() -> Self {
		Self {
			value: String::new(),
			cursor: 0,
			scroll: 0,
			node: NodeState::default(),
			enabled: true,
			anchor: None,
			text_x: 0,
			text_width: 0,
			event: None,
		}
	}
}

impl TextInputState {
	pub fn new() -> Self {
		Self::default()
	}

	/// Поле с начальным значением, курсор в конце.
	pub fn with_value(value: impl Into<String>) -> Self {
		let value = value.into();
		Self {
			cursor: value.chars().count(),
			value,
			..Self::default()
		}
	}

	pub fn value(&self) -> &str {
		&self.value
	}

	pub fn len(&self) -> usize {
		self.value.chars().count()
	}

	pub fn is_empty(&self) -> bool {
		self.value.is_empty()
	}

	/// Заменить значение целиком (курсор в конце, выделение снято). Программная смена — дело
	/// приложения, события `Changed` она не даёт: события отражают ввод пользователя.
	pub fn set_value(&mut self, value: impl Into<String>) {
		self.value = value.into();
		self.cursor = self.len();
		self.scroll = 0;
		self.anchor = None;
	}

	pub fn clear(&mut self) {
		self.set_value(String::new());
	}

	/// Выделенный промежуток `(начало, конец)` в символах; `None` — выделения нет.
	pub fn selection(&self) -> Option<(usize, usize)> {
		let len = self.len();
		let anchor = self.anchor?.min(len);
		let cursor = self.cursor.min(len);
		(anchor != cursor).then(|| (anchor.min(cursor), anchor.max(cursor)))
	}

	/// Выделенный текст; пустая строка — выделения нет.
	pub fn selected_text(&self) -> &str {
		match self.selection() {
			Some((start, end)) => &self.value[self.byte_index(start)..self.byte_index(end)],
			None => "",
		}
	}

	/// Выделить всё значение, курсор в конце.
	pub fn select_all(&mut self) {
		self.anchor = Some(0);
		self.cursor = self.len();
	}

	pub fn clear_selection(&mut self) {
		self.anchor = None;
	}

	/// Удалить выделенное (курсор — в его начало); `false` — выделения не было.
	pub fn delete_selection(&mut self) -> bool {
		let is_removed = self.remove_selection();
		if is_removed {
			self.changed();
		}
		is_removed
	}

	/// Вставить текст в позицию курсора вместо выделенного; управляющие символы отбрасываются.
	pub fn insert_str(&mut self, text: &str) {
		let clean: String = text.chars().filter(|ch| !ch.is_control()).collect();
		self.insert_clean(&clean);
	}

	/// Вставить текст из буфера обмена (ADR-0030): поле однострочное, поэтому перевод строки и
	/// табуляция становятся пробелом, прочие управляющие символы отбрасываются.
	pub fn paste_str(&mut self, text: &str) {
		let clean: String = text
			.replace("\r\n", "\n")
			.chars()
			.filter_map(|ch| match ch {
				'\n' | '\r' | '\t' => Some(' '),
				ch if ch.is_control() => None,
				ch => Some(ch),
			})
			.collect();
		self.insert_clean(&clean);
	}

	/// Удалить выделенное либо символ перед курсором.
	pub fn backspace(&mut self) {
		if self.delete_selection() || self.cursor == 0 {
			return;
		}
		self.cursor -= 1;
		self.remove_at_cursor();
	}

	/// Удалить выделенное либо символ под курсором.
	pub fn delete(&mut self) {
		if self.delete_selection() || self.cursor >= self.len() {
			return;
		}
		self.remove_at_cursor();
	}

	/// Курсор влево; при выделении — в его начало. Выделение снимается.
	pub fn move_left(&mut self) {
		self.cursor = match self.selection() {
			Some((start, _)) => start,
			None => self.cursor.saturating_sub(1),
		};
		self.anchor = None;
	}

	/// Курсор вправо; при выделении — в его конец. Выделение снимается.
	pub fn move_right(&mut self) {
		self.cursor = match self.selection() {
			Some((_, end)) => end,
			None => (self.cursor + 1).min(self.len()),
		};
		self.anchor = None;
	}

	pub fn move_home(&mut self) {
		self.cursor = 0;
		self.anchor = None;
	}

	pub fn move_end(&mut self) {
		self.cursor = self.len();
		self.anchor = None;
	}

	/// Курсор в колонку ячейки `col` системы координат дерева (по последней отрисовке);
	/// выделение снимается.
	pub fn cursor_to_col(&mut self, col: i32) {
		self.cursor = self.position_at(col);
		self.anchor = None;
	}

	/// Перенести курсор в `position`, оставив якорь на месте: выделение растёт или сжимается.
	fn extend_to(&mut self, position: usize) {
		if self.anchor.is_none() {
			self.anchor = Some(self.cursor);
		}
		self.cursor = position.min(self.len());
	}

	/// Позиция символа под колонкой `col`; левее текста — первый видимый символ.
	fn position_at(&self, col: i32) -> usize {
		let offset = usize::try_from(col - i32::from(self.text_x)).unwrap_or(0);
		(self.scroll + offset).min(self.len())
	}

	/// Позиция для протяжки мышью: за краем текста — на символ дальше видимого, чтобы следующая
	/// отрисовка прокрутила поле в эту сторону.
	fn drag_position(&self, col: i32) -> usize {
		let offset = col - i32::from(self.text_x);
		if offset < 0 {
			self.scroll.saturating_sub(1)
		} else if offset >= i32::from(self.text_width) {
			(self.scroll + usize::from(self.text_width)).min(self.len())
		} else {
			self.position_at(col)
		}
	}

	fn insert_clean(&mut self, clean: &str) {
		if clean.is_empty() {
			return;
		}
		self.remove_selection();
		let at = self.byte_index(self.cursor);
		self.value.insert_str(at, clean);
		self.cursor += clean.chars().count();
		self.changed();
	}

	/// Убрать выделенное без события; якорь снимается в любом случае.
	fn remove_selection(&mut self) -> bool {
		let selection = self.selection();
		self.anchor = None;
		let Some((start, end)) = selection else {
			return false;
		};
		let range = self.byte_index(start)..self.byte_index(end);
		self.value.replace_range(range, "");
		self.cursor = start;
		true
	}

	fn remove_at_cursor(&mut self) {
		let start = self.byte_index(self.cursor);
		let end = self.byte_index(self.cursor + 1);
		self.value.replace_range(start..end, "");
		self.changed();
	}

	fn byte_index(&self, chars: usize) -> usize {
		self.value
			.char_indices()
			.nth(chars)
			.map_or(self.value.len(), |(index, _)| index)
	}

	fn changed(&mut self) {
		self.event = Some(WidgetEvent::Changed(self.value.clone()));
	}

	/// Подогнать прокрутку под ширину области так, чтобы курсор был виден (вызывает отрисовка).
	fn fit_scroll(&mut self, width: usize) {
		if width == 0 {
			return;
		}
		self.cursor = self.cursor.min(self.len());
		if self.cursor < self.scroll {
			self.scroll = self.cursor;
		} else if self.cursor >= self.scroll + width {
			self.scroll = self.cursor + 1 - width;
		}
	}
}

impl<M> Control<M> for TextInputState {
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
		// `Shift` с клавишей движения расширяет выделение от якоря, без него — снимает.
		let is_extending = msg.modifiers.shift_key();
		if msg.is_named(NamedKey::ArrowLeft) {
			if is_extending {
				self.extend_to(self.cursor.saturating_sub(1));
			} else {
				self.move_left();
			}
		} else if msg.is_named(NamedKey::ArrowRight) {
			if is_extending {
				self.extend_to(self.cursor + 1);
			} else {
				self.move_right();
			}
		} else if msg.is_named(NamedKey::Home) {
			if is_extending {
				self.extend_to(0);
			} else {
				self.move_home();
			}
		} else if msg.is_named(NamedKey::End) {
			if is_extending {
				self.extend_to(self.len());
			} else {
				self.move_end();
			}
		} else if msg.is_named(NamedKey::Backspace) {
			self.backspace();
		} else if msg.is_named(NamedKey::Delete) {
			self.delete();
		} else if msg.is_named(NamedKey::Enter) {
			self.event = Some(WidgetEvent::Submitted(self.value.clone()));
		} else {
			return Handled::No;
		}
		Handled::Yes
	}

	fn mouse(&mut self, msg: &MouseMsg, _out: &mut Messages<M>) -> Handled {
		if !self.enabled {
			return Handled::No;
		}
		match msg.kind {
			MouseKind::Down if msg.is_left_down() => {
				let position = self.position_at(msg.col);
				if msg.modifiers.shift_key() {
					self.extend_to(position);
				} else {
					self.cursor = position;
					self.anchor = Some(position);
				}
				Handled::Yes
			}
			// Протяжка: нажатие принято, поле держит захват до отпускания (ADR-0026).
			MouseKind::Move if self.node.captured => {
				self.extend_to(self.drag_position(msg.col));
				Handled::Yes
			}
			MouseKind::Up | MouseKind::Cancel => {
				if self.selection().is_none() {
					self.anchor = None;
				}
				Handled::Yes
			}
			_ => Handled::No,
		}
	}

	fn text(&mut self, text: &str, _out: &mut Messages<M>) -> Handled {
		if !self.enabled {
			return Handled::No;
		}
		self.insert_str(text);
		Handled::Yes
	}

	/// Команды правки поле под фокусом поглощает всегда — и при пустом выделении, и с пустым
	/// буфером (ADR-0030): до карты сочетаний они не доходят.
	fn edit(
		&mut self,
		command: EditCommand,
		clipboard: &mut dyn Clipboard,
		_out: &mut Messages<M>,
	) -> Handled {
		if !self.enabled {
			return Handled::No;
		}
		match command {
			EditCommand::Copy => {
				if self.selection().is_some() {
					clipboard.set(self.selected_text());
				}
			}
			// Без доступного буфера вырезание текст не трогает — иначе он пропал бы.
			EditCommand::Cut => {
				if self.selection().is_some() && clipboard.is_available() {
					clipboard.set(self.selected_text());
					self.delete_selection();
				}
			}
			EditCommand::Paste => {
				if let Some(text) = clipboard.get() {
					self.paste_str(&text);
				}
			}
			EditCommand::SelectAll => self.select_all(),
		}
		Handled::Yes
	}
}

impl WidgetEvents for TextInputState {
	fn take_event(&mut self) -> Option<WidgetEvent> {
		self.event.take()
	}
}

/// Поле ввода: одна строка текста с прокруткой, подсказка при пустом значении, курсор и
/// выделение под фокусом.
#[derive(Clone, Debug, Default)]
pub struct TextInput<'a> {
	style: Style,
	focus_style: Style,
	selection_style: Option<Style>,
	placeholder: Option<Line<'a>>,
	placeholder_style: Style,
	block: Option<Block<'a>>,
}

impl<'a> TextInput<'a> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	/// Накладывается на всё поле и на рамку блока, пока поле под фокусом.
	pub fn focus_style(mut self, style: Style) -> Self {
		self.focus_style = style;
		self
	}

	/// Стиль выделенных ячеек (ADR-0030); без него — обмен fg/bg, как у курсора.
	pub fn selection_style(mut self, style: Style) -> Self {
		self.selection_style = Some(style);
		self
	}

	/// Текст при пустом значении.
	pub fn placeholder<T: Into<Line<'a>>>(mut self, placeholder: T) -> Self {
		self.placeholder = Some(placeholder.into());
		self
	}

	pub fn placeholder_style(mut self, style: Style) -> Self {
		self.placeholder_style = style;
		self
	}

	pub fn block(mut self, block: Block<'a>) -> Self {
		self.block = Some(block);
		self
	}

	/// Подставить стили темы под заданные явно (ADR-0037) — для виджета вне дерева `Ui`; в
	/// дереве то же делает [`Ui::theme`](crate::ui::Ui::theme).
	pub fn themed(mut self, theme: &Theme) -> Self {
		self.set_theme(theme);
		self
	}

	/// Области `ui.input.*`; в теме без них поле — как меню helix, выделение — `ui.selection`.
	pub(crate) fn set_theme(&mut self, theme: &Theme) {
		self.style = theme.first(&["ui.input", scope::MENU]).patch(self.style);
		self.focus_style = theme
			.first(&["ui.input.focus", scope::TEXT_FOCUS])
			.patch(self.focus_style);
		self.selection_style = self
			.selection_style
			.or_else(|| theme.find(&["ui.input.selection", scope::SELECTION]));
		self.placeholder_style = theme
			.inactive("ui.input.placeholder")
			.patch(self.placeholder_style);
		if let Some(block) = &mut self.block {
			block.set_theme(theme);
		}
	}
}

/// Поменять местами fg и bg ячейки — вид курсора и выделения по умолчанию.
fn swap_colors(buf: &mut ScreenBuffer, x: u16, y: u16) {
	let cell = buf.cell(usize::from(x), usize::from(y));
	buf.set_style(Rect::new(x, y, 1, 1), Style::new().fg(cell.bg).bg(cell.fg));
}

impl StatefulWidget for TextInput<'_> {
	type State = TextInputState;

	fn apply_theme(&mut self, theme: &Theme) {
		self.set_theme(theme);
	}

	fn render(self, area: Rect, buf: &mut ScreenBuffer, state: &mut TextInputState) {
		let area = area.intersection(buf.area());
		if area.is_empty() {
			return;
		}
		let focused = state.node.focused && state.enabled;
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
		state.text_x = inner.x;
		state.text_width = inner.width;
		let width = usize::from(inner.width);
		state.fit_scroll(width);
		let row = Rect::new(inner.x, inner.y, inner.width, 1);
		if state.value.is_empty() {
			if let Some(placeholder) = &self.placeholder {
				let styled = crate::text::StyledLine {
					chars: placeholder
						.styled_chars(style.patch(self.placeholder_style))
						.collect(),
					alignment: crate::text::Alignment::Left,
				};
				buf.set_line(row, &styled, 0);
			}
		} else {
			let visible: String = state.value.chars().skip(state.scroll).take(width).collect();
			buf.set_string(inner.x, inner.y, &visible, style, row);
		}
		if !focused {
			return;
		}
		match state.selection() {
			// Выделение: видимая часть промежутка; отдельная ячейка курсора не рисуется.
			Some((start, end)) => {
				let first = start.max(state.scroll);
				let last = end.min(state.scroll + width);
				for position in first..last {
					let x = inner.x + (position - state.scroll) as u16;
					match self.selection_style {
						Some(selection_style) => {
							buf.set_style(Rect::new(x, inner.y, 1, 1), selection_style);
						}
						None => swap_colors(buf, x, inner.y),
					}
				}
			}
			None => {
				let cursor_x = inner.x + (state.cursor - state.scroll) as u16;
				if cursor_x < inner.right() {
					swap_colors(buf, cursor_x, inner.y);
				}
			}
		}
	}
}
