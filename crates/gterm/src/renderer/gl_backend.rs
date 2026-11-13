//! OpenGL 3.3 core / GLES 3.0 бэкенд (ТЗ §6, §7, §9; ADR-0002, ADR-0003).
//!
//! Единственный модуль с `unsafe` (NFR-9): создание контекста/поверхности через `glutin` и вызовы
//! `glow`. Один draw call на кадр — полноэкранный треугольник по `gl_VertexID` (FR-12); вся логика
//! ячеек — во фрагментном шейдере `shaders/cell.frag`, бит-идентичном `soft_backend.rs`.
//! Любая ошибка инициализации — `RenderError` для отката на следующий уровень (NFR-7, FR-18);
//! потеря контекста во время работы — `RenderError::ContextLost` (FR-21).

use std::num::NonZeroU32;
use std::time::{Duration, Instant};

use glow::HasContext;
use glutin::config::{Config, ConfigTemplateBuilder};
use glutin::context::{
	ContextApi, ContextAttributesBuilder, GlProfile, PossiblyCurrentContext, Version,
};
use glutin::display::GetGlDisplay;
use glutin::prelude::*;
use glutin::surface::{Surface, SurfaceAttributesBuilder, SwapInterval, WindowSurface};
use raw_window_handle::HasWindowHandle;

use crate::atlas::{Atlas, CHANNELS, EMPTY_SLOT};
use crate::buffer::ScreenBuffer;
use crate::cell::Style;
use crate::overlay::Overlay;
use crate::rasterizer::{BoxDrawingMode, Rasterizer};
use crate::renderer::{RenderError, Renderer, RendererKind};

const VERT_SRC: &str = include_str!("shaders/cell.vert");
const FRAG_SRC: &str = include_str!("shaders/cell.frag");
/// Проход наложения одной записи (ADR-0014).
const OVERLAY_FRAG_SRC: &str = include_str!("shaders/overlay.frag");
const HEADER_GL: &str = "#version 330 core\n";
const HEADER_GLES: &str =
	"#version 300 es\nprecision highp float;\nprecision highp int;\nprecision highp usampler2D;\n";
/// Байт на тексель текстуры данных (`RGBA32UI`).
const DATA_TEXEL_BYTES: usize = 16;
/// Драйверы, которые считаются отказом GL всегда (§9 п.6): OpenGL 1.1 от Microsoft.
const REJECTED_RENDERERS: [&str; 1] = ["GDI Generic"];
/// Программные GL-драйверы: допускаются при `allow_sw_gl` (§9 п.6).
const SOFTWARE_RENDERERS: [&str; 2] = ["llvmpipe", "softpipe"];
/// `GL_CONTEXT_LOST` (GL 4.5 / KHR_robustness): контекст потерян (FR-21).
const GL_CONTEXT_LOST: u32 = 0x0507;

/// Параметры создания контекста из конфигурации (§9 п.6, §11.4 п.4).
#[derive(Clone, Copy, Debug)]
pub struct GlOptions {
	/// Имитировать отказ создания контекста.
	pub fail_gl: bool,
	/// Разрешать `llvmpipe`/`softpipe`.
	pub allow_sw_gl: bool,
	/// Ждать кадровой синхронизации в `swap_buffers` (`SwapInterval::Wait(1)`). `false` — только
	/// для замера `--bench` (ADR-0006): при включённом VSync ожидание ложится не в `swap_buffers`,
	/// а в первый GL-вызов следующего кадра, и CPU-часть кадра неотличима от периода монитора.
	pub vsync: bool,
}

impl Default for GlOptions {
	fn default() -> Self {
		Self {
			fail_gl: false,
			allow_sw_gl: true,
			vsync: true,
		}
	}
}

/// Сведения о контексте для лога (FR-20, §9 п.7).
#[derive(Clone, Debug)]
pub struct GlInfo {
	pub kind: RendererKind,
	pub vendor: String,
	pub renderer: String,
	pub version: String,
	pub max_texture_size: u32,
}

/// Шаблон конфигурации §9 п.1: 8 бит на канал, без depth/stencil/MSAA, предпочитать аппаратное.
pub fn config_template() -> ConfigTemplateBuilder {
	ConfigTemplateBuilder::new()
		.with_alpha_size(8)
		.with_depth_size(0)
		.with_stencil_size(0)
		.prefer_hardware_accelerated(Some(true))
}

/// Выбор конфигурации из предложенных: аппаратная, без мультисэмплинга.
pub fn pick_config(configs: Box<dyn Iterator<Item = Config> + '_>) -> Config {
	configs
		.reduce(|best, c| {
			let score = |cfg: &Config| {
				(
					cfg.hardware_accelerated() as u8,
					u8::MAX - cfg.num_samples(),
				)
			};
			if score(&c) > score(&best) { c } else { best }
		})
		.expect("glutin always offers at least one config")
}

/// Контекст, поверхность и загрузчик функций. Создание — единственное место с `unsafe` вне `glow`.
pub struct GlContext {
	context: PossiblyCurrentContext,
	surface: Surface<WindowSurface>,
	gl: glow::Context,
	info: GlInfo,
}

impl GlContext {
	/// §9 п.2–7 для одного уровня цепочки: контекст, поверхность, current, VSync, `glow`, проверки.
	pub fn new(
		config: &Config,
		window: &impl HasWindowHandle,
		width: u32,
		height: u32,
		kind: RendererKind,
		options: &GlOptions,
	) -> Result<Self, RenderError> {
		if options.fail_gl {
			return Err(RenderError::Other("forced failure (--fail-gl)".to_owned()));
		}
		let api = match kind {
			RendererKind::OpenGl33 => ContextApi::OpenGl(Some(Version::new(3, 3))),
			RendererKind::Gles30 => ContextApi::Gles(Some(Version::new(3, 0))),
			RendererKind::Software => {
				return Err(RenderError::Other("Software is not a GL level".to_owned()));
			}
		};
		let raw = window
			.window_handle()
			.map_err(|e| RenderError::Other(format!("window handle: {e}")))?
			.as_raw();
		let display = config.display();
		let mut builder = ContextAttributesBuilder::new().with_context_api(api);
		if kind == RendererKind::OpenGl33 {
			builder = builder.with_profile(GlProfile::Core);
		}
		let attributes = builder.build(Some(raw));
		// SAFETY: атрибуты построены из действительного raw-handle живого окна `window`.
		let not_current = unsafe { display.create_context(config, &attributes) }
			.map_err(|e| RenderError::Other(format!("create context: {e}")))?;

		let w = NonZeroU32::new(width.max(1)).expect("non-zero");
		let h = NonZeroU32::new(height.max(1)).expect("non-zero");
		let surface_attributes = SurfaceAttributesBuilder::<WindowSurface>::new().build(raw, w, h);
		// SAFETY: тот же действительный raw-handle; окно живёт дольше поверхности (App хранит Arc).
		let surface = unsafe { display.create_window_surface(config, &surface_attributes) }
			.map_err(|e| RenderError::Other(format!("create surface: {e}")))?;
		let context = not_current
			.make_current(&surface)
			.map_err(|e| RenderError::Other(format!("make current: {e}")))?;
		// VSync: ошибка не критична (§9 п.3, §14). Без ожидания — только замер (ADR-0006).
		let interval = if options.vsync {
			SwapInterval::Wait(NonZeroU32::new(1).expect("1"))
		} else {
			SwapInterval::DontWait
		};
		let _ = surface.set_swap_interval(&context, interval);

		// SAFETY: контекст текущий в этом потоке; загрузчик — get_proc_address этого дисплея.
		let gl = unsafe {
			glow::Context::from_loader_function_cstr(|name| display.get_proc_address(name))
		};
		let info = Self::query_info(&gl, kind, options)?;
		Ok(Self {
			context,
			surface,
			gl,
			info,
		})
	}

	fn query_info(
		gl: &glow::Context,
		kind: RendererKind,
		options: &GlOptions,
	) -> Result<GlInfo, RenderError> {
		// SAFETY: контекст текущий; запросы строк и целых без побочных эффектов.
		let (vendor, renderer, version, max_texture_size) = unsafe {
			(
				gl.get_parameter_string(glow::VENDOR),
				gl.get_parameter_string(glow::RENDERER),
				gl.get_parameter_string(glow::VERSION),
				gl.get_parameter_i32(glow::MAX_TEXTURE_SIZE),
			)
		};
		if REJECTED_RENDERERS.iter().any(|r| renderer.contains(r)) {
			return Err(RenderError::Other(format!(
				"renderer '{renderer}' is not supported"
			)));
		}
		if !options.allow_sw_gl && SOFTWARE_RENDERERS.iter().any(|r| renderer.contains(r)) {
			return Err(RenderError::Other(format!(
				"software renderer '{renderer}' rejected (--allow-sw-gl=false)"
			)));
		}
		if max_texture_size < 1024 {
			return Err(RenderError::Other(format!(
				"GL_MAX_TEXTURE_SIZE {max_texture_size} < 1024"
			)));
		}
		Ok(GlInfo {
			kind,
			vendor,
			renderer,
			version,
			max_texture_size: max_texture_size as u32,
		})
	}

	pub fn info(&self) -> &GlInfo {
		&self.info
	}

	pub fn max_texture_size(&self) -> u32 {
		self.info.max_texture_size
	}

	fn resize_surface(&self, width: u32, height: u32) {
		if let (Some(w), Some(h)) = (NonZeroU32::new(width), NonZeroU32::new(height)) {
			self.surface.resize(&self.context, w, h);
		}
	}

	/// Ошибка обмена буферов трактуется как потеря контекста (FR-21): дальше рисовать нечем.
	fn swap_buffers(&self) -> Result<(), RenderError> {
		self.surface
			.swap_buffers(&self.context)
			.map_err(|_| RenderError::ContextLost)
	}
}

/// Локации uniform'ов §7.2.
struct Uniforms {
	data: Option<glow::UniformLocation>,
	atlas: Option<glow::UniformLocation>,
	grid: Option<glow::UniformLocation>,
	glyph: Option<glow::UniformLocation>,
	origin: Option<glow::UniformLocation>,
	background: Option<glow::UniformLocation>,
	viewport_h: Option<glow::UniformLocation>,
	atlas_cols: Option<glow::UniformLocation>,
	style_channel: Option<glow::UniformLocation>,
	/// Текстура наложения (ADR-0014) и признак, что она есть в этом кадре.
	overlay: Option<glow::UniformLocation>,
	has_overlay: Option<glow::UniformLocation>,
}

/// Uniform-переменные прохода наложения (`overlay.frag`).
struct OverlayUniforms {
	prev: Option<glow::UniformLocation>,
	layer: Option<glow::UniformLocation>,
	atlas: Option<glow::UniformLocation>,
	layer_origin: Option<glow::UniformLocation>,
	layer_size: Option<glow::UniformLocation>,
	glyph: Option<glow::UniformLocation>,
	atlas_cols: Option<glow::UniformLocation>,
	style_channel: Option<glow::UniformLocation>,
}

/// Объекты наложения (ADR-0014): программа, две текстуры `RGBA8UI` размером с сетку с FBO к
/// каждой (проходы пишут попеременно, ping-pong) и текстура данных записи `RGBA32UI`.
/// Создаются при первом кадре с наложением, пересоздаются при смене размера сетки.
struct OverlayGl {
	program: glow::Program,
	uniforms: OverlayUniforms,
	textures: [glow::Texture; 2],
	fbos: [glow::Framebuffer; 2],
	/// Размер текстур наложения в пикселях — сетка.
	size: (u32, u32),
	layer_texture: glow::Texture,
	/// Размер текстуры данных записи в ячейках; растёт до наибольшей записи.
	layer_size: (usize, usize),
}

/// Объекты GL вместе с контекстом; освобождаются в `Drop`. Отдельно от `GlBackend`, чтобы
/// атлас можно было забрать из бэкенда при откате (FR-21).
struct GlResources {
	ctx: GlContext,
	program: glow::Program,
	vao: glow::VertexArray,
	atlas_texture: glow::Texture,
	data_texture: glow::Texture,
	/// FBO + текстура для `render_to_frame`; создаются по требованию.
	offscreen: Option<(glow::Framebuffer, glow::Texture)>,
	/// Объекты наложения; создаются по требованию.
	overlay: Option<OverlayGl>,
}

impl Drop for GlResources {
	fn drop(&mut self) {
		let gl = &self.ctx.gl;
		// SAFETY: контекст ещё жив (поле `ctx` уничтожается после этого); объекты созданы нами.
		unsafe {
			if let Some((fbo, tex)) = self.offscreen.take() {
				gl.delete_framebuffer(fbo);
				gl.delete_texture(tex);
			}
			if let Some(overlay) = self.overlay.take() {
				delete_overlay(gl, overlay);
			}
			gl.delete_texture(self.data_texture);
			gl.delete_texture(self.atlas_texture);
			gl.delete_vertex_array(self.vao);
			gl.delete_program(self.program);
		}
	}
}

/// Куда рисовать: экран или FBO для чтения кадра (тесты §11.2).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
	Screen,
	Offscreen,
}

/// GL-бэкенд: текстуры данных и атласа, программа, один draw call.
pub struct GlBackend {
	res: GlResources,
	atlas: Atlas,
	uniforms: Uniforms,
	width: u32,
	height: u32,
	/// Размер сетки, под который создана текстура данных.
	grid: (usize, usize),
	/// Буфер строки данных `16 × cols` байт (переиспользуется, NFR-6).
	data_staging: Vec<u8>,
	/// Буфер строки данных записи наложения (переиспользуется, NFR-6).
	layer_staging: Vec<u8>,
	/// Буфер одного слота атласа `glyph_w × glyph_h × 4` байт.
	slot_staging: Vec<u8>,
	/// Кадр после `render_to_frame`.
	frame: Vec<u32>,
	read_staging: Vec<u8>,
	atlas_resets_seen: u32,
	needs_full_upload: bool,
	first_frame_checked: bool,
	/// Длительность CPU-части последнего `render` — без `swap_buffers` (ADR-0006, NFR-4).
	last_cpu_time: Duration,
}

impl GlBackend {
	/// Программа, VAO, текстуры; при ошибке атлас возвращается для следующего уровня.
	pub fn new(ctx: GlContext, atlas: Atlas) -> Result<Self, (RenderError, Box<Atlas>)> {
		let (program, vao, atlas_texture, data_texture, uniforms) = match Self::init(&ctx, &atlas) {
			Ok(objects) => objects,
			Err(e) => return Err((e, Box::new(atlas))),
		};
		let res = GlResources {
			ctx,
			program,
			vao,
			atlas_texture,
			data_texture,
			offscreen: None,
			overlay: None,
		};
		if let Err(e) = upload_atlas_page(&res.ctx.gl, res.atlas_texture, &atlas) {
			return Err((e, Box::new(atlas))); // `res` освободит объекты в Drop
		}
		let slot_bytes = atlas.glyph_w() * atlas.glyph_h() * CHANNELS;
		Ok(Self {
			atlas_resets_seen: atlas.resets(),
			res,
			atlas,
			uniforms,
			width: 0,
			height: 0,
			grid: (0, 0),
			data_staging: Vec::new(),
			layer_staging: Vec::new(),
			slot_staging: vec![0; slot_bytes],
			frame: Vec::new(),
			read_staging: Vec::new(),
			needs_full_upload: true,
			first_frame_checked: false,
			last_cpu_time: Duration::ZERO,
		})
	}

	/// CPU-часть последнего кадра: сборка данных, загрузка текстур, draw call — без ожидания
	/// VSync в `swap_buffers` (ADR-0006; это и есть величина NFR-4).
	pub fn last_cpu_time(&self) -> Duration {
		self.last_cpu_time
	}

	pub fn info(&self) -> &GlInfo {
		self.res.ctx.info()
	}

	pub fn atlas(&self) -> &Atlas {
		&self.atlas
	}

	/// Забрать атлас (с растеризатором) при откате на другой бэкенд; объекты GL освобождаются.
	pub fn into_atlas(self) -> Atlas {
		self.atlas
	}

	/// Кадр после `render_to_frame` (`0x00RRGGBB`, строки сверху вниз).
	pub fn frame(&self) -> &[u32] {
		&self.frame
	}

	/// Размер сетки в пикселях окна: ячейка из метрик растеризации, масштаба нет (ADR-0005).
	pub fn grid_size_px(&self, screen: &ScreenBuffer) -> (u32, u32) {
		(
			screen.cols() as u32 * self.atlas.glyph_w() as u32,
			screen.rows() as u32 * self.atlas.glyph_h() as u32,
		)
	}

	/// Пересобрать атлас под новый размер растеризации (ADR-0005 п.7): страница текстуры та же,
	/// поэтому перезаливается она целиком по счётчику сбросов атласа; буфер слота — нового размера.
	pub fn set_rasterizer(&mut self, rasterizer: Rasterizer) {
		self.atlas.set_rasterizer(rasterizer);
		self.after_rasterizer_change();
	}

	/// Сменить физический размер растеризации на месте — путь смены DPI (FR-24).
	///
	/// Страница атласа та же, поэтому она перезаливается целиком: счётчик сбросов вырос, а при
	/// меньшем размере глифа часть страницы обнулена и частичная загрузка была бы неверной.
	pub fn set_size(&mut self, size_px: u16) -> Result<(), RenderError> {
		self.atlas
			.set_size(size_px)
			.map_err(|e| RenderError::Other(format!("font size {size_px} px: {e}")))?;
		self.after_rasterizer_change();
		Ok(())
	}

	/// Источник псевдографики (ADR-0018): страница атласа сброшена — полная перезаливка, как
	/// после смены размера.
	pub fn set_box_drawing_mode(&mut self, mode: BoxDrawingMode) {
		if self.atlas.box_drawing_mode() == mode {
			return;
		}
		self.atlas.set_box_drawing_mode(mode);
		self.after_rasterizer_change();
	}

	/// Общая часть смены растеризатора: буфер слота нового размера и полная перезаливка.
	fn after_rasterizer_change(&mut self) {
		let slot_bytes = self.atlas.glyph_w() * self.atlas.glyph_h() * CHANNELS;
		self.slot_staging.clear();
		self.slot_staging.resize(slot_bytes, 0);
		self.needs_full_upload = true;
	}

	#[allow(clippy::type_complexity)]
	fn init(
		ctx: &GlContext,
		atlas: &Atlas,
	) -> Result<
		(
			glow::Program,
			glow::VertexArray,
			glow::Texture,
			glow::Texture,
			Uniforms,
		),
		RenderError,
	> {
		let gl = &ctx.gl;
		let header = match ctx.info.kind {
			RendererKind::Gles30 => HEADER_GLES,
			_ => HEADER_GL,
		};
		if atlas.page_px() as u32 > ctx.info.max_texture_size {
			return Err(RenderError::Other(format!(
				"atlas page {} exceeds GL_MAX_TEXTURE_SIZE {}",
				atlas.page_px(),
				ctx.info.max_texture_size
			)));
		}
		// SAFETY: контекст текущий; все объекты создаются и проверяются здесь, ошибки — через Result.
		unsafe {
			let program = build_program(gl, header, FRAG_SRC)?;
			let vao = gl.create_vertex_array().map_err(RenderError::Other)?;

			let atlas_texture = gl.create_texture().map_err(RenderError::Other)?;
			gl.bind_texture(glow::TEXTURE_2D, Some(atlas_texture));
			set_nearest_clamp(gl);
			gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
			gl.tex_image_2d(
				glow::TEXTURE_2D,
				0,
				glow::RGBA8UI as i32,
				atlas.page_px() as i32,
				atlas.page_px() as i32,
				0,
				glow::RGBA_INTEGER,
				glow::UNSIGNED_BYTE,
				glow::PixelUnpackData::Slice(None),
			);

			let data_texture = gl.create_texture().map_err(RenderError::Other)?;
			gl.bind_texture(glow::TEXTURE_2D, Some(data_texture));
			set_nearest_clamp(gl);
			gl.bind_texture(glow::TEXTURE_2D, None);

			let uniforms = Uniforms {
				data: gl.get_uniform_location(program, "u_data"),
				atlas: gl.get_uniform_location(program, "u_atlas"),
				grid: gl.get_uniform_location(program, "u_grid"),
				glyph: gl.get_uniform_location(program, "u_glyph"),
				origin: gl.get_uniform_location(program, "u_origin"),
				background: gl.get_uniform_location(program, "u_background"),
				viewport_h: gl.get_uniform_location(program, "u_viewport_h"),
				atlas_cols: gl.get_uniform_location(program, "u_atlas_cols"),
				style_channel: gl.get_uniform_location(program, "u_style_channel"),
				overlay: gl.get_uniform_location(program, "u_overlay"),
				has_overlay: gl.get_uniform_location(program, "u_has_overlay"),
			};
			check_gl_error(gl, "init")?;
			Ok((program, vao, atlas_texture, data_texture, uniforms))
		}
	}

	/// Загрузить страницу атласа целиком (после сброса).
	fn upload_atlas_page(&mut self) -> Result<(), RenderError> {
		upload_atlas_page(&self.res.ctx.gl, self.res.atlas_texture, &self.atlas)?;
		self.atlas.take_dirty_slots();
		self.atlas_resets_seen = self.atlas.resets();
		Ok(())
	}

	/// Загрузить только слоты, изменённые с прошлого кадра.
	fn upload_dirty_slots(&mut self) {
		let dirty = self.atlas.take_dirty_slots();
		if dirty.is_empty() {
			return;
		}
		let (gw, gh) = (self.atlas.glyph_w(), self.atlas.glyph_h());
		let page = self.atlas.page_px();
		let gl = &self.res.ctx.gl;
		// SAFETY: контекст текущий; буфер слота ровно gw×gh×4 байт, координаты внутри страницы.
		unsafe {
			gl.bind_texture(glow::TEXTURE_2D, Some(self.res.atlas_texture));
			gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
			for slot in dirty {
				let (ox, oy) = self.atlas.slot_origin(slot);
				let cells = self.atlas.cells();
				for y in 0..gh {
					let src = ((oy + y) * page + ox) * CHANNELS;
					let dst = y * gw * CHANNELS;
					self.slot_staging[dst..dst + gw * CHANNELS]
						.copy_from_slice(&cells[src..src + gw * CHANNELS]);
				}
				gl.tex_sub_image_2d(
					glow::TEXTURE_2D,
					0,
					ox as i32,
					oy as i32,
					gw as i32,
					gh as i32,
					glow::RGBA_INTEGER,
					glow::UNSIGNED_BYTE,
					glow::PixelUnpackData::Slice(Some(&self.slot_staging)),
				);
			}
		}
	}

	/// Промахи кэша для рисуемых строк и всех записей наложения (их видимой части); `true` —
	/// страница сбрасывалась.
	fn prepare_glyphs(&mut self, screen: &ScreenBuffer, overlay: &Overlay, full: bool) -> bool {
		let before = self.atlas.resets();
		for row in 0..screen.rows() {
			if full || screen.is_row_dirty(row) {
				for col in 0..screen.cols() {
					let cell = screen.cell(col, row);
					self.atlas.ensure(cell.ch, cell.style());
				}
			}
		}
		for layer in overlay.layers() {
			for row in layer.grid_rows(screen.rows()) {
				for col in layer.grid_cols(screen.cols()) {
					let local_col = (col as i64 - i64::from(layer.col)) as usize;
					let local_row = (row as i64 - i64::from(layer.row)) as usize;
					let cell = layer.cell(local_col, local_row);
					self.atlas.ensure(cell.ch, cell.style());
				}
			}
		}
		self.atlas.resets() != before
	}

	/// Пересоздать текстуру данных под сетку (§6.2).
	fn recreate_data_texture(&mut self, cols: usize, rows: usize) -> Result<(), RenderError> {
		let max = self.res.ctx.info.max_texture_size;
		if cols as u32 > max || rows as u32 > max {
			return Err(RenderError::Other(format!(
				"grid {cols}x{rows} exceeds GL_MAX_TEXTURE_SIZE {max}"
			)));
		}
		let gl = &self.res.ctx.gl;
		// SAFETY: контекст текущий; данные не передаются (NULL), размеры проверены.
		unsafe {
			gl.bind_texture(glow::TEXTURE_2D, Some(self.res.data_texture));
			gl.tex_image_2d(
				glow::TEXTURE_2D,
				0,
				glow::RGBA32UI as i32,
				cols.max(1) as i32,
				rows.max(1) as i32,
				0,
				glow::RGBA_INTEGER,
				glow::UNSIGNED_INT,
				glow::PixelUnpackData::Slice(None),
			);
		}
		self.grid = (cols, rows);
		self.data_staging.clear();
		self.data_staging.resize(cols * DATA_TEXEL_BYTES, 0);
		self.needs_full_upload = true;
		check_gl_error(gl, "data texture")
	}

	/// Загрузить строки данных (§6.2): dirty либо все.
	fn upload_data(&mut self, screen: &ScreenBuffer, full: bool) {
		let cols = screen.cols();
		let gl = &self.res.ctx.gl;
		// SAFETY: контекст текущий; буфер строки ровно 16 × cols байт.
		unsafe {
			gl.bind_texture(glow::TEXTURE_2D, Some(self.res.data_texture));
			gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 4);
			for row in 0..screen.rows() {
				if !(full || screen.is_row_dirty(row)) {
					continue;
				}
				for col in 0..cols {
					let packed = pack_cell(&self.atlas, screen.cell(col, row));
					write_texel(&mut self.data_staging, col, &packed);
				}
				gl.tex_sub_image_2d(
					glow::TEXTURE_2D,
					0,
					0,
					row as i32,
					cols as i32,
					1,
					glow::RGBA_INTEGER,
					glow::UNSIGNED_INT,
					glow::PixelUnpackData::Slice(Some(&self.data_staging)),
				);
			}
		}
	}

	fn draw(
		&mut self,
		screen: &ScreenBuffer,
		target: Target,
		overlay: Option<glow::Texture>,
	) -> Result<(), RenderError> {
		let (grid_w, grid_h) = self.grid_size_px(screen);
		let origin = (
			self.width.saturating_sub(grid_w) / 2,
			self.height.saturating_sub(grid_h) / 2,
		);
		let background = screen.background();
		let mut channels = [0i32; 4];
		for (i, &s) in Style::ALL.iter().enumerate() {
			channels[i] = self.atlas.rasterizer().family().style_source(s).channel() as i32;
		}
		let gl = &self.res.ctx.gl;
		let u = &self.uniforms;
		// SAFETY: контекст текущий; программа/VAO/текстуры созданы в `init`.
		unsafe {
			let fbo = match (target, self.res.offscreen) {
				(Target::Offscreen, Some((fbo, _))) => Some(fbo),
				_ => None,
			};
			gl.bind_framebuffer(glow::FRAMEBUFFER, fbo);
			gl.viewport(0, 0, self.width as i32, self.height as i32);
			gl.disable(glow::BLEND);
			gl.disable(glow::DEPTH_TEST);
			gl.disable(glow::SCISSOR_TEST);
			gl.use_program(Some(self.res.program));
			gl.active_texture(glow::TEXTURE0);
			gl.bind_texture(glow::TEXTURE_2D, Some(self.res.data_texture));
			gl.active_texture(glow::TEXTURE1);
			gl.bind_texture(glow::TEXTURE_2D, Some(self.res.atlas_texture));
			// Без наложения на блок 2 привязан атлас: целочисленная текстура того же типа, шейдер
			// её не читает (`u_has_overlay = 0`).
			gl.active_texture(glow::TEXTURE2);
			gl.bind_texture(
				glow::TEXTURE_2D,
				Some(overlay.unwrap_or(self.res.atlas_texture)),
			);
			gl.uniform_1_i32(u.data.as_ref(), 0);
			gl.uniform_1_i32(u.atlas.as_ref(), 1);
			gl.uniform_1_i32(u.overlay.as_ref(), 2);
			gl.uniform_1_i32(u.has_overlay.as_ref(), i32::from(overlay.is_some()));
			gl.uniform_2_i32(u.grid.as_ref(), screen.cols() as i32, screen.rows() as i32);
			gl.uniform_2_i32(
				u.glyph.as_ref(),
				self.atlas.glyph_w() as i32,
				self.atlas.glyph_h() as i32,
			);
			gl.uniform_2_i32(u.origin.as_ref(), origin.0 as i32, origin.1 as i32);
			gl.uniform_3_i32(
				u.background.as_ref(),
				i32::from(background.r()),
				i32::from(background.g()),
				i32::from(background.b()),
			);
			gl.uniform_1_i32(u.viewport_h.as_ref(), self.height as i32);
			gl.uniform_1_i32(
				u.atlas_cols.as_ref(),
				(self.atlas.page_px() / self.atlas.glyph_w()) as i32,
			);
			gl.uniform_4_i32(
				u.style_channel.as_ref(),
				channels[0],
				channels[1],
				channels[2],
				channels[3],
			);
			gl.bind_vertex_array(Some(self.res.vao));
			gl.draw_arrays(glow::TRIANGLES, 0, 3);
			gl.bind_vertex_array(None);
		}
		// FR-18: ошибка `glGetError` при первом кадре — повод для отката; FR-21: потеря контекста — всегда.
		// SAFETY: контекст текущий; запрос без побочных эффектов.
		let code = unsafe { gl.get_error() };
		match code {
			glow::NO_ERROR => {}
			GL_CONTEXT_LOST => return Err(RenderError::ContextLost),
			_ if !self.first_frame_checked => {
				return Err(RenderError::Other(format!(
					"glGetError 0x{code:X} after first frame"
				)));
			}
			_ => {}
		}
		self.first_frame_checked = true;
		Ok(())
	}

	/// Общая часть `render_overlaid` и `render_to_frame_overlaid`: глифы → текстуры → проходы
	/// наложения (по одному на запись, ADR-0014) → основной draw call.
	fn render_into(
		&mut self,
		screen: &mut ScreenBuffer,
		overlay: &Overlay,
		target: Target,
	) -> Result<(), RenderError> {
		if self.atlas.glyph_w() == 0 || self.atlas.glyph_h() == 0 {
			return Err(RenderError::Other("cell size must be >= 1 px".to_owned()));
		}
		if self.width == 0 || self.height == 0 {
			let (w, h) = self.grid_size_px(screen);
			self.resize(w, h)?;
		}
		if self.grid != (screen.cols(), screen.rows()) {
			self.recreate_data_texture(screen.cols(), screen.rows())?;
		}
		let mut full = self.needs_full_upload;
		if self.prepare_glyphs(screen, overlay, full) {
			full = true;
			if self.prepare_glyphs(screen, overlay, true) {
				return Err(RenderError::Other(format!(
					"atlas page {}px holds {} glyphs, screen needs more",
					self.atlas.page_px(),
					self.atlas.capacity()
				)));
			}
		}
		if self.atlas.resets() != self.atlas_resets_seen {
			self.upload_atlas_page()?;
			full = true;
		} else {
			self.upload_dirty_slots();
		}
		self.upload_data(screen, full);
		screen.clear_dirty_rows();
		self.needs_full_upload = false;
		let overlay_texture = if overlay.is_empty() {
			None
		} else {
			Some(self.draw_overlay(screen, overlay)?)
		};
		self.draw(screen, target, overlay_texture)
	}

	/// Создать или пересоздать объекты наложения под сетку `grid_px` (ADR-0014).
	fn ensure_overlay(&mut self, grid_px: (u32, u32)) -> Result<(), RenderError> {
		if self.res.overlay.as_ref().is_some_and(|o| o.size == grid_px) {
			return Ok(());
		}
		let header = match self.res.ctx.info.kind {
			RendererKind::Gles30 => HEADER_GLES,
			_ => HEADER_GL,
		};
		let gl = &self.res.ctx.gl;
		// SAFETY: контекст текущий; старые объекты удаляются, новые проверяются (FBO — на полноту),
		// при ошибке созданное освобождается и возвращается `RenderError` — путь отката (FR-18).
		unsafe {
			if let Some(old) = self.res.overlay.take() {
				delete_overlay(gl, old);
			}
			let program = build_program(gl, header, OVERLAY_FRAG_SRC)?;
			let uniforms = OverlayUniforms {
				prev: gl.get_uniform_location(program, "u_prev"),
				layer: gl.get_uniform_location(program, "u_layer"),
				atlas: gl.get_uniform_location(program, "u_atlas"),
				layer_origin: gl.get_uniform_location(program, "u_layer_origin"),
				layer_size: gl.get_uniform_location(program, "u_layer_size"),
				glyph: gl.get_uniform_location(program, "u_glyph"),
				atlas_cols: gl.get_uniform_location(program, "u_atlas_cols"),
				style_channel: gl.get_uniform_location(program, "u_style_channel"),
			};
			let mut textures = Vec::with_capacity(2);
			let mut fbos = Vec::with_capacity(2);
			for _ in 0..2 {
				let tex = gl.create_texture().map_err(RenderError::Other)?;
				gl.bind_texture(glow::TEXTURE_2D, Some(tex));
				set_nearest_clamp(gl);
				gl.tex_image_2d(
					glow::TEXTURE_2D,
					0,
					glow::RGBA8UI as i32,
					grid_px.0.max(1) as i32,
					grid_px.1.max(1) as i32,
					0,
					glow::RGBA_INTEGER,
					glow::UNSIGNED_BYTE,
					glow::PixelUnpackData::Slice(None),
				);
				let fbo = gl.create_framebuffer().map_err(RenderError::Other)?;
				gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
				gl.framebuffer_texture_2d(
					glow::FRAMEBUFFER,
					glow::COLOR_ATTACHMENT0,
					glow::TEXTURE_2D,
					Some(tex),
					0,
				);
				textures.push(tex);
				fbos.push(fbo);
			}
			let layer_texture = gl.create_texture().map_err(RenderError::Other)?;
			gl.bind_texture(glow::TEXTURE_2D, Some(layer_texture));
			set_nearest_clamp(gl);
			gl.bind_texture(glow::TEXTURE_2D, None);
			let objects = OverlayGl {
				program,
				uniforms,
				textures: [textures[0], textures[1]],
				fbos: [fbos[0], fbos[1]],
				size: grid_px,
				layer_texture,
				layer_size: (0, 0),
			};
			let mut status = glow::FRAMEBUFFER_COMPLETE;
			for &fbo in &objects.fbos {
				gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
				let s = gl.check_framebuffer_status(glow::FRAMEBUFFER);
				if s != glow::FRAMEBUFFER_COMPLETE {
					status = s;
				}
			}
			gl.bind_framebuffer(glow::FRAMEBUFFER, None);
			if status != glow::FRAMEBUFFER_COMPLETE {
				delete_overlay(gl, objects);
				return Err(RenderError::Other(format!(
					"overlay framebuffer incomplete: 0x{status:X}"
				)));
			}
			self.res.overlay = Some(objects);
			check_gl_error(gl, "overlay")
		}
	}

	/// Записи наложения в текстуру — проход на запись, попеременно между двумя текстурами
	/// (ADR-0014, Н1); возвращает текстуру с итогом.
	fn draw_overlay(
		&mut self,
		screen: &ScreenBuffer,
		overlay: &Overlay,
	) -> Result<glow::Texture, RenderError> {
		let grid_px = self.grid_size_px(screen);
		self.ensure_overlay(grid_px)?;
		let max = self.res.ctx.info.max_texture_size as usize;
		let mut channels = [0i32; 4];
		for (i, &s) in Style::ALL.iter().enumerate() {
			channels[i] = self.atlas.rasterizer().family().style_source(s).channel() as i32;
		}
		let atlas_cols = (self.atlas.page_px() / self.atlas.glyph_w()) as i32;
		let glyph = (self.atlas.glyph_w() as i32, self.atlas.glyph_h() as i32);
		let Some(objects) = self.res.overlay.as_mut() else {
			return Err(RenderError::Other("overlay objects missing".to_owned()));
		};
		let gl = &self.res.ctx.gl;
		let mut current = 0;
		// SAFETY: контекст текущий; объекты созданы в `ensure_overlay`, размеры текстуры записи
		// проверены против `GL_MAX_TEXTURE_SIZE`, буфер строки — ровно 16 × cols байт.
		unsafe {
			gl.disable(glow::BLEND);
			gl.disable(glow::DEPTH_TEST);
			gl.disable(glow::SCISSOR_TEST);
			gl.bind_framebuffer(glow::FRAMEBUFFER, Some(objects.fbos[0]));
			gl.clear_buffer_u32_slice(glow::COLOR, 0, &[0, 0, 0, 0]);
			for layer in overlay.layers() {
				if layer.grid_rows(screen.rows()).is_empty()
					|| layer.grid_cols(screen.cols()).is_empty()
				{
					continue;
				}
				if layer.cols > max || layer.rows > max {
					return Err(RenderError::Other(format!(
						"overlay buffer {}x{} exceeds GL_MAX_TEXTURE_SIZE {max}",
						layer.cols, layer.rows
					)));
				}
				gl.bind_texture(glow::TEXTURE_2D, Some(objects.layer_texture));
				if layer.cols > objects.layer_size.0 || layer.rows > objects.layer_size.1 {
					objects.layer_size = (
						layer.cols.max(objects.layer_size.0),
						layer.rows.max(objects.layer_size.1),
					);
					gl.tex_image_2d(
						glow::TEXTURE_2D,
						0,
						glow::RGBA32UI as i32,
						objects.layer_size.0 as i32,
						objects.layer_size.1 as i32,
						0,
						glow::RGBA_INTEGER,
						glow::UNSIGNED_INT,
						glow::PixelUnpackData::Slice(None),
					);
				}
				gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 4);
				self.layer_staging.clear();
				self.layer_staging.resize(layer.cols * DATA_TEXEL_BYTES, 0);
				for row in 0..layer.rows {
					for col in 0..layer.cols {
						let packed = pack_cell(&self.atlas, layer.cell(col, row));
						write_texel(&mut self.layer_staging, col, &packed);
					}
					gl.tex_sub_image_2d(
						glow::TEXTURE_2D,
						0,
						0,
						row as i32,
						layer.cols as i32,
						1,
						glow::RGBA_INTEGER,
						glow::UNSIGNED_INT,
						glow::PixelUnpackData::Slice(Some(&self.layer_staging)),
					);
				}

				let next = 1 - current;
				let u = &objects.uniforms;
				gl.bind_framebuffer(glow::FRAMEBUFFER, Some(objects.fbos[next]));
				gl.viewport(0, 0, grid_px.0 as i32, grid_px.1 as i32);
				gl.use_program(Some(objects.program));
				gl.active_texture(glow::TEXTURE0);
				gl.bind_texture(glow::TEXTURE_2D, Some(objects.textures[current]));
				gl.active_texture(glow::TEXTURE1);
				gl.bind_texture(glow::TEXTURE_2D, Some(objects.layer_texture));
				gl.active_texture(glow::TEXTURE2);
				gl.bind_texture(glow::TEXTURE_2D, Some(self.res.atlas_texture));
				gl.uniform_1_i32(u.prev.as_ref(), 0);
				gl.uniform_1_i32(u.layer.as_ref(), 1);
				gl.uniform_1_i32(u.atlas.as_ref(), 2);
				gl.uniform_2_i32(u.layer_origin.as_ref(), layer.col, layer.row);
				gl.uniform_2_i32(u.layer_size.as_ref(), layer.cols as i32, layer.rows as i32);
				gl.uniform_2_i32(u.glyph.as_ref(), glyph.0, glyph.1);
				gl.uniform_1_i32(u.atlas_cols.as_ref(), atlas_cols);
				gl.uniform_4_i32(
					u.style_channel.as_ref(),
					channels[0],
					channels[1],
					channels[2],
					channels[3],
				);
				gl.bind_vertex_array(Some(self.res.vao));
				gl.draw_arrays(glow::TRIANGLES, 0, 3);
				gl.bind_vertex_array(None);
				current = next;
			}
			gl.bind_framebuffer(glow::FRAMEBUFFER, None);
		}
		Ok(objects.textures[current])
	}

	/// Нарисовать в FBO и прочитать кадр (`0x00RRGGBB`, строки сверху вниз) — для сравнения с Software (§11.2).
	pub fn render_to_frame(&mut self, screen: &mut ScreenBuffer) -> Result<&[u32], RenderError> {
		self.render_to_frame_overlaid(screen, &Overlay::new())
	}

	/// То же с наложением (ADR-0014).
	pub fn render_to_frame_overlaid(
		&mut self,
		screen: &mut ScreenBuffer,
		overlay: &Overlay,
	) -> Result<&[u32], RenderError> {
		if self.width == 0 || self.height == 0 {
			let (w, h) = self.grid_size_px(screen);
			self.resize(w, h)?;
		}
		self.ensure_offscreen()?;
		self.render_into(screen, overlay, Target::Offscreen)?;
		let (w, h) = (self.width as usize, self.height as usize);
		self.read_staging.clear();
		self.read_staging.resize(w * h * 4, 0);
		self.frame.clear();
		self.frame.resize(w * h, 0);
		let gl = &self.res.ctx.gl;
		// SAFETY: FBO с текстурой RGBA8 размера width×height привязан; буфер ровно w×h×4 байт.
		unsafe {
			gl.bind_framebuffer(glow::FRAMEBUFFER, self.res.offscreen.map(|(f, _)| f));
			gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
			gl.read_pixels(
				0,
				0,
				w as i32,
				h as i32,
				glow::RGBA,
				glow::UNSIGNED_BYTE,
				glow::PixelPackData::Slice(Some(&mut self.read_staging)),
			);
			gl.bind_framebuffer(glow::FRAMEBUFFER, None);
		}
		check_gl_error(gl, "read pixels")?;
		// GL отдаёт строки снизу вверх.
		for y in 0..h {
			let src_row = (h - 1 - y) * w;
			for x in 0..w {
				let p = &self.read_staging[(src_row + x) * 4..][..3];
				self.frame[y * w + x] = (p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32;
			}
		}
		Ok(&self.frame)
	}

	fn ensure_offscreen(&mut self) -> Result<(), RenderError> {
		let gl = &self.res.ctx.gl;
		// SAFETY: контекст текущий; текстура RGBA8 размера окна, FBO проверяется на полноту.
		unsafe {
			if let Some((fbo, tex)) = self.res.offscreen.take() {
				gl.delete_framebuffer(fbo);
				gl.delete_texture(tex);
			}
			let tex = gl.create_texture().map_err(RenderError::Other)?;
			gl.bind_texture(glow::TEXTURE_2D, Some(tex));
			set_nearest_clamp(gl);
			gl.tex_image_2d(
				glow::TEXTURE_2D,
				0,
				glow::RGBA8 as i32,
				self.width as i32,
				self.height as i32,
				0,
				glow::RGBA,
				glow::UNSIGNED_BYTE,
				glow::PixelUnpackData::Slice(None),
			);
			let fbo = gl.create_framebuffer().map_err(RenderError::Other)?;
			gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
			gl.framebuffer_texture_2d(
				glow::FRAMEBUFFER,
				glow::COLOR_ATTACHMENT0,
				glow::TEXTURE_2D,
				Some(tex),
				0,
			);
			let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
			gl.bind_framebuffer(glow::FRAMEBUFFER, None);
			if status != glow::FRAMEBUFFER_COMPLETE {
				return Err(RenderError::Other(format!(
					"framebuffer incomplete: 0x{status:X}"
				)));
			}
			self.res.offscreen = Some((fbo, tex));
		}
		Ok(())
	}
}

impl Renderer for GlBackend {
	fn name(&self) -> &'static str {
		self.res.ctx.info.kind.name()
	}

	fn resize(&mut self, width_px: u32, height_px: u32) -> Result<(), RenderError> {
		self.width = width_px;
		self.height = height_px;
		self.res.ctx.resize_surface(width_px, height_px);
		if self.res.offscreen.is_some() {
			self.ensure_offscreen()?;
		}
		self.needs_full_upload = true;
		Ok(())
	}

	fn render_overlaid(
		&mut self,
		screen: &mut ScreenBuffer,
		overlay: &Overlay,
	) -> Result<(), RenderError> {
		let started = Instant::now();
		self.render_into(screen, overlay, Target::Screen)?;
		self.last_cpu_time = started.elapsed();
		self.res.ctx.swap_buffers()
	}
}

/// Тексель данных ячейки (§6.1): слот и флаги, цвета целиком `0xAARRGGBB` (ADR-0013).
fn pack_cell(atlas: &Atlas, cell: crate::cell::Cell) -> [u32; 4] {
	let slot = atlas.slot_of(cell.ch).unwrap_or(EMPTY_SLOT) as u32;
	[
		slot | ((cell.flags.bits() as u32) << 16),
		cell.fg.0,
		cell.bg.0,
		0,
	]
}

/// Записать тексель `col` в буфер строки данных.
fn write_texel(staging: &mut [u8], col: usize, packed: &[u32; 4]) {
	for (i, v) in packed.iter().enumerate() {
		let at = col * DATA_TEXEL_BYTES + i * 4;
		staging[at..at + 4].copy_from_slice(&v.to_ne_bytes());
	}
}

/// Программа из общего вершинного шейдера и фрагментного `frag_src`.
///
/// # Safety
/// Контекст `gl` текущий.
unsafe fn build_program(
	gl: &glow::Context,
	header: &str,
	frag_src: &str,
) -> Result<glow::Program, RenderError> {
	// SAFETY: вызывающий гарантирует текущий контекст; шейдеры удаляются после компоновки.
	unsafe {
		let program = gl.create_program().map_err(RenderError::Other)?;
		let vs = compile_shader(gl, glow::VERTEX_SHADER, header, VERT_SRC)?;
		let fs = compile_shader(gl, glow::FRAGMENT_SHADER, header, frag_src)?;
		gl.attach_shader(program, vs);
		gl.attach_shader(program, fs);
		gl.link_program(program);
		gl.delete_shader(vs);
		gl.delete_shader(fs);
		if !gl.get_program_link_status(program) {
			let log = gl.get_program_info_log(program);
			gl.delete_program(program);
			return Err(RenderError::Other(format!("link: {log}")));
		}
		Ok(program)
	}
}

/// Освободить объекты наложения.
///
/// # Safety
/// Контекст `gl` текущий, объекты созданы в нём.
unsafe fn delete_overlay(gl: &glow::Context, objects: OverlayGl) {
	// SAFETY: вызывающий гарантирует текущий контекст и принадлежность объектов.
	unsafe {
		for fbo in objects.fbos {
			gl.delete_framebuffer(fbo);
		}
		for tex in objects.textures {
			gl.delete_texture(tex);
		}
		gl.delete_texture(objects.layer_texture);
		gl.delete_program(objects.program);
	}
}

/// Страница атласа целиком в текстуру `RGBA8UI`.
fn upload_atlas_page(
	gl: &glow::Context,
	texture: glow::Texture,
	atlas: &Atlas,
) -> Result<(), RenderError> {
	let page = atlas.page_px() as i32;
	// SAFETY: контекст текущий; срез `cells()` ровно page² × 4 байт.
	unsafe {
		gl.bind_texture(glow::TEXTURE_2D, Some(texture));
		gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
		gl.tex_sub_image_2d(
			glow::TEXTURE_2D,
			0,
			0,
			0,
			page,
			page,
			glow::RGBA_INTEGER,
			glow::UNSIGNED_BYTE,
			glow::PixelUnpackData::Slice(Some(atlas.cells())),
		);
	}
	check_gl_error(gl, "upload atlas page")
}

/// Компиляция шейдера с подставленным заголовком; ошибка — с логом драйвера.
unsafe fn compile_shader(
	gl: &glow::Context,
	kind: u32,
	header: &str,
	source: &str,
) -> Result<glow::Shader, RenderError> {
	// SAFETY: вызывающий гарантирует текущий контекст.
	unsafe {
		let shader = gl.create_shader(kind).map_err(RenderError::Other)?;
		let full = format!("{header}{source}");
		gl.shader_source(shader, &full);
		gl.compile_shader(shader);
		if !gl.get_shader_compile_status(shader) {
			let log = gl.get_shader_info_log(shader);
			gl.delete_shader(shader);
			let what = if kind == glow::VERTEX_SHADER {
				"vertex"
			} else {
				"fragment"
			};
			return Err(RenderError::Other(format!("{what} shader: {log}")));
		}
		Ok(shader)
	}
}

/// NEAREST + CLAMP_TO_EDGE для привязанной текстуры (§6.3, FR-13).
unsafe fn set_nearest_clamp(gl: &glow::Context) {
	// SAFETY: вызывающий гарантирует текущий контекст и привязанную TEXTURE_2D.
	unsafe {
		gl.tex_parameter_i32(
			glow::TEXTURE_2D,
			glow::TEXTURE_MIN_FILTER,
			glow::NEAREST as i32,
		);
		gl.tex_parameter_i32(
			glow::TEXTURE_2D,
			glow::TEXTURE_MAG_FILTER,
			glow::NEAREST as i32,
		);
		gl.tex_parameter_i32(
			glow::TEXTURE_2D,
			glow::TEXTURE_WRAP_S,
			glow::CLAMP_TO_EDGE as i32,
		);
		gl.tex_parameter_i32(
			glow::TEXTURE_2D,
			glow::TEXTURE_WRAP_T,
			glow::CLAMP_TO_EDGE as i32,
		);
	}
}

/// `glGetError` → `RenderError` (§9 п.5, FR-18); потеря контекста — отдельный вариант (FR-21).
fn check_gl_error(gl: &glow::Context, stage: &str) -> Result<(), RenderError> {
	// SAFETY: контекст текущий; запрос без побочных эффектов.
	let code = unsafe { gl.get_error() };
	match code {
		glow::NO_ERROR => Ok(()),
		GL_CONTEXT_LOST => Err(RenderError::ContextLost),
		_ => Err(RenderError::Other(format!(
			"glGetError 0x{code:X} after {stage}"
		))),
	}
}
