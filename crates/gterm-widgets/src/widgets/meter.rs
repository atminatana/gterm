//! Индикатор уровня (ADR-0034): [`Meter`] — горизонтальная полоса с цветовыми зонами и отметкой
//! пика. В отличие от [`Gauge`](super::Gauge) цвет ячейки задаёт её место на шкале, а не
//! текущий уровень: заполненная часть многоцветна, как у измерителей громкости.
//!
//! Уровень и пик — числа от 0 до 1; значения вне отрезка обрезаются, «не число» считается
//! нулём. Удержание пика и спад уровня во времени считает приложение: виджет ничего не хранит.

use gterm::buffer::ScreenBuffer;

use super::gauge::{clamp_ratio, render_block};
use crate::buffer::BufferExt;
use crate::geometry::Rect;
use crate::style::Style;
use crate::widgets::{Block, Widget};

/// Полностью заполненная ячейка.
const FULL_BLOCK: char = '█';
/// Левая половина ячейки: уровень рисуется с точностью до половины ячейки.
const LEFT_HALF_BLOCK: char = '▌';
/// Незаполненная ячейка. Все три знака рисуются процедурно (ADR-0022) и от шрифта не зависят.
const LIGHT_SHADE: char = '░';

/// Полоса уровня на всю область: слева направо, на всю высоту.
#[derive(Clone, Debug)]
pub struct Meter<'a> {
	block: Option<Block<'a>>,
	ratio: f64,
	peak: Option<f64>,
	zones: &'a [(f64, Style)],
	style: Style,
	filled_style: Style,
	unfilled_style: Style,
	filled_symbol: char,
	half_symbol: char,
	unfilled_symbol: char,
	peak_symbol: char,
}

impl Default for Meter<'_> {
	fn default() -> Self {
		Self {
			block: None,
			ratio: 0.0,
			peak: None,
			zones: &[],
			style: Style::new(),
			filled_style: Style::new(),
			unfilled_style: Style::new(),
			filled_symbol: FULL_BLOCK,
			half_symbol: LEFT_HALF_BLOCK,
			unfilled_symbol: LIGHT_SHADE,
			peak_symbol: FULL_BLOCK,
		}
	}
}

impl<'a> Meter<'a> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn block(mut self, block: Block<'a>) -> Self {
		self.block = Some(block);
		self
	}

	/// Уровень от 0 до 1.
	pub fn ratio(mut self, ratio: f64) -> Self {
		self.ratio = clamp_ratio(ratio);
		self
	}

	/// Отметка пика от 0 до 1: одна ячейка [`peak_symbol`](Self::peak_symbol) в стиле своей
	/// зоны, в том числе правее заполненной части. Нулевой пик не рисуется.
	pub fn peak(mut self, peak: Option<f64>) -> Self {
		self.peak = peak.map(clamp_ratio);
		self
	}

	/// Цветовые зоны: пары «порог от 0 до 1, стиль» по возрастанию порога. Зона действует от
	/// своего порога до следующего; её стиль накладывается на
	/// [`filled_style`](Self::filled_style) ячеек, чей левый край лежит в зоне.
	pub fn zones(mut self, zones: &'a [(f64, Style)]) -> Self {
		self.zones = zones;
		self
	}

	/// Стиль всей области индикатора.
	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	/// Стиль заполненных ячеек до наложения зоны.
	pub fn filled_style(mut self, style: Style) -> Self {
		self.filled_style = style;
		self
	}

	/// Стиль незаполненных ячеек; его фон — и у правой половины «половинной» ячейки.
	pub fn unfilled_style(mut self, style: Style) -> Self {
		self.unfilled_style = style;
		self
	}

	pub fn filled_symbol(mut self, symbol: char) -> Self {
		self.filled_symbol = symbol;
		self
	}

	/// Знак ячейки, заполненной наполовину.
	pub fn half_symbol(mut self, symbol: char) -> Self {
		self.half_symbol = symbol;
		self
	}

	pub fn unfilled_symbol(mut self, symbol: char) -> Self {
		self.unfilled_symbol = symbol;
		self
	}

	pub fn peak_symbol(mut self, symbol: char) -> Self {
		self.peak_symbol = symbol;
		self
	}

	/// Сколько ячеек заполнено целиком и заполнена ли следующая за ними наполовину.
	fn filled(&self, width: u16) -> (u16, bool) {
		let halves = (self.ratio * f64::from(width) * 2.0).round() as u32;
		((halves / 2) as u16, halves % 2 == 1)
	}

	/// Ячейка, в которую попадает пик: та, чей отрезок шкалы его содержит.
	fn peak_cell(&self, width: u16) -> Option<u16> {
		let peak = self.peak.filter(|&peak| peak > 0.0)?;
		let cells = (peak * f64::from(width)).ceil() as u16;
		Some(cells.clamp(1, width) - 1)
	}

	/// Стиль заполненной ячейки `cell` из `width`: зона выбирается по левому краю ячейки.
	fn lit_style(&self, cell: u16, width: u16) -> Style {
		let position = f64::from(cell) / f64::from(width);
		let zone = self
			.zones
			.iter()
			.rev()
			.find(|(threshold, _)| *threshold <= position);
		match zone {
			Some((_, style)) => self.filled_style.patch(*style),
			None => self.filled_style,
		}
	}
}

impl Widget for Meter<'_> {
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
		let (full, has_half) = self.filled(inner.width);
		let peak_cell = self.peak_cell(inner.width);
		for cell in 0..inner.width {
			let lit = self.lit_style(cell, inner.width);
			let (symbol, style) = if peak_cell == Some(cell) {
				(self.peak_symbol, lit)
			} else if cell < full {
				(self.filled_symbol, lit)
			} else if cell == full && has_half {
				// Левая половина — цветом зоны, правая — фоном незаполненной части.
				let mut style = self.unfilled_style;
				style.fg = lit.fg.or(style.fg);
				(self.half_symbol, style)
			} else {
				(self.unfilled_symbol, self.unfilled_style)
			};
			let column = Rect::new(inner.x + cell, inner.y, 1, inner.height);
			buf.fill(column, symbol, style);
		}
	}
}
