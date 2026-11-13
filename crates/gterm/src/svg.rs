//! SVG-глифы (ТЗ FR-8.6, ADR-0008): изображения из каталога SVG-глифов дополняют набор глифов шрифта.
//!
//! Имя файла — HEX-код кодовой точки и `.svg` (`E000.svg`, `F0001.svg`, `100000.svg`: 4–6 цифр).
//! Файл разбирается `usvg` один раз при старте в контуры на холсте SVG (`transform`, `use` и
//! фигуры уже применены, обводки переведены в заливку). При промахе атласа холст вписывается в
//! ячейку с сохранением пропорций и растеризуется `zeno` — как глиф шрифта. Цвета и прозрачность
//! файла игнорируются: всё закрашенное или обведённое даёт покрытие; `fill="none"` и
//! `stroke="none"` учитываются. Растровые `<image>`, текст, маски и обрезка не поддерживаются.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use usvg::tiny_skia_path::{self, PathSegment, PathStroker};
use zeno::{Command, Fill, Mask, Origin, Vector};

use crate::font::GLYPH_PX_MAX;

/// Расширение файла SVG-глифа.
const SVG_EXTENSION: &str = ".svg";
/// Допустимая длина HEX-кода в имени: U+0000…U+10FFFF.
const HEX_DIGITS: std::ops::RangeInclusive<usize> = 4..=6;

/// Кодовая точка из имени файла: ровно 4–6 HEX-цифр и `.svg` (регистр не важен). Суррогаты и
/// значения больше U+10FFFF — `None`.
pub fn codepoint_from_file_name(name: &str) -> Option<char> {
	let split = name.len().checked_sub(SVG_EXTENSION.len())?;
	let (hex, extension) = (name.get(..split)?, name.get(split..)?);
	let is_hex_code = HEX_DIGITS.contains(&hex.len()) && hex.bytes().all(|b| b.is_ascii_hexdigit());
	if !extension.eq_ignore_ascii_case(SVG_EXTENSION) || !is_hex_code {
		return None;
	}
	u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
}

/// Ошибка загрузки одного SVG-файла; файл пропускается (NFR-7).
#[derive(Debug)]
pub enum SvgError {
	Read(io::Error),
	Parse(usvg::Error),
}

impl fmt::Display for SvgError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Read(e) => write!(f, "cannot read: {e}"),
			Self::Parse(e) => write!(f, "cannot parse: {e}"),
		}
	}
}

impl std::error::Error for SvgError {}

/// Контур одного пути на холсте SVG (ось y вниз).
struct SvgPath {
	rule: Fill,
	commands: Vec<Command>,
}

/// Разобранный SVG-глиф: размер холста и контуры.
struct SvgGlyph {
	ch: char,
	width: f32,
	height: f32,
	paths: Vec<SvgPath>,
}

/// Набор SVG-глифов с буферами растеризации (переиспользуются между промахами атласа).
#[derive(Default)]
pub struct SvgGlyphs {
	/// По возрастанию кодовой точки — поиск двоичный.
	glyphs: Vec<SvgGlyph>,
	commands: Vec<Command>,
	coverage: Vec<u8>,
}

impl SvgGlyphs {
	/// Загрузить все `*.svg` каталога. Каталога нет — пустой набор. Файл с неверным именем,
	/// повтор кодовой точки или ошибка разбора — строка в stderr и пропуск файла.
	pub fn load(dir: &Path) -> Self {
		let mut set = Self::default();
		let entries = match fs::read_dir(dir) {
			Ok(entries) => entries,
			Err(e) if e.kind() == io::ErrorKind::NotFound => return set,
			Err(e) => {
				eprintln!("gterm: svg: {}: {e}", dir.display());
				return set;
			}
		};
		let mut files: Vec<PathBuf> = entries
			.filter_map(Result::ok)
			.map(|entry| entry.path())
			.filter(|path| path.is_file())
			.collect();
		// Порядок `read_dir` не определён; при повторе кодовой точки побеждает первое имя.
		files.sort();
		for path in files {
			set.load_file(&path);
		}
		set.glyphs.sort_by_key(|glyph| glyph.ch);
		set
	}

	fn load_file(&mut self, path: &Path) {
		let name = path
			.file_name()
			.and_then(|name| name.to_str())
			.unwrap_or("");
		let Some(ch) = codepoint_from_file_name(name) else {
			eprintln!(
				"gterm: svg: {}: name is not a hex codepoint (XXXX.svg … XXXXXX.svg), skipped",
				path.display()
			);
			return;
		};
		// Набор ещё не отсортирован — двоичный поиск `contains` здесь неприменим.
		if self.glyphs.iter().any(|glyph| glyph.ch == ch) {
			eprintln!(
				"gterm: svg: {}: U+{:04X} already loaded, skipped",
				path.display(),
				u32::from(ch)
			);
			return;
		}
		match fs::read(path)
			.map_err(SvgError::Read)
			.and_then(|data| parse(ch, &data))
		{
			Ok(glyph) => self.glyphs.push(glyph),
			Err(e) => eprintln!("gterm: svg: {}: {e}, skipped", path.display()),
		}
	}

	/// Число загруженных глифов.
	pub fn len(&self) -> usize {
		self.glyphs.len()
	}

	pub fn is_empty(&self) -> bool {
		self.glyphs.is_empty()
	}

	/// Есть ли SVG-глиф у кодовой точки.
	pub fn contains(&self, ch: char) -> bool {
		self.find(ch).is_some()
	}

	/// Кодовые точки загруженных глифов по возрастанию.
	pub fn codepoints(&self) -> impl Iterator<Item = char> + '_ {
		self.glyphs.iter().map(|glyph| glyph.ch)
	}

	fn find(&self, ch: char) -> Option<&SvgGlyph> {
		self.glyphs
			.binary_search_by_key(&ch, |glyph| glyph.ch)
			.ok()
			.map(|index| &self.glyphs[index])
	}

	/// Растеризовать глиф в `out` (`w × h`, покрытие 0…255): холст вписывается с сохранением
	/// пропорций и центрируется, пути объединяются по максимуму покрытия. `false` — SVG-глифа
	/// нет (буфер не тронут) или он ничего не нарисовал (буфер обнулён).
	pub fn rasterize(&mut self, ch: char, w: usize, h: usize, out: &mut [u8]) -> bool {
		let Self {
			glyphs,
			commands,
			coverage,
		} = self;
		let Ok(index) = glyphs.binary_search_by_key(&ch, |glyph| glyph.ch) else {
			return false;
		};
		let glyph = &glyphs[index];
		out[..w * h].fill(0);
		let scale = (w as f32 / glyph.width).min(h as f32 / glyph.height);
		let offset = Vector::new(
			(w as f32 - glyph.width * scale) / 2.0,
			(h as f32 - glyph.height * scale) / 2.0,
		);
		coverage.resize(w * h, 0);
		for path in &glyph.paths {
			commands.clear();
			commands.extend(path.commands.iter().map(|&c| place(c, scale, offset)));
			coverage.fill(0);
			Mask::new(&commands[..])
				.size(w as u32, h as u32)
				.origin(Origin::TopLeft)
				.style(path.rule)
				.render_into(coverage, None);
			for (dst, &src) in out.iter_mut().zip(coverage.iter()) {
				*dst = (*dst).max(src);
			}
		}
		out[..w * h].iter().any(|&v| v > 0)
	}
}

/// Разобрать SVG в контуры на холсте `tree.size()`.
fn parse(ch: char, data: &[u8]) -> Result<SvgGlyph, SvgError> {
	let tree = usvg::Tree::from_data(data, &usvg::Options::default()).map_err(SvgError::Parse)?;
	let size = tree.size();
	let mut paths = Vec::new();
	collect_paths(tree.root(), size.width().max(size.height()), &mut paths);
	Ok(SvgGlyph {
		ch,
		width: size.width(),
		height: size.height(),
		paths,
	})
}

fn collect_paths(group: &usvg::Group, canvas_max: f32, out: &mut Vec<SvgPath>) {
	for node in group.children() {
		match node {
			usvg::Node::Group(group) => collect_paths(group, canvas_max, out),
			usvg::Node::Path(path) if path.is_visible() => push_path(path, canvas_max, out),
			_ => {}
		}
	}
}

/// Заливка и обводка пути — отдельные контуры: у заливки правило файла, обводка уже переведена
/// в замкнутый контур и заливается `NonZero`.
fn push_path(path: &usvg::Path, canvas_max: f32, out: &mut Vec<SvgPath>) {
	let transform = path.abs_transform();
	if let Some(fill) = path.fill() {
		if let Some(outline) = path.data().clone().transform(transform) {
			let rule = match fill.rule() {
				usvg::FillRule::NonZero => Fill::NonZero,
				usvg::FillRule::EvenOdd => Fill::EvenOdd,
			};
			out.push(SvgPath {
				rule,
				commands: to_commands(&outline),
			});
		}
	}
	if let Some(stroke) = path.stroke() {
		// Точность обводки — под самую крупную ячейку: холст вписывается не больше чем в
		// `GLYPH_PX_MAX` пикселей по большей стороне.
		let resolution = PathStroker::compute_resolution_scale(&transform)
			* f32::from(GLYPH_PX_MAX)
			/ canvas_max;
		let stroke = stroke.to_tiny_skia();
		let dashed = match &stroke.dash {
			Some(dash) => path.data().dash(dash, resolution),
			None => Some(path.data().clone()),
		};
		if let Some(outline) = dashed
			.and_then(|data| data.stroke(&stroke, resolution))
			.and_then(|data| data.transform(transform))
		{
			out.push(SvgPath {
				rule: Fill::NonZero,
				commands: to_commands(&outline),
			});
		}
	}
}

fn to_commands(path: &tiny_skia_path::Path) -> Vec<Command> {
	let point = |p: tiny_skia_path::Point| Vector::new(p.x, p.y);
	path.segments()
		.map(|segment| match segment {
			PathSegment::MoveTo(p) => Command::MoveTo(point(p)),
			PathSegment::LineTo(p) => Command::LineTo(point(p)),
			PathSegment::QuadTo(c, p) => Command::QuadTo(point(c), point(p)),
			PathSegment::CubicTo(c0, c1, p) => Command::CurveTo(point(c0), point(c1), point(p)),
			PathSegment::Close => Command::Close,
		})
		.collect()
}

/// Перенести точку команды с холста SVG в ячейку.
fn place(command: Command, scale: f32, offset: Vector) -> Command {
	let at = |v: Vector| Vector::new(v.x * scale + offset.x, v.y * scale + offset.y);
	match command {
		Command::MoveTo(p) => Command::MoveTo(at(p)),
		Command::LineTo(p) => Command::LineTo(at(p)),
		Command::QuadTo(c, p) => Command::QuadTo(at(c), at(p)),
		Command::CurveTo(c0, c1, p) => Command::CurveTo(at(c0), at(c1), at(p)),
		Command::Close => Command::Close,
	}
}
