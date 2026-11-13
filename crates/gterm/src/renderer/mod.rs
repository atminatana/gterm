/// Трейты и типы для рендеринга.
use crate::buffer::ScreenBuffer;
use crate::overlay::Overlay;

/// CPU-рендерер (fallback, эталон алгоритма §7.3).
pub mod soft_backend;

/// Попиксельная арифметика Software-рендерера (ADR-0015).
pub mod pixel;

/// Отрисовка полос строк Software-рендерера (ADR-0016).
mod soft_band;

/// OpenGL 3.3 core / GLES 3.0 бэкенд (единственный модуль с `unsafe`, NFR-9).
pub mod gl_backend;

/// Уровень цепочки бэкендов (FR-17): GL 3.3 core → GLES 3.0 → Software.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RendererKind {
	OpenGl33,
	Gles30,
	Software,
}

impl RendererKind {
	/// Имя уровня для лога и заголовка (FR-20).
	pub const fn name(self) -> &'static str {
		match self {
			Self::OpenGl33 => "OpenGL 3.3",
			Self::Gles30 => "OpenGL ES 3.0",
			Self::Software => "Software",
		}
	}

	pub const fn is_gl(self) -> bool {
		!matches!(self, Self::Software)
	}
}

/// Ошибка рендерера.
#[derive(Debug)]
pub enum RenderError {
	/// Контекст потерян, нужен откат.
	ContextLost,
	/// Другая ошибка рендеринга.
	Other(String),
}

impl std::fmt::Display for RenderError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::ContextLost => write!(f, "context lost, fallback needed"),
			Self::Other(s) => write!(f, "render failed: {s}"),
		}
	}
}

impl std::error::Error for RenderError {}

/// Трейт рендерера. Реализуется для GL и Software бэкендов.
pub trait Renderer {
	/// Имя бэкенда.
	fn name(&self) -> &'static str;

	/// Изменить размер окна (в пикселях). Пересоздаёт кадровый буфер.
	fn resize(&mut self, width_px: u32, height_px: u32) -> Result<(), RenderError>;

	/// Отрендерить буфер экрана в кадр.
	fn render(&mut self, screen: &mut ScreenBuffer) -> Result<(), RenderError> {
		self.render_overlaid(screen, &Overlay::new())
	}

	/// Отрендерить буфер экрана и поверх него записи наложения в порядке записи (ADR-0014).
	fn render_overlaid(
		&mut self,
		screen: &mut ScreenBuffer,
		overlay: &Overlay,
	) -> Result<(), RenderError>;
}

/// Результат рендеринга — сырой кадр в формате `0x00RRGGBB`.
pub type Frame = Vec<u32>;
