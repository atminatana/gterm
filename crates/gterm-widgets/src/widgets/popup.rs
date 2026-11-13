//! Всплывающее окно поверх экрана через наложение (ADR-0014): прямоугольник на основной сетке и
//! отдельный прозрачный буфер, в который приложение рисует виджеты; фон окна — цвет с альфой,
//! поэтому экран под ним просвечивает. Рисуется в `AppHandler::on_redraw` через
//! [`DrawCtx::draw_buffer`].

use gterm::app::DrawCtx;
use gterm::buffer::ScreenBuffer;
use gterm::cell::{CellFlags, Color};

use crate::geometry::Rect;
use crate::style::Style;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Popup {
	/// Положение и размер окна на основной сетке.
	pub area: Rect,
	/// Стиль фона окна: `bg` с альфой — полупрозрачное окно, `fg` — цвет текста по умолчанию.
	pub style: Style,
}

impl Popup {
	pub fn new(area: Rect) -> Self {
		Self {
			area,
			style: Style::new(),
		}
	}

	/// Окно `width × height`, отцентрованное на сетке `cols × rows`.
	pub fn centered(cols: u16, rows: u16, width: u16, height: u16) -> Self {
		Self::new(Rect::new(0, 0, cols, rows).centered(width, height))
	}

	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	/// Область для виджетов внутри буфера окна (начало в нуле).
	pub fn local_area(&self) -> Rect {
		Rect::new(0, 0, self.area.width, self.area.height)
	}

	/// Новый буфер окна: прозрачный, залитый фоном стиля (по умолчанию — полностью прозрачный).
	pub fn buffer(&self) -> ScreenBuffer {
		let fg = self.style.fg.unwrap_or(Color::WHITE);
		let bg = self.style.bg.unwrap_or(Color::TRANSPARENT);
		let mut buffer = ScreenBuffer::with_default_colors(
			usize::from(self.area.width),
			usize::from(self.area.height),
			fg,
			bg,
		);
		// Подложка буфера — прозрачная: в наложении видны только ячейки с их альфой.
		buffer.set_background(Color::TRANSPARENT);
		buffer.fill_rect(
			0,
			0,
			usize::from(self.area.width),
			usize::from(self.area.height),
			'\0',
			fg,
			bg,
			CellFlags::empty(),
		);
		buffer
	}

	/// Положить буфер окна поверх экрана в этом кадре.
	pub fn draw(&self, ctx: &mut DrawCtx<'_>, buffer: &ScreenBuffer) {
		ctx.draw_buffer(buffer, i32::from(self.area.x), i32::from(self.area.y));
	}
}
