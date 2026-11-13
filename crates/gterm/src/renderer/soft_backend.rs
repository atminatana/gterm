//! CPU-рендерер (ТЗ §8) — эталонная реализация алгоритма фрагментного шейдера §7.3.
//! Кадр `0x00RRGGBB` (формат `softbuffer`), предвыделен, перерисовываются только dirty-строки;
//! при `resize`, смене фона буфера или сбросе атласа — кадр целиком. Перерисованное отдаётся полосами [`RowBand`] — по ним окно
//! копирует и выводит только изменённое (ADR-0010). Горячий путь без аллокаций
//! (NFR-6); промах кэша глифов — отдельный путь в [`crate::atlas`] (ADR-0003 п.7).
//! Покрытие глифа `a` (0…255) смешивает fg и bg **целочисленно** (ADR-0002) — та же формула в шейдере.
//! Один пиксель глифа = один пиксель кадра: блока увеличения нет (FR-13, ADR-0005).
//! Наложение клиентских буферов (ADR-0014): записи рисуются в текстуру размером с сетку
//! (предумноженная альфа), она накладывается на перерисованные строки; строки под наложением
//! в этом и прошлом кадре перерисовываются всегда. Текстура пересобирается, только если записи
//! или геометрия изменились с прошлой сборки (ADR-0015).
//! Кадр и текстура рисуются полосами строк сетки в постоянном пуле потоков `rayon`, малые кадры —
//! в текущем потоке; результат от числа потоков не зависит (ADR-0016).

use crate::atlas::Atlas;
use crate::buffer::ScreenBuffer;
use crate::cell::Color;
use crate::overlay::Overlay;
use crate::rasterizer::BoxDrawingMode;
use crate::renderer::pixel::encode_color;
use crate::renderer::soft_band::{MainPass, OverlayPass, layer_cell};
use crate::renderer::{Frame, RenderError, Renderer};

use rayon::prelude::*;

/// Наибольшее число потоков отрисовки по умолчанию (ADR-0016): дальше прирост мал, а Software
/// часто работает на ВМ и RDP с 2–4 vCPU.
const MAX_THREADS: usize = 4;
/// Меньше строк к отрисовке — в текущем потоке: раздача полос дороже самой работы (ввод с
/// клавиатуры обычно меняет одну строку).
const PARALLEL_MIN_ROWS: usize = 8;

/// Прямоугольник кадра, перерисованный последним `render` (ADR-0010): полоса подряд идущих строк
/// сетки шириной сетки либо весь кадр; обрезан по границам кадра, размеры ненулевые.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowBand {
	pub x: u32,
	pub y: u32,
	pub width: u32,
	pub height: u32,
}

/// Бэкенд Software-рендеринга.
pub struct SoftBackend {
	/// Атлас глифов; размер ячейки берётся из него (FR-10).
	atlas: Atlas,
	/// Ширина окна в пикселях.
	width: u32,
	/// Высота окна в пикселях.
	height: u32,
	/// Кадр: `width * height` слов `0x00RRGGBB`.
	frame: Frame,
	/// Фон буфера прошлого кадра; другой фон — полная перерисовка (ADR-0013).
	last_background: Option<Color>,
	/// Кадр ещё не рисовался или изменил размер.
	needs_full_redraw: bool,
	/// Число сбросов атласа на прошлом кадре.
	atlas_resets_seen: u32,
	/// Строки, рисуемые текущим кадром; переиспользуется (NFR-6).
	rows_to_draw: Vec<bool>,
	/// Перерисованное последним `render`; переиспользуется (NFR-6).
	damage: Vec<RowBand>,
	/// Текстура наложения размером с сетку в пикселях: `0xAARRGGBB`, RGB предумножен (ADR-0014).
	overlay_px: Vec<u32>,
	/// Размер `overlay_px` в пикселях.
	overlay_size: (u32, u32),
	/// Строки сетки под наложением в этом кадре.
	overlay_rows: Vec<bool>,
	/// Строки сетки под наложением в прошлом кадре: их тоже нужно перерисовать.
	prev_overlay_rows: Vec<bool>,
	/// Записи, по которым собрана `overlay_px` (ADR-0015); память переиспользуется (NFR-6).
	overlay_drawn: Overlay,
	/// `overlay_px` собрана по `overlay_drawn` при текущих метриках ячейки.
	is_overlay_valid: bool,
	/// Пул потоков отрисовки (ADR-0016); `None` — один поток или пул не создался.
	pool: Option<rayon::ThreadPool>,
}

/// Геометрия кадра, одинаковая для всех ячеек одного рендера.
struct Layout {
	cell_h: u32,
	/// Смещение сетки в окне (§7.2 `u_origin`): сетка центрируется.
	origin_x: u32,
	origin_y: u32,
}

impl SoftBackend {
	/// Рендерер с числом потоков по умолчанию: ядра машины, но не больше `MAX_THREADS`.
	pub fn new(atlas: Atlas) -> Self {
		let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
		Self::with_threads(atlas, threads.min(MAX_THREADS))
	}

	/// Рендерер с `threads` потоками отрисовки (тесты, замеры). Пул не создался — рисуем в текущем
	/// потоке и сообщаем причину: отказ графики не повод для паники (NFR-7).
	pub fn with_threads(atlas: Atlas, threads: usize) -> Self {
		let pool = (threads > 1)
			.then(|| {
				rayon::ThreadPoolBuilder::new()
					.num_threads(threads)
					.thread_name(|i| format!("gterm-soft-{i}"))
					.build()
					.inspect_err(|e| eprintln!("gterm: Software render threads: {e}; using one"))
					.ok()
			})
			.flatten();
		Self {
			atlas_resets_seen: atlas.resets(),
			atlas,
			width: 0,
			height: 0,
			frame: Vec::new(),
			last_background: None,
			needs_full_redraw: true,
			rows_to_draw: Vec::new(),
			damage: Vec::new(),
			overlay_px: Vec::new(),
			overlay_size: (0, 0),
			overlay_rows: Vec::new(),
			prev_overlay_rows: Vec::new(),
			overlay_drawn: Overlay::new(),
			is_overlay_valid: false,
			pool,
		}
	}

	/// Что перерисовал последний `render`: весь кадр — один прямоугольник, иначе полосы строк по
	/// возрастанию `y`; пусто — кадр не менялся.
	pub fn damage(&self) -> &[RowBand] {
		&self.damage
	}

	/// Ширина кадра в пикселях.
	pub fn width(&self) -> u32 {
		self.width
	}

	/// Высота кадра в пикселях.
	pub fn height(&self) -> u32 {
		self.height
	}

	/// Сырой кадр (`0x00RRGGBB`), `width * height` слов.
	pub fn frame(&self) -> &[u32] {
		&self.frame
	}

	/// Число потоков отрисовки.
	pub fn threads(&self) -> usize {
		self.pool
			.as_ref()
			.map_or(1, |pool| pool.current_num_threads())
	}

	/// Атлас глифов, с которым работает рендерер.
	pub fn atlas(&self) -> &Atlas {
		&self.atlas
	}

	/// Атлас для подготовки глифов (тесты, смена размера шрифта).
	pub fn atlas_mut(&mut self) -> &mut Atlas {
		&mut self.atlas
	}

	/// Забрать атлас (с растеризатором) при переходе на другой бэкенд (FR-21).
	pub fn into_atlas(self) -> Atlas {
		self.atlas
	}

	/// Сменить физический размер растеризации на месте — путь смены DPI (FR-24, ADR-0005 п.7).
	///
	/// Метрики ячейки изменились, поэтому кадр, нарисованный по старой сетке, больше не подходит:
	/// все строки перерисовываются целиком. Счётчик сбросов приводится к текущему, иначе первая
	/// же проверка в `render` сочла бы страницу только что сброшенной — результат тот же, но
	/// лишняя полная перерисовка на каждом кадре.
	pub fn set_size(&mut self, size_px: u16) -> Result<(), RenderError> {
		self.atlas
			.set_size(size_px)
			.map_err(|e| RenderError::Other(format!("font size {size_px} px: {e}")))?;
		self.atlas_resets_seen = self.atlas.resets();
		self.needs_full_redraw = true;
		// Размер сетки в пикселях может совпасть при других метриках — растр глифов уже другой.
		self.is_overlay_valid = false;
		Ok(())
	}

	/// Источник псевдографики (ADR-0018): страница атласа сброшена — полная перерисовка, как
	/// после смены размера.
	pub fn set_box_drawing_mode(&mut self, mode: BoxDrawingMode) {
		if self.atlas.box_drawing_mode() == mode {
			return;
		}
		self.atlas.set_box_drawing_mode(mode);
		self.atlas_resets_seen = self.atlas.resets();
		self.needs_full_redraw = true;
		self.is_overlay_valid = false;
	}

	/// Размер сетки в пикселях окна: ячейка из метрик растеризации, масштаба нет (ADR-0005).
	pub fn grid_size_px(&self, screen: &ScreenBuffer) -> (u32, u32) {
		(
			screen.cols() as u32 * self.atlas.glyph_w() as u32,
			screen.rows() as u32 * self.atlas.glyph_h() as u32,
		)
	}

	fn layout(&self, screen: &ScreenBuffer) -> Layout {
		let (grid_w, grid_h) = self.grid_size_px(screen);
		Layout {
			cell_h: self.atlas.glyph_h() as u32,
			origin_x: self.width.saturating_sub(grid_w) / 2,
			origin_y: self.height.saturating_sub(grid_h) / 2,
		}
	}

	/// Отметить строки кадра: все — при полном кадре, иначе dirty-строки (ADR-0010 п.1) и строки
	/// под наложением в этом или прошлом кадре (ADR-0014).
	fn mark_rows(&mut self, screen: &ScreenBuffer, full: bool) {
		let (now, before) = (&self.overlay_rows, &self.prev_overlay_rows);
		self.rows_to_draw.clear();
		self.rows_to_draw.extend((0..screen.rows()).map(|row| {
			full || screen.is_row_dirty(row)
				|| now[row] || before.get(row).copied().unwrap_or(false)
		}));
	}

	/// Строки сетки, которые задевает наложение этого кадра.
	fn mark_overlay_rows(&mut self, screen: &ScreenBuffer, overlay: &Overlay) {
		self.overlay_rows.clear();
		self.overlay_rows.resize(screen.rows(), false);
		for layer in overlay.layers() {
			if layer.grid_cols(screen.cols()).is_empty() {
				continue;
			}
			for row in layer.grid_rows(screen.rows()) {
				self.overlay_rows[row] = true;
			}
		}
	}

	/// Промахи кэша для всех ячеек отмеченных строк. Возвращает `true`, если при этом страница
	/// атласа сбрасывалась — тогда нужен полный кадр.
	///
	/// Записи наложения готовятся целиком (видимая часть): их глифы нужны в том же кадре.
	fn prepare_glyphs(&mut self, screen: &ScreenBuffer, overlay: &Overlay) -> bool {
		let before = self.atlas.resets();
		for row in 0..screen.rows() {
			if self.rows_to_draw[row] {
				for col in 0..screen.cols() {
					let cell = screen.cell(col, row);
					self.atlas.ensure(cell.ch, cell.style());
				}
			}
		}
		for layer in overlay.layers() {
			for row in layer.grid_rows(screen.rows()) {
				for col in layer.grid_cols(screen.cols()) {
					let cell = layer_cell(&layer, col, row);
					self.atlas.ensure(cell.ch, cell.style());
				}
			}
		}
		self.atlas.resets() != before
	}

	/// Текстура собрана по тем же записям и той же геометрии сетки (ADR-0015).
	fn is_overlay_current(&self, screen: &ScreenBuffer, overlay: &Overlay) -> bool {
		self.is_overlay_valid
			&& self.overlay_size == self.grid_size_px(screen)
			&& self.overlay_drawn == *overlay
	}

	/// Собрать текстуру заново: очистить строки прошлой сборки и нарисовать записи. После смены
	/// метрик (`!is_overlay_valid`) строки прошлой сборки лежали при другой высоте ячейки —
	/// текстура очищается целиком.
	fn build_overlay(&mut self, screen: &ScreenBuffer, overlay: &Overlay, layout: &Layout) {
		let (grid_w, grid_h) = self.grid_size_px(screen);
		let is_cleared = !self.is_overlay_valid || self.overlay_size != (grid_w, grid_h);
		if is_cleared {
			self.overlay_px.clear();
			self.overlay_px.resize((grid_w * grid_h) as usize, 0);
			self.overlay_size = (grid_w, grid_h);
		}
		let pass = OverlayPass {
			atlas: &self.atlas,
			overlay,
			grid_cols: screen.cols(),
			grid_rows: screen.rows(),
			overlay_rows: &self.overlay_rows,
			prev_overlay_rows: &self.prev_overlay_rows,
			is_cleared,
		};
		let work_rows = self.overlay_rows.iter().filter(|&&row| row).count();
		for_each_band(
			self.pool.as_ref(),
			&mut self.overlay_px,
			screen.rows(),
			layout.cell_h as usize * grid_w as usize,
			work_rows,
			|first_row, band| pass.build_band(first_row, band),
		);
		self.overlay_drawn.clone_from(overlay);
		self.is_overlay_valid = true;
	}

	/// Основной слой отмеченных строк и наложение текстуры на строки под ней — по полосам строк
	/// сетки (ADR-0016).
	fn draw_rows(&mut self, screen: &ScreenBuffer, layout: &Layout) {
		if self.width == 0 || self.height == 0 {
			return;
		}
		let width = self.width as usize;
		let pass = MainPass {
			atlas: &self.atlas,
			screen,
			rows_to_draw: &self.rows_to_draw,
			overlay_rows: &self.overlay_rows,
			texture: &self.overlay_px,
			grid_w: self.overlay_size.0 as usize,
			width,
			origin_x: layout.origin_x as usize,
		};
		let start = layout.origin_y as usize * width;
		let work_rows = self.rows_to_draw.iter().filter(|&&row| row).count();
		for_each_band(
			self.pool.as_ref(),
			&mut self.frame[start..],
			screen.rows(),
			layout.cell_h as usize * width,
			work_rows,
			|first_row, band| pass.draw_band(first_row, band),
		);
	}

	/// Полосы перерисованного (ADR-0010 п.2): полный кадр — весь кадр; иначе подряд идущие
	/// отмеченные строки сливаются в полосу шириной сетки, обрезанную по кадру.
	fn collect_damage(&mut self, screen: &ScreenBuffer, full: bool, layout: &Layout) {
		if self.width == 0 || self.height == 0 {
			return;
		}
		if full {
			self.damage.push(RowBand {
				x: 0,
				y: 0,
				width: self.width,
				height: self.height,
			});
			return;
		}
		let (grid_w, _) = self.grid_size_px(screen);
		let width = grid_w.min(self.width - layout.origin_x);
		if width == 0 {
			return;
		}
		let rows = self.rows_to_draw.len();
		let mut row = 0;
		while row < rows {
			if !self.rows_to_draw[row] {
				row += 1;
				continue;
			}
			let first = row;
			while row < rows && self.rows_to_draw[row] {
				row += 1;
			}
			let y = layout.origin_y + first as u32 * layout.cell_h;
			if y >= self.height {
				break;
			}
			let height = ((row - first) as u32 * layout.cell_h).min(self.height - y);
			self.damage.push(RowBand {
				x: layout.origin_x,
				y,
				width,
				height,
			});
		}
	}
}

impl Renderer for SoftBackend {
	fn name(&self) -> &'static str {
		"Software"
	}

	fn resize(&mut self, width_px: u32, height_px: u32) -> Result<(), RenderError> {
		self.width = width_px;
		self.height = height_px;
		self.frame.clear();
		self.frame.resize((width_px * height_px) as usize, 0);
		self.needs_full_redraw = true;
		Ok(())
	}

	fn render_overlaid(
		&mut self,
		screen: &mut ScreenBuffer,
		overlay: &Overlay,
	) -> Result<(), RenderError> {
		if self.atlas.glyph_w() == 0 || self.atlas.glyph_h() == 0 {
			return Err(RenderError::Other("cell size must be >= 1 px".to_owned()));
		}
		// Кадр не задан — берём точный размер сетки (удобно для тестов и offscreen).
		if self.width == 0 && self.height == 0 {
			let (w, h) = self.grid_size_px(screen);
			self.resize(w, h)?;
		}
		let layout = self.layout(screen);
		self.damage.clear();
		let mut full = self.needs_full_redraw
			|| self.last_background != Some(screen.background())
			|| self.atlas.resets() != self.atlas_resets_seen;
		self.mark_overlay_rows(screen, overlay);
		self.mark_rows(screen, full);

		// Сброс страницы посреди подготовки делает ранние слоты недействительными — повторяем
		// уже для всех строк. Страница ≥ 1024 px вмещает все глифы шрифта, поэтому второй проход
		// не сбрасывает; если всё же сбросил — экран не помещается, рисовать мусор нельзя (NFR-7).
		if self.prepare_glyphs(screen, overlay) {
			full = true;
			self.mark_rows(screen, true);
			if self.prepare_glyphs(screen, overlay) {
				return Err(RenderError::Other(format!(
					"atlas page {}px holds {} glyphs, screen needs more",
					self.atlas.page_px(),
					self.atlas.capacity()
				)));
			}
		}
		self.atlas_resets_seen = self.atlas.resets();

		if full {
			self.frame.fill(encode_color(screen.background()));
		}
		// Текстура от кадра не зависит — собирается до основного слоя, и полосы кадра
		// накладывают её сразу после своих строк.
		let has_overlay = !overlay.is_empty() || self.prev_overlay_rows.contains(&true);
		if has_overlay && !self.is_overlay_current(screen, overlay) {
			self.build_overlay(screen, overlay, &layout);
		}
		self.draw_rows(screen, &layout);
		self.collect_damage(screen, full, &layout);
		screen.clear_dirty_rows();

		self.needs_full_redraw = false;
		self.last_background = Some(screen.background());
		std::mem::swap(&mut self.overlay_rows, &mut self.prev_overlay_rows);
		Ok(())
	}
}

/// Раздать `data` полосами по `grid_rows` строк сетки (`row_len` элементов на строку) и вызвать
/// `work(first_row, band)` для каждой; поле окна под сеткой — лишние полосы без работы. С пулом и не меньше `PARALLEL_MIN_ROWS` строк работы —
/// по полосе на поток, иначе одна полоса на всё в текущем потоке. `par_chunks_mut` в пуле не
/// выделяет память (NFR-6, тест `soft_threads_alloc`).
fn for_each_band(
	pool: Option<&rayon::ThreadPool>,
	data: &mut [u32],
	grid_rows: usize,
	row_len: usize,
	work_rows: usize,
	work: impl Fn(usize, &mut [u32]) + Sync,
) {
	if data.is_empty() || row_len == 0 {
		return;
	}
	match pool {
		Some(pool) if work_rows >= PARALLEL_MIN_ROWS => {
			let rows_per_band = grid_rows.div_ceil(pool.current_num_threads()).max(1);
			pool.install(|| {
				data.par_chunks_mut(rows_per_band * row_len)
					.enumerate()
					.for_each(|(i, band)| work(i * rows_per_band, band));
			});
		}
		_ => work(0, data),
	}
}
