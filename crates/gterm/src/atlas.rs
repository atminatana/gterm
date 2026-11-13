//! Динамический атлас глифов (ТЗ FR-9, §6.3, ADR-0003): одна страница `RGBA8`, где каналы —
//! начертания (R Regular, G Bold, B Italic, A Bold Italic). Слот выделяется кодовой точке при
//! первом показе; каналы заполняются лениво. Переполнение — сброс страницы.

use crate::cell::Style;
use crate::font::{CellMetrics, FontError};
use crate::rasterizer::{BoxDrawingMode, Rasterizer};

/// Слот «глифа нет» — пустой растр во всех каналах.
pub const EMPTY_SLOT: u16 = 0;
/// Верхняя граница стороны страницы (ADR-0003 п.5).
pub const PAGE_PX_MAX: u32 = 2048;
/// Нижняя граница — минимум `GL_MAX_TEXTURE_SIZE` по стандарту GL 3.3.
pub const PAGE_PX_MIN: u32 = 1024;
/// Байт на тексель (четыре канала-начертания).
pub const CHANNELS: usize = 4;

/// Кодовая точка ещё не запрашивалась.
const NOT_LOADED: u16 = u16::MAX;
/// Число кодовых точек Unicode U+0000–U+10FFFF: плоская таблица, O(1) (ADR-0009).
const CODEPOINT_COUNT: usize = char::MAX as usize + 1;

/// Сторона страницы по лимиту драйвера: `min(limit, PAGE_PX_MAX)`, но не меньше `PAGE_PX_MIN`.
pub fn page_px_for_limit(max_texture_size: Option<u32>) -> u32 {
	max_texture_size
		.unwrap_or(PAGE_PX_MIN)
		.clamp(PAGE_PX_MIN, PAGE_PX_MAX)
}

/// Страница атласа с кэшем «кодовая точка → слот».
pub struct Atlas {
	/// В куче: растеризатор несёт таблицы автохинтера и велик для стека/перемещений.
	rasterizer: Box<Rasterizer>,
	glyph_w: usize,
	glyph_h: usize,
	page_px: usize,
	cols: usize,
	capacity: usize,
	/// Пиксели страницы, `page_px² × CHANNELS` байт, row-major, каналы подряд.
	cells: Vec<u8>,
	/// Слот каждой кодовой точки Unicode: `NOT_LOADED`, `EMPTY_SLOT` или слот ≥ 1.
	slot_by_codepoint: Vec<u16>,
	/// Кодовая точка каждого занятого слота.
	codepoint_of_slot: Vec<char>,
	/// Биты готовых каналов на слот.
	ready: Vec<u8>,
	/// Буфер растеризации одного глифа.
	scratch: Vec<u8>,
	/// Слоты, изменённые с прошлого `take_dirty_slots` (для загрузки в текстуру GL).
	dirty_slots: Vec<u16>,
	/// Счётчик сбросов страницы; по нему рендерер узнаёт, что старые слоты недействительны.
	resets: u32,
}

impl Atlas {
	/// Создать пустую страницу `page_px × page_px` под семейство растеризатора.
	pub fn new(rasterizer: Rasterizer, page_px: u32) -> Self {
		let m = rasterizer.metrics();
		let page_px = page_px as usize;
		let cols = (page_px / m.glyph_w).max(1);
		let rows = (page_px / m.glyph_h).max(1);
		let capacity = cols * rows;
		let mut atlas = Self {
			rasterizer: Box::new(rasterizer),
			glyph_w: m.glyph_w,
			glyph_h: m.glyph_h,
			page_px,
			cols,
			capacity,
			cells: vec![0; page_px * page_px * CHANNELS],
			slot_by_codepoint: vec![NOT_LOADED; CODEPOINT_COUNT],
			codepoint_of_slot: Vec::with_capacity(capacity),
			ready: Vec::with_capacity(capacity),
			scratch: vec![0; m.glyph_w * m.glyph_h],
			dirty_slots: Vec::new(),
			resets: 0,
		};
		atlas.clear_page();
		atlas
	}

	pub fn glyph_w(&self) -> usize {
		self.glyph_w
	}

	pub fn glyph_h(&self) -> usize {
		self.glyph_h
	}

	/// Физический размер растеризации, под который построена страница.
	pub fn size_px(&self) -> u16 {
		self.rasterizer.size_px()
	}

	/// Пересобрать страницу под растеризатор другого размера (ADR-0005 п.7): хинтеры уже
	/// пересобраны в нём, сетка слотов пересчитывается, все слоты становятся неготовыми.
	///
	/// Пиксели страницы (16 МБ) переиспользуются, а не перевыделяются: меняется только
	/// `scratch`. Счётчик сбросов растёт — рендерер по нему перезаливает текстуру целиком
	/// (в том числе при меньшем размере глифа, когда часть страницы залита нулями).
	pub fn set_rasterizer(&mut self, rasterizer: Rasterizer) {
		self.apply_rasterizer(rasterizer);
	}

	/// Сменить физический размер растеризации на месте — путь смены DPI (ADR-0005 п.7).
	///
	/// Семейство остаётся прежним, перестраиваются только хинтеры и сетка слотов, поэтому
	/// страница не перевыделяется. Тот же размер — ничего не делает: слоты и пиксели страницы
	/// остаются в силе, счётчик сбросов не растёт. Ошибка оставляет атлас как был.
	pub fn set_size(&mut self, size_px: u16) -> Result<(), FontError> {
		if self.size_px() == size_px {
			return Ok(());
		}
		self.rasterizer.set_size(size_px)?;
		self.regrid(self.rasterizer.metrics());
		self.clear_page();
		Ok(())
	}

	/// Источник псевдографики (ADR-0018): прежние слоты рамок устарели — страница сбрасывается,
	/// как при смене размера; тот же режим — без изменений.
	pub fn set_box_drawing_mode(&mut self, mode: BoxDrawingMode) {
		if self.rasterizer.box_drawing_mode() == mode {
			return;
		}
		self.rasterizer.set_box_drawing_mode(mode);
		self.clear_page();
	}

	pub fn box_drawing_mode(&self) -> BoxDrawingMode {
		self.rasterizer.box_drawing_mode()
	}

	/// Общая часть смены растеризатора: сетка слотов, `scratch`, сброс страницы.
	fn apply_rasterizer(&mut self, rasterizer: Rasterizer) {
		let m = rasterizer.metrics();
		*self.rasterizer = rasterizer;
		self.regrid(m);
		self.clear_page();
	}

	/// Пересчитать геометрию слотов под новую ячейку, не трогая пиксели страницы.
	fn regrid(&mut self, m: CellMetrics) {
		self.glyph_w = m.glyph_w;
		self.glyph_h = m.glyph_h;
		self.cols = (self.page_px / self.glyph_w).max(1);
		self.capacity = self.cols * (self.page_px / self.glyph_h).max(1);
		self.scratch.clear();
		self.scratch.resize(self.glyph_w * self.glyph_h, 0);
	}

	/// Сторона страницы в пикселях (ширина = высота).
	pub fn page_px(&self) -> usize {
		self.page_px
	}

	/// Максимум слотов на странице (включая пустой слот 0).
	pub fn capacity(&self) -> usize {
		self.capacity
	}

	/// Занятых слотов, включая слот 0.
	pub fn used_slots(&self) -> usize {
		self.codepoint_of_slot.len()
	}

	/// Пиксели страницы `RGBA8` для загрузки в текстуру.
	pub fn cells(&self) -> &[u8] {
		&self.cells
	}

	pub fn rasterizer(&self) -> &Rasterizer {
		&self.rasterizer
	}

	/// Забрать растеризатор обратно (откат на другой бэкенд с новой страницей).
	pub fn into_rasterizer(self) -> Rasterizer {
		*self.rasterizer
	}

	/// Сколько раз страница сбрасывалась; изменение значения между кадрами = все слоты заново.
	pub fn resets(&self) -> u32 {
		self.resets
	}

	/// Слот кодовой точки, если он уже загружен (без растеризации).
	pub fn slot_of(&self, codepoint: char) -> Option<u16> {
		match self.slot_by_codepoint[codepoint as usize] {
			NOT_LOADED => None,
			slot => Some(slot),
		}
	}

	/// Готов ли канал начертания в слоте.
	pub fn is_ready(&self, slot: u16, style: Style) -> bool {
		let channel = self.rasterizer.family().style_source(style).channel();
		self.ready
			.get(slot as usize)
			.is_some_and(|bits| bits & (1 << channel) != 0)
	}

	/// Слот с готовым каналом начертания; промах — растеризация (ADR-0003 п.1–2),
	/// нет места — сброс страницы и повтор (п.6). Всегда возвращает действительный слот.
	pub fn ensure(&mut self, codepoint: char, style: Style) -> u16 {
		if codepoint == '\0' {
			return EMPTY_SLOT;
		}
		let source = self.rasterizer.family().style_source(style);
		let channel = source.channel();
		let mut slot = self.slot_by_codepoint[codepoint as usize];
		if slot == NOT_LOADED {
			if !self.rasterizer.has_glyph(codepoint, source) {
				self.slot_by_codepoint[codepoint as usize] = EMPTY_SLOT;
				return EMPTY_SLOT;
			}
			if self.codepoint_of_slot.len() >= self.capacity {
				self.clear_page();
			}
			slot = self.codepoint_of_slot.len() as u16;
			self.codepoint_of_slot.push(codepoint);
			self.ready.push(0);
			self.slot_by_codepoint[codepoint as usize] = slot;
		}
		if slot == EMPTY_SLOT || self.ready[slot as usize] & (1 << channel) != 0 {
			return slot;
		}
		self.fill_channel(slot, codepoint, source);
		slot
	}

	/// Покрытие пикселя `(x, y)` слота в канале начертания; O(1), без аллокаций (NFR-6).
	// Горячий путь: при `opt-level = "z"` без принудительного встраивания вызов на пиксель
	// стоит до трети кадра Software (замер, ADR-0014).
	#[inline(always)]
	pub fn pixel(&self, slot: u16, x: usize, y: usize, style: Style) -> u8 {
		debug_assert!(x < self.glyph_w && y < self.glyph_h);
		let channel = self.rasterizer.family().style_source(style).channel();
		let (ox, oy) = self.slot_origin(slot);
		self.cells[((oy + y) * self.page_px + ox + x) * CHANNELS + channel]
	}

	/// Пиксельное положение слота на странице.
	#[inline(always)]
	pub fn slot_origin(&self, slot: u16) -> (usize, usize) {
		let slot = slot as usize;
		(
			(slot % self.cols) * self.glyph_w,
			(slot / self.cols) * self.glyph_h,
		)
	}

	/// Забрать список слотов, изменённых с прошлого вызова (для `glTexSubImage2D`).
	pub fn take_dirty_slots(&mut self) -> Vec<u16> {
		std::mem::take(&mut self.dirty_slots)
	}

	/// Копия растра глифа в начертании, `glyph_w × glyph_h` (тесты и отладка; аллоцирует).
	pub fn glyph(&mut self, codepoint: char, style: Style) -> Vec<u8> {
		let slot = self.ensure(codepoint, style);
		self.glyph_bytes(slot, style)
	}

	/// Копия растра слота в канале начертания без растеризации (тесты и отладка; аллоцирует).
	pub fn glyph_bytes(&self, slot: u16, style: Style) -> Vec<u8> {
		(0..self.glyph_h)
			.flat_map(|y| (0..self.glyph_w).map(move |x| (x, y)))
			.map(|(x, y)| self.pixel(slot, x, y, style))
			.collect()
	}

	fn clear_page(&mut self) {
		if !self.codepoint_of_slot.is_empty() {
			self.resets += 1;
		}
		self.cells.fill(0);
		self.slot_by_codepoint.fill(NOT_LOADED);
		self.codepoint_of_slot.clear();
		self.ready.clear();
		self.dirty_slots.clear();
		// Слот 0 — пустой, во всех каналах готов.
		self.codepoint_of_slot.push('\0');
		self.ready.push(0b1111);
		self.slot_by_codepoint[0] = EMPTY_SLOT;
	}

	/// Растеризовать канал слота; SVG-глифы и псевдографика одинаковы во всех каналах — заполняет все.
	fn fill_channel(&mut self, slot: u16, codepoint: char, source: Style) {
		let drawn = self
			.rasterizer
			.rasterize(codepoint, source, &mut self.scratch);
		let channels: u8 = if self.rasterizer.is_style_independent(codepoint) {
			0b1111
		} else {
			1 << source.channel()
		};
		if drawn {
			let (ox, oy) = self.slot_origin(slot);
			for y in 0..self.glyph_h {
				for x in 0..self.glyph_w {
					let v = self.scratch[y * self.glyph_w + x];
					let base = ((oy + y) * self.page_px + ox + x) * CHANNELS;
					for c in 0..CHANNELS {
						if channels & (1 << c) != 0 {
							self.cells[base + c] = v;
						}
					}
				}
			}
			self.dirty_slots.push(slot);
		}
		self.ready[slot as usize] |= channels;
	}
}
