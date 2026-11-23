//! Индикаторы доли (ADR-0023, часть 2): [`Gauge`] — полоса на всю область с подписью по
//! центру, [`LineGauge`] — подпись и линия в одну строку. Доля — число от 0 до 1; значения вне
//! отрезка обрезаются, «не число» считается нулём.

use gterm::buffer::ScreenBuffer;

use crate::buffer::BufferExt;
use crate::geometry::Rect;
use crate::style::Style;
use crate::text::{Alignment, Line, StyledLine};
use crate::theme::{Theme, fg_only, scope};
use crate::widgets::{Block, Widget};

/// Полностью залитая ячейка.
const FULL_BLOCK: char = '█';
/// Кодовая точка сразу за левыми блоками-восьмушками: `0x2590 - n` — блок шириной `n/8`
/// (U+2589 — 7/8 … U+258F — 1/8). Все они рисуются процедурно (ADR-0022) и от шрифта не зависят.
const EIGHTHS_END: u32 = 0x2590;

pub(super) fn clamp_ratio(ratio: f64) -> f64 {
	if ratio.is_nan() {
		0.0
	} else {
		ratio.clamp(0.0, 1.0)
	}
}

/// Подпись по умолчанию — доля в процентах.
fn percent_label(ratio: f64) -> String {
	format!("{}%", (ratio * 100.0).round())
}

/// Область внутри блока; сам блок при этом рисуется.
pub(super) fn render_block(block: Option<&Block<'_>>, area: Rect, buf: &mut ScreenBuffer) -> Rect {
	match block {
		Some(block) => {
			let inner = block.inner(area);
			block.clone().render(area, buf);
			inner
		}
		None => area,
	}
}

/// Области заливки индикаторов в теме (ADR-0037).
const FILLED_SCOPES: [&str; 2] = ["ui.gauge.filled", scope::MENU_SCROLL];

/// Полоса-индикатор: заливка слева направо на всю высоту области, подпись по центру.
#[derive(Clone, Debug)]
pub struct Gauge<'a> {
	block: Option<Block<'a>>,
	ratio: f64,
	label: Option<Line<'a>>,
	style: Style,
	gauge_style: Style,
	use_unicode: bool,
}

impl Default for Gauge<'_> {
	fn default() -> Self {
		Self {
			block: None,
			ratio: 0.0,
			label: None,
			style: Style::new(),
			gauge_style: Style::new(),
			use_unicode: true,
		}
	}
}

impl<'a> Gauge<'a> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn block(mut self, block: Block<'a>) -> Self {
		self.block = Some(block);
		self
	}

	/// Доля от 0 до 1.
	pub fn ratio(mut self, ratio: f64) -> Self {
		self.ratio = clamp_ratio(ratio);
		self
	}

	/// Доля в процентах, 0…100.
	pub fn percent(self, percent: u16) -> Self {
		self.ratio(f64::from(percent.min(100)) / 100.0)
	}

	/// Подпись по центру; без неё — доля в процентах. Пустая строка убирает подпись.
	pub fn label<T: Into<Line<'a>>>(mut self, label: T) -> Self {
		self.label = Some(label.into());
		self
	}

	/// Стиль всей области индикатора.
	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	/// Стиль полосы: `fg` — цвет заливки, `bg` — фон незалитой части.
	pub fn gauge_style(mut self, style: Style) -> Self {
		self.gauge_style = style;
		self
	}

	/// Дробная часть ячейки блоками-восьмушками (по умолчанию включено); без неё заливка
	/// округляется до целой ячейки.
	pub fn use_unicode(mut self, use_unicode: bool) -> Self {
		self.use_unicode = use_unicode;
		self
	}

	/// Подставить стили темы под заданные явно (ADR-0037) — для виджета вне дерева `Ui`; в
	/// дереве то же делает [`Ui::theme`](crate::ui::Ui::theme).
	pub fn themed(mut self, theme: &Theme) -> Self {
		self.set_theme(theme);
		self
	}

	/// Области `ui.gauge.*`; в теме без них заливка — полоса прокрутки меню helix
	/// (`ui.menu.scroll`: символ — заливка, фон — незалитая часть).
	pub(crate) fn set_theme(&mut self, theme: &Theme) {
		self.style = theme.first(&["ui.gauge"]).patch(self.style);
		self.gauge_style = theme.first(&FILLED_SCOPES).patch(self.gauge_style);
		if let Some(block) = &mut self.block {
			block.set_theme(theme);
		}
	}

	/// Сколько ячеек залито полностью и сколько восьмых у следующей за ними (0 — её нет).
	fn filled(&self, width: u16) -> (u16, u32) {
		let width = f64::from(width);
		if self.use_unicode {
			let eighths = (self.ratio * width * 8.0).round() as u32;
			((eighths / 8) as u16, eighths % 8)
		} else {
			((self.ratio * width).round() as u16, 0)
		}
	}
}

impl Widget for Gauge<'_> {
	fn apply_theme(&mut self, theme: &Theme) {
		self.set_theme(theme);
	}

	fn render(self, area: Rect, buf: &mut ScreenBuffer) {
		let area = area.intersection(buf.area());
		if area.is_empty() {
			return;
		}
		buf.set_style(area, self.style);
		let inner = render_block(self.block.as_ref(), area, buf);
		if inner.is_empty() {
			return;
		}
		buf.fill(inner, ' ', self.gauge_style);
		let (full, eighths) = self.filled(inner.width);
		for y in inner.top()..inner.bottom() {
			buf.fill(Rect::new(inner.x, y, full, 1), FULL_BLOCK, self.gauge_style);
			if eighths > 0 {
				if let Some(partial) = char::from_u32(EIGHTHS_END - eighths) {
					buf.set_styled(inner.x + full, y, partial, self.gauge_style, inner);
				}
			}
		}

		let label = match &self.label {
			Some(label) => label.clone(),
			None => Line::raw(percent_label(self.ratio)),
		};
		let chars: Vec<_> = label.styled_chars(Style::new()).collect();
		let visible = chars.len().min(usize::from(inner.width));
		let start = inner.x + (inner.width - visible as u16) / 2;
		let y = inner.y + inner.height / 2;
		for (i, &(ch, style)) in chars.iter().take(visible).enumerate() {
			let x = start + i as u16;
			// Над заливкой символ рисуется цветом фона по цвету заливки, иначе он не виден.
			let style = if x - inner.x < full {
				let cell = buf.cell(usize::from(x), usize::from(y));
				style.fg(cell.bg).bg(cell.fg)
			} else {
				style
			};
			buf.set_styled(x, y, ch, style, inner);
		}
	}
}

/// Индикатор в одну строку: подпись, пробел и линия, заполненная на долю.
#[derive(Clone, Debug)]
pub struct LineGauge<'a> {
	block: Option<Block<'a>>,
	ratio: f64,
	label: Option<Line<'a>>,
	style: Style,
	filled_style: Style,
	unfilled_style: Style,
	filled_symbol: char,
	unfilled_symbol: char,
}

impl Default for LineGauge<'_> {
	fn default() -> Self {
		Self {
			block: None,
			ratio: 0.0,
			label: None,
			style: Style::new(),
			filled_style: Style::new(),
			unfilled_style: Style::new(),
			filled_symbol: '━',
			unfilled_symbol: '─',
		}
	}
}

impl<'a> LineGauge<'a> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn block(mut self, block: Block<'a>) -> Self {
		self.block = Some(block);
		self
	}

	/// Доля от 0 до 1.
	pub fn ratio(mut self, ratio: f64) -> Self {
		self.ratio = clamp_ratio(ratio);
		self
	}

	/// Подпись слева от линии; без неё — доля в процентах.
	pub fn label<T: Into<Line<'a>>>(mut self, label: T) -> Self {
		self.label = Some(label.into());
		self
	}

	/// Стиль всей области индикатора.
	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	pub fn filled_style(mut self, style: Style) -> Self {
		self.filled_style = style;
		self
	}

	pub fn unfilled_style(mut self, style: Style) -> Self {
		self.unfilled_style = style;
		self
	}

	/// Знаки заполненной и незаполненной части линии (по умолчанию `━` и `─`).
	pub fn symbols(mut self, filled: char, unfilled: char) -> Self {
		self.filled_symbol = filled;
		self.unfilled_symbol = unfilled;
		self
	}

	/// Подставить стили темы под заданные явно (ADR-0037) — для виджета вне дерева `Ui`; в
	/// дереве то же делает [`Ui::theme`](crate::ui::Ui::theme).
	pub fn themed(mut self, theme: &Theme) -> Self {
		self.set_theme(theme);
		self
	}

	/// Те же области, что у [`Gauge`]; у линии берётся только цвет символа.
	pub(crate) fn set_theme(&mut self, theme: &Theme) {
		self.style = theme.first(&["ui.gauge"]).patch(self.style);
		self.filled_style = fg_only(theme.first(&FILLED_SCOPES)).patch(self.filled_style);
		self.unfilled_style =
			fg_only(theme.first(&["ui.gauge.unfilled", scope::WINDOW])).patch(self.unfilled_style);
		if let Some(block) = &mut self.block {
			block.set_theme(theme);
		}
	}
}

impl Widget for LineGauge<'_> {
	fn apply_theme(&mut self, theme: &Theme) {
		self.set_theme(theme);
	}

	fn render(self, area: Rect, buf: &mut ScreenBuffer) {
		let area = area.intersection(buf.area());
		if area.is_empty() {
			return;
		}
		buf.set_style(area, self.style);
		let inner = render_block(self.block.as_ref(), area, buf);
		if inner.is_empty() {
			return;
		}
		let row = Rect::new(inner.x, inner.y, inner.width, 1);
		let label = match &self.label {
			Some(label) => label.clone(),
			None => Line::raw(percent_label(self.ratio)),
		};
		let styled = StyledLine {
			chars: label.styled_chars(Style::new()).collect(),
			alignment: Alignment::Left,
		};
		buf.set_line(row, &styled, 0);

		// Линия — после подписи и одной ячейки отступа, в оставшейся ширине.
		let label_width = styled.chars.len().min(usize::from(row.width)) as u16;
		let offset = label_width.saturating_add(1);
		if offset >= row.width {
			return;
		}
		let line = Rect::new(row.x + offset, row.y, row.width - offset, 1);
		let filled = (self.ratio * f64::from(line.width)).round() as u16;
		buf.fill(
			Rect::new(line.x, line.y, filled, 1),
			self.filled_symbol,
			self.filled_style,
		);
		buf.fill(
			Rect::new(line.x + filled, line.y, line.width - filled, 1),
			self.unfilled_symbol,
			self.unfilled_style,
		);
	}
}
