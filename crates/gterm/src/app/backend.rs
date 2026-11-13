//! Состояние активного бэкенда: [`Backend`] и [`SoftState`] с выводом Software в окно (ADR-0010).

use std::num::NonZeroU32;
use std::sync::Arc;

use softbuffer::{Rect, Surface};
use winit::window::Window;

use super::AppError;
use crate::rasterizer::{BoxDrawingMode, Rasterizer};
use crate::renderer::gl_backend::GlBackend;
use crate::renderer::pixel::copy_opaque;
use crate::renderer::soft_backend::SoftBackend;
use crate::renderer::{Renderer, RendererKind};

/// Software-бэкенд вместе с поверхностью вывода.
pub(super) struct SoftState {
	pub(super) renderer: SoftBackend,
	pub(super) surface: Surface<Arc<Window>, Arc<Window>>,
	/// Прямоугольники частичного вывода; переиспользуются (NFR-6, ADR-0010).
	pub(super) damage_rects: Vec<Rect>,
	/// Окно на Wayland: в буфер окна пиксели идут с непрозрачным старшим байтом (ADR-0024).
	pub(super) is_wayland: bool,
}

/// Копия пикселей кадра в буфер окна. На X11 и Win32 старший байт игнорируется — прямая копия;
/// на Wayland он выставляется в `0xFF` (ADR-0024).
fn copy_to_window(dst: &mut [u32], src: &[u32], is_wayland: bool) {
	if is_wayland {
		copy_opaque(dst, src);
	} else {
		dst.copy_from_slice(src);
	}
}

impl SoftState {
	/// Вывести кадр в окно (ADR-0010 п.3–6): целиком, если вывод полный по требованию или буфер
	/// окна не хранит прошлый кадр (`age() ≠ 1`); иначе только перерисованные полосы; если ничего
	/// не перерисовано — не выводить вовсе. `debug` (`--debug`) — строка в stderr на каждый показ:
	/// возраст буфера окна, выбранная ветка, размеры буфера и кадра.
	pub(super) fn present(&mut self, is_full: bool, debug: bool) -> Result<(), AppError> {
		let damage = self.renderer.damage();
		if damage.is_empty() && !is_full {
			if debug {
				eprintln!("gterm: present: nothing to show (no damage)");
			}
			return Ok(());
		}
		let is_wayland = self.is_wayland;
		let mut buffer = self.surface.buffer_mut().map_err(AppError::Surface)?;
		let frame = self.renderer.frame();
		let describe = |branch: &str, buffer: &softbuffer::Buffer<'_, Arc<Window>, Arc<Window>>| {
			if debug {
				eprintln!(
					"gterm: present: {branch}; age {}, buffer {}x{} ({} px), frame {}x{} ({} px), bands {}, full {is_full}",
					buffer.age(),
					buffer.width(),
					buffer.height(),
					buffer.len(),
					self.renderer.width(),
					self.renderer.height(),
					frame.len(),
					damage.len(),
				);
			}
		};
		if buffer.len() != frame.len() {
			describe("SIZE MISMATCH — buffer shown without writing", &buffer);
			return buffer.present().map_err(AppError::Surface);
		}
		if is_full || buffer.age() != 1 {
			describe("full copy", &buffer);
			copy_to_window(&mut buffer, frame, is_wayland);
			return buffer.present().map_err(AppError::Surface);
		}
		describe("partial bands", &buffer);
		let width = self.renderer.width() as usize;
		self.damage_rects.clear();
		for band in damage {
			if band.x == 0 && band.width as usize == width {
				// Полоса во всю ширину кадра лежит в памяти сплошным куском.
				let start = band.y as usize * width;
				let end = start + band.height as usize * width;
				copy_to_window(&mut buffer[start..end], &frame[start..end], is_wayland);
			} else {
				for y in band.y..band.y + band.height {
					let start = y as usize * width + band.x as usize;
					let end = start + band.width as usize;
					copy_to_window(&mut buffer[start..end], &frame[start..end], is_wayland);
				}
			}
			if let (Some(w), Some(h)) = (NonZeroU32::new(band.width), NonZeroU32::new(band.height))
			{
				self.damage_rects.push(Rect {
					x: band.x,
					y: band.y,
					width: w,
					height: h,
				});
			}
		}
		buffer
			.present_with_damage(&self.damage_rects)
			.map_err(AppError::Surface)
	}
}

/// Активный бэкенд (в куче: состояния крупные — таблицы функций GL, атлас, поверхность).
pub(super) enum Backend {
	Gl(Box<GlBackend>),
	Soft(Box<SoftState>),
}

impl Backend {
	pub(super) fn renderer_mut(&mut self) -> &mut dyn Renderer {
		match self {
			Backend::Gl(gl) => gl.as_mut(),
			Backend::Soft(soft) => &mut soft.renderer,
		}
	}

	pub(super) fn name(&self) -> &'static str {
		match self {
			Backend::Gl(gl) => gl.name(),
			Backend::Soft(soft) => soft.renderer.name(),
		}
	}

	/// Уровень цепочки, представленный этим бэкендом.
	pub(super) fn kind(&self) -> RendererKind {
		match self {
			Backend::Gl(gl) => gl.info().kind,
			Backend::Soft(_) => RendererKind::Software,
		}
	}

	/// Забрать растеризатор для следующего уровня цепочки или отката (FR-21).
	pub(super) fn into_rasterizer(self) -> Rasterizer {
		match self {
			Backend::Gl(gl) => gl.into_atlas().into_rasterizer(),
			// `SoftBackend` владеет атласом, поэтому он вынимается целиком.
			Backend::Soft(soft) => soft.renderer.into_atlas().into_rasterizer(),
		}
	}

	/// Сменить физический размер растеризации на месте (FR-24, ADR-0005 п.7).
	pub(super) fn set_size(&mut self, size_px: u16) -> Result<(), AppError> {
		match self {
			Backend::Gl(gl) => gl.set_size(size_px).map_err(AppError::Render),
			Backend::Soft(soft) => soft.renderer.set_size(size_px).map_err(AppError::Render),
		}
	}

	/// Сменить источник псевдографики на лету (ADR-0018).
	pub(super) fn set_box_drawing_mode(&mut self, mode: BoxDrawingMode) {
		match self {
			Backend::Gl(gl) => gl.set_box_drawing_mode(mode),
			Backend::Soft(soft) => soft.renderer.set_box_drawing_mode(mode),
		}
	}
}
