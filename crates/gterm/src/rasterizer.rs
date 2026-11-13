//! Растеризация одного глифа в буфер ячейки (ТЗ FR-8.4, ADR-0002, ADR-0003).
//!
//! `skrifa` даёт контур с хинтингом (нативные инструкции или автохинтер), `zeno` — покрытие
//! 0…255 прямо в предвыделенный буфер `glyph_w × glyph_h`. Псевдографика (рамки, мозаики, фигуры) —
//! по режиму [`BoxDrawingMode`] (ADR-0018): процедурно ([`crate::procedural`], умолчание), из
//! шрифта или комбинированно. SVG-глифы ([`crate::svg`]) главнее псевдографики и шрифта
//! (ADR-0008). Путь промаха кэша: аллокации только при первом вызове.

use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{
	DrawSettings, Engine, HintingInstance, HintingOptions, OutlineGlyphCollection, OutlinePen,
	SmoothMode, Target,
};
use skrifa::{FontRef, MetadataProvider};

use crate::cell::Style;
use crate::font::{CellMetrics, FONT_SIZE_PX, FontError, FontFamily, has_glyph_instructions};
use crate::procedural;
use crate::svg::SvgGlyphs;

/// Источник псевдографики — рамок, мозаик и фигур (ADR-0018, ADR-0022).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BoxDrawingMode {
	/// Формулы [`crate::procedural`] перекрывают шрифт (FR-8.4, ADR-0002) — умолчание.
	#[default]
	Procedural,
	/// Глиф шрифта; нет глифа — пустая ячейка, как у любого символа.
	Font,
	/// Глиф шрифта, если он есть, иначе формула.
	Combined,
}

/// Контур глифа в командах `zeno` (y вверх, как отдаёт `skrifa`); буфер переиспользуется.
struct PathPen(Vec<zeno::Command>);

impl OutlinePen for PathPen {
	fn move_to(&mut self, x: f32, y: f32) {
		self.0.push(zeno::Command::MoveTo((x, y).into()));
	}
	fn line_to(&mut self, x: f32, y: f32) {
		self.0.push(zeno::Command::LineTo((x, y).into()));
	}
	fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
		self.0
			.push(zeno::Command::QuadTo((cx, cy).into(), (x, y).into()));
	}
	fn curve_to(&mut self, c0x: f32, c0y: f32, c1x: f32, c1y: f32, x: f32, y: f32) {
		self.0.push(zeno::Command::CurveTo(
			(c0x, c0y).into(),
			(c1x, c1y).into(),
			(x, y).into(),
		));
	}
	fn close(&mut self) {
		self.0.push(zeno::Command::Close);
	}
}

/// Подготовленное начертание: хинтер строится один раз (у автохинтера это дорого).
struct StyleState {
	hinter: HintingInstance,
}

/// Растеризатор семейства: по одному состоянию на начертание-источник.
///
/// Хинтеры строятся на конкретный размер, поэтому смена размера (DPI, `FONT_SIZE_PX`,
/// `FONT_SCALE`) требует перестроения хинтеров — [`Rasterizer::new_at`] для нового
/// растеризатора либо [`Rasterizer::set_size`] на месте (ADR-0005 п.7).
pub struct Rasterizer {
	family: FontFamily,
	metrics: CellMetrics,
	/// Размер растеризации, для которого построены хинтеры.
	size_px: u16,
	/// Состояние по каналу; для подставленных начертаний — `None`, берётся источник.
	states: [Option<StyleState>; 4],
	pen: PathPen,
	/// SVG-глифы; переживают смену размера и откат бэкенда вместе с растеризатором.
	svg: SvgGlyphs,
	/// Источник псевдографики (ADR-0018); переживает смену размера и бэкенда так же.
	box_drawing: BoxDrawingMode,
}

impl Rasterizer {
	/// Подготовить хинтеры для всех реально присутствующих начертаний при `FONT_SIZE_PX`.
	pub fn new(family: FontFamily) -> Result<Self, FontError> {
		Self::new_at(family, FONT_SIZE_PX)
	}

	/// То же, но при заданном физическом размере растеризации (ADR-0005).
	///
	/// Метрики берутся из файла Regular тем же расчётом, что и при загрузке, поэтому ячейка
	/// всегда совпадает с сеткой слотов атласа, построенной из метрик растеризатора.
	pub fn new_at(family: FontFamily, size_px: u16) -> Result<Self, FontError> {
		let metrics = family.metrics_at(size_px)?;
		let states = Self::build_states(&family, size_px)?;
		Ok(Self {
			family,
			metrics,
			size_px,
			states,
			pen: PathPen(Vec::with_capacity(256)),
			svg: SvgGlyphs::default(),
			box_drawing: BoxDrawingMode::default(),
		})
	}

	/// Подключить SVG-глифы (ADR-0008). Вызывается до создания атласа: уже растеризованные
	/// слоты не пересчитываются.
	pub fn set_svg_glyphs(&mut self, svg: SvgGlyphs) {
		self.svg = svg;
	}

	pub fn svg_glyphs(&self) -> &SvgGlyphs {
		&self.svg
	}

	/// Источник псевдографики (ADR-0018). Уже растеризованные слоты не пересчитываются —
	/// атлас сбрасывает страницу сам ([`crate::atlas::Atlas::set_box_drawing_mode`]).
	pub fn set_box_drawing_mode(&mut self, mode: BoxDrawingMode) {
		self.box_drawing = mode;
	}

	pub fn box_drawing_mode(&self) -> BoxDrawingMode {
		self.box_drawing
	}

	/// Одинаков ли растр во всех начертаниях: SVG-глифы и процедурная псевдографика не зависят
	/// от `BOLD`/`ITALIC` — атлас заполняет все каналы сразу. В режимах `Font` и `Combined`
	/// у начертаний свои глифы рамок (ADR-0018).
	pub fn is_style_independent(&self, codepoint: char) -> bool {
		self.svg.contains(codepoint)
			|| (self.box_drawing == BoxDrawingMode::Procedural
				&& procedural::is_procedural(codepoint))
	}

	/// Есть ли глиф у шрифта или псевдографики, без учёта SVG — для поиска перекрытий. В режиме
	/// `Font` псевдографика есть только у шрифта (ADR-0018).
	pub fn has_font_glyph(&self, codepoint: char, style: Style) -> bool {
		if procedural::is_procedural(codepoint) && self.box_drawing != BoxDrawingMode::Font {
			return true;
		}
		let source = self.family.style_source(style);
		FontRef::new(&self.family.face(source).bytes)
			.map(|font| font.charmap().map(codepoint).is_some())
			.unwrap_or(false)
	}

	/// По одному хинтеру на реально присутствующее начертание; подставленные пропускаются.
	fn build_states(
		family: &FontFamily,
		size_px: u16,
	) -> Result<[Option<StyleState>; 4], FontError> {
		let size = Size::new(size_px as f32);
		let mut states = [None, None, None, None];
		for style in Style::ALL {
			if family.style_source(style) != style {
				continue;
			}
			let bytes = &family.face(style).bytes;
			let font = FontRef::new(bytes).map_err(|_| FontError::TtfParse("not a valid font"))?;
			let outlines = font.outline_glyphs();
			let engine = hinting_engine(&font, &outlines);
			let options = HintingOptions {
				target: hinting_target(&engine),
				engine,
			};
			let hinter = HintingInstance::new(&outlines, size, LocationRef::default(), options)
				.map_err(|_| FontError::TtfParse("cannot build hinting instance"))?;
			states[style.channel()] = Some(StyleState { hinter });
		}
		Ok(states)
	}

	/// Перестроить хинтеры под другой физический размер на месте (ADR-0005 п.7): семейство и
	/// буфер пера остаются прежними, поэтому смена DPI не требует пересборки атласа.
	///
	/// Размер, равный текущему, — ничего не делает; остальное меняет метрики ячейки, то есть
	/// всю геометрию кадра (FR-10, FR-13). Ошибка оставляет растеризатор как был: метрики
	/// считаются первыми, хинтеры — вторыми, а поля меняются только после успеха.
	pub fn set_size(&mut self, size_px: u16) -> Result<(), FontError> {
		if size_px == self.size_px {
			return Ok(());
		}
		let metrics = self.family.metrics_at(size_px)?;
		let states = Self::build_states(&self.family, size_px)?;
		self.metrics = metrics;
		self.states = states;
		self.size_px = size_px;
		Ok(())
	}

	pub fn family(&self) -> &FontFamily {
		&self.family
	}

	pub fn metrics(&self) -> CellMetrics {
		self.metrics
	}

	/// Физический размер растеризации, для которого построены хинтеры.
	pub fn size_px(&self) -> u16 {
		self.size_px
	}

	/// Забрать семейство обратно (нужно для пересборки растеризатора на другом размере).
	pub fn into_family(self) -> FontFamily {
		self.family
	}

	/// Есть ли у кодовой точки глиф: SVG, процедурный диапазон или начертание-источник шрифта.
	pub fn has_glyph(&self, codepoint: char, style: Style) -> bool {
		self.svg.contains(codepoint) || self.has_font_glyph(codepoint, style)
	}

	/// Растеризовать глиф в `out` (`glyph_w × glyph_h`, row-major, покрытие 0…255).
	/// Возвращает `true`, если что-то нарисовано (пробел и отсутствующий глиф дают `false`, буфер обнулён).
	pub fn rasterize(&mut self, codepoint: char, style: Style, out: &mut [u8]) -> bool {
		let (w, h) = (self.metrics.glyph_w, self.metrics.glyph_h);
		debug_assert_eq!(out.len(), w * h);
		if self.svg.contains(codepoint) {
			return self.svg.rasterize(codepoint, w, h, out);
		}
		let is_box = procedural::is_procedural(codepoint);
		if is_box
			&& self.box_drawing == BoxDrawingMode::Procedural
			&& procedural::draw(codepoint, w, h, out)
		{
			return true;
		}
		// Рамки из шрифта вписываются в ячейку (ADR-0020): путь шрифта для них идёт только в
		// `Font`/`Combined`.
		let drawn = self.rasterize_font_glyph(codepoint, style, is_box, out);
		if !drawn && is_box && self.box_drawing == BoxDrawingMode::Combined {
			return procedural::draw(codepoint, w, h, out);
		}
		drawn
	}

	/// Контур шрифта начертания-источника через его хинтер в `out`; `false` — глифа нет или
	/// он пуст (буфер обнулён). `fit_to_cell` — вписать контур в ячейку (ADR-0020).
	fn rasterize_font_glyph(
		&mut self,
		codepoint: char,
		style: Style,
		fit_to_cell: bool,
		out: &mut [u8],
	) -> bool {
		let (w, h) = (self.metrics.glyph_w, self.metrics.glyph_h);
		out.fill(0);
		let source = self.family.style_source(style);
		let Some(state) = &self.states[source.channel()] else {
			return false;
		};
		let Ok(font) = FontRef::new(&self.family.face(source).bytes) else {
			return false;
		};
		let outlines = font.outline_glyphs();
		let Some(gid) = font.charmap().map(codepoint) else {
			return false;
		};
		let Some(glyph) = outlines.get(gid) else {
			return false;
		};
		self.pen.0.clear();
		// Вписываемый контур берётся без хинтинга (ADR-0020): хинтер округляет концы штрихов к
		// пиксельной сетке номинального размера, и после пересчёта под ячейку они не доходят до её
		// края.
		let settings = if fit_to_cell {
			DrawSettings::unhinted(Size::new(self.size_px as f32), LocationRef::default())
		} else {
			DrawSettings::hinted(&state.hinter, false)
		};
		if glyph.draw(settings, &mut self.pen).is_err() || self.pen.0.is_empty() {
			return false;
		}
		if fit_to_cell {
			let size = Size::new(self.size_px as f32);
			let location = LocationRef::default();
			let line = font.metrics(size, location);
			let advance = font.glyph_metrics(size, location).advance_width(gid);
			fit_commands_to_cell(
				&mut self.pen.0,
				advance,
				line.ascent,
				line.descent,
				w,
				h,
				self.metrics.baseline,
			);
			snap_commands_to_cell_grid(&mut self.pen.0, w, h, self.metrics.baseline);
		}
		// Явный размер маски = ячейка; при `BottomLeft` ось y вверх, базовая линия — на
		// `h − baseline` от низа. Всё вне ячейки zeno обрезает сам.
		zeno::Mask::new(&self.pen.0[..])
			.size(w as u32, h as u32)
			.origin(zeno::Origin::BottomLeft)
			.offset(zeno::Vector::new(
				0.0,
				(h as i32 - self.metrics.baseline) as f32,
			))
			.render_into(out, None);
		out.iter().any(|&v| v > 0)
	}
}

/// Вписать контур рамки в ячейку (ADR-0020): по x — advance → ширина ячейки, по y — строка
/// `descent…ascent` → высота ячейки. Координаты пера: пиксели, y вверх, базовая линия в 0,
/// низ ячейки — `baseline − h`. Негодные метрики (advance ≤ 0, `ascent ≤ descent`) — без
/// изменений.
fn fit_commands_to_cell(
	commands: &mut [zeno::Command],
	advance: Option<f32>,
	ascent: f32,
	descent: f32,
	w: usize,
	h: usize,
	baseline: i32,
) {
	let Some(advance) = advance.filter(|a| *a > 0.0) else {
		return;
	};
	let line_h = ascent - descent;
	if line_h <= 0.0 {
		return;
	}
	let sx = w as f32 / advance;
	let sy = h as f32 / line_h;
	let bottom = (baseline - h as i32) as f32;
	for_each_point(commands, |p| {
		p.x *= sx;
		p.y = bottom + (p.y - descent) * sy;
	});
}

/// Выровнять вписанный контур рамки по пиксельной сетке ячейки (ADR-0021): по каждой оси
/// различимые координаты точек отображаются в целые монотонно — ни один штрих не схлопывается
/// и не сливается с соседним; результат зажат в ячейку (`0…w`, `baseline − h…baseline`).
fn snap_commands_to_cell_grid(commands: &mut [zeno::Command], w: usize, h: usize, baseline: i32) {
	let mut xs = Vec::with_capacity(16);
	let mut ys = Vec::with_capacity(16);
	for_each_point(commands, |p| {
		xs.push(p.x);
		ys.push(p.y);
	});
	let x_map = snapped_axis(xs, 0.0, w as f32);
	let y_map = snapped_axis(ys, (baseline - h as i32) as f32, baseline as f32);
	for_each_point(commands, |p| {
		p.x = lookup(&x_map, p.x);
		p.y = lookup(&y_map, p.y);
	});
}

/// Две координаты различимы, если отличаются больше чем на 1/64 px.
const SNAP_EPSILON: f32 = 1.0 / 64.0;

/// Отображение различимых координат в целые: округление к ближайшему, затем сдвиг вверх до
/// строго возрастающих с шагом ≥ 1, при выходе за `max` — сдвиг вниз от края, затем зажим в
/// `[min, max]`. Возвращает пары `(исходная, выровненная)` по возрастанию исходной.
fn snapped_axis(mut values: Vec<f32>, min: f32, max: f32) -> Vec<(f32, f32)> {
	values.sort_by(f32::total_cmp);
	values.dedup_by(|a, b| (*a - *b).abs() <= SNAP_EPSILON);
	let mut snapped: Vec<f32> = values.iter().map(|v| v.round()).collect();
	for i in 1..snapped.len() {
		if snapped[i] <= snapped[i - 1] {
			snapped[i] = snapped[i - 1] + 1.0;
		}
	}
	for i in (0..snapped.len()).rev() {
		let limit = if i + 1 < snapped.len() {
			snapped[i + 1] - 1.0
		} else {
			max
		};
		if snapped[i] > limit {
			snapped[i] = limit;
		}
	}
	values
		.into_iter()
		.zip(snapped)
		.map(|(v, s)| (v, s.clamp(min, max)))
		.collect()
}

/// Выровненное значение координаты: ближайшая исходная в таблице (в пределах порога).
fn lookup(map: &[(f32, f32)], value: f32) -> f32 {
	let i = map.partition_point(|(v, _)| *v < value - SNAP_EPSILON);
	map.get(i).map_or(value.round(), |(_, s)| *s)
}

fn for_each_point(commands: &mut [zeno::Command], mut f: impl FnMut(&mut zeno::Vector)) {
	for command in commands {
		match command {
			zeno::Command::MoveTo(p) | zeno::Command::LineTo(p) => f(p),
			zeno::Command::QuadTo(c, p) => {
				f(c);
				f(p);
			}
			zeno::Command::CurveTo(c0, c1, p) => {
				f(c0);
				f(c1);
				f(p);
			}
			zeno::Command::Close => {}
		}
	}
}

/// Движок хинтинга. `AutoFallback` у skrifa выбирает интерпретатор уже при наличии `prep`, даже если
/// в глифах нет ни одной инструкции (так у Nerd-сборок) — тогда контуры остаются сырыми. Поэтому:
/// глифы без инструкций ([`has_glyph_instructions`]) → автохинтер, иначе — интерпретатор.
fn hinting_engine(font: &FontRef<'_>, outlines: &OutlineGlyphCollection<'_>) -> Engine {
	if has_glyph_instructions(font) && outlines.prefer_interpreter() {
		Engine::Interpreter
	} else {
		Engine::Auto(None)
	}
}

/// Цель хинтинга по движку (ADR-0025). Интерпретатор — `Mono` (ADR-0002): у инструктированных
/// шрифтов он даёт штрихи 1 px. Автохинтер в `Mono` (аналог `FT_LOAD_TARGET_MONO`, рассчитан на
/// 1-битный растр) привязывает стойки по обеим осям и расширяет их до 2 px, сжимая буквы;
/// `Smooth/Normal` лишь приводит ширину стойки к целым пикселям, сохраняя пропорции.
fn hinting_target(engine: &Engine) -> Target {
	if matches!(engine, Engine::Interpreter) {
		Target::Mono
	} else {
		SmoothMode::Normal.into()
	}
}
