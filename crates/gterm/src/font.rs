//! Семейство шрифтов (ТЗ FR-8, ADR-0003, ADR-0017): четыре начертания из каталога, списка файлов
//! или системных шрифтов ([`FontFamily::system`], [`FontFamily::system_monospace`]), подстановка
//! недостающих, проверка моноширинности, метрики ячейки. Имя семейства и
//! каталоги задаёт приложение — библиотека ничего не ищет относительно exe. Растеризация —
//! [`crate::rasterizer`], атлас — [`crate::atlas`].

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use skrifa::instance::{LocationRef, Size};
use skrifa::raw::TableProvider;
use skrifa::string::StringId;
use skrifa::{FontRef, MetadataProvider};

use crate::cell::Style;

mod system;

/// Логический размер шрифта в пикселях (FR-8, ADR-0005): физический при `scale_factor = 1`.
pub const FONT_SIZE_PX: u16 = 16;
/// Множитель размера, задаваемый пользователем (FR-8, ADR-0005). В 1.0 — константа: зума нет
/// (FR-28), точка подключения будущего файла настроек.
pub const FONT_SCALE: f32 = 1.0;
/// Нижняя граница физического размера растеризации: меньше ячейка нечитаема.
pub const GLYPH_PX_MIN: u16 = 8;
/// Верхняя граница физического размера растеризации (ADR-0005 п.4): при 64 px страница 1024
/// вмещала бы 16×16 глифов — меньше, чем нужно сцене «таблица всех глифов» (FR-27).
pub const GLYPH_PX_MAX: u16 = 48;

/// Физический размер растеризации из логического размера, множителя пользователя и DPI
/// (FR-8, FR-24): `clamp(round(base · scale · scale_factor), 8, 48)`.
///
/// Значения, не являющиеся конечным положительным числом (`NaN`, `inf`, `0`, отрицательные),
/// трактуются как отсутствие множителя: берётся `base` без округления и зажимается диапазоном.
pub fn glyph_px_for(base: u16, scale: f32, scale_factor: f64) -> u16 {
	let product = f64::from(base) * f64::from(scale) * scale_factor;
	if !product.is_finite() || product <= 0.0 {
		return base.clamp(GLYPH_PX_MIN, GLYPH_PX_MAX);
	}
	(product.round() as u64).clamp(u64::from(GLYPH_PX_MIN), u64::from(GLYPH_PX_MAX)) as u16
}

/// Размер растеризации для окна с данным DPI: `FONT_SIZE_PX · FONT_SCALE · scale_factor`.
pub fn glyph_px(scale_factor: f64) -> u16 {
	glyph_px_for(FONT_SIZE_PX, FONT_SCALE, scale_factor)
}
/// Вес, начиная с которого начертание считается жирным (`OS/2.usWeightClass`).
const BOLD_WEIGHT_MIN: f32 = 600.0;
/// Контрольный набор проверки моноширинности: advance каждого из этих глифов, если он есть в
/// шрифте, должен совпадать с advance `M` — шириной ячейки (FR-8).
const MONOSPACE_PROBE: [char; 5] = ['i', '.', 'W', '0', ' '];

/// Ошибки загрузки шрифта и растеризации.
#[derive(Debug)]
pub enum FontError {
	/// Ошибка ввода-вывода с путём.
	Io(PathBuf, std::io::Error),
	/// Среди файлов нет семейства `family` с начертанием Regular; `found` — что было найдено.
	NoRegularFace { family: String, found: Vec<String> },
	/// Начертание Regular не моноширинное: advance глифа `glyph` отличается от advance `M`
	/// (`expected`) при `FONT_SIZE_PX` (ADR-0017).
	NotMonospace {
		family: String,
		glyph: char,
		advance: f32,
		expected: f32,
	},
	/// Ни одно семейство-кандидат системного моноширинного шрифта не подошло; `tried` — причина
	/// по каждому (ADR-0017).
	NoSystemMonospace { tried: Vec<String> },
	/// Размер глифа вне допустимого диапазона 1–255.
	BadGlyphSize { glyph_w: usize, glyph_h: usize },
	/// Ошибка разбора TTF или подготовки растеризатора.
	TtfParse(&'static str),
}

impl fmt::Display for FontError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Io(path, err) => write!(f, "{}: {err}", path.display()),
			Self::NoRegularFace { family, found } => write!(
				f,
				"no Regular face of family '{family}' (found: {})",
				if found.is_empty() {
					"nothing".to_owned()
				} else {
					found.join(", ")
				}
			),
			Self::NotMonospace {
				family,
				glyph,
				advance,
				expected,
			} => write!(
				f,
				"family '{family}' is not monospace: advance of '{glyph}' is {advance} px, \
                 'M' is {expected} px at {FONT_SIZE_PX} px"
			),
			Self::NoSystemMonospace { tried } => {
				write!(f, "no system monospace font (tried: {})", tried.join("; "))
			}
			Self::BadGlyphSize { glyph_w, glyph_h } => {
				write!(f, "glyph size {glyph_w}x{glyph_h} out of range 1..=255")
			}
			Self::TtfParse(msg) => write!(f, "ttf: {msg}"),
		}
	}
}

impl std::error::Error for FontError {}

/// Один файл начертания.
#[derive(Debug)]
pub struct Face {
	/// Путь к файлу (для сообщений).
	pub path: PathBuf,
	/// Содержимое TTF; `FontRef` строится из него при каждой растеризации (дёшево).
	pub bytes: Vec<u8>,
	/// Начертание по `OS/2`.
	pub style: Style,
}

/// Метрики ячейки, общие для всех начертаний (по Regular).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellMetrics {
	/// Ширина глифа в пикселях (advance `M`).
	pub glyph_w: usize,
	/// Высота глифа в пикселях (ascent − descent).
	pub glyph_h: usize,
	/// Базовая линия — строк от верха ячейки.
	pub baseline: i32,
}

/// Семейство: загруженные начертания и таблица подстановки.
#[derive(Debug)]
pub struct FontFamily {
	faces: Vec<Face>,
	/// Имя семейства (`name` ID 1), как было запрошено при загрузке.
	family: String,
	/// Индекс файла для каждого начертания (после подстановки).
	face_of_style: [usize; 4],
	/// Из какого начертания реально берётся глиф: `style_source[Bold] == Regular`, если Bold нет.
	style_source: [Style; 4],
	metrics: CellMetrics,
	/// Размер, для которого собраны `metrics`.
	size_px: u16,
}

impl FontFamily {
	/// Загрузить семейство `family` из каталога: все `*.ttf`/`*.otf`, у которых имя семейства
	/// (`name` ID 1) равно `family`; файлы других семейств и не-шрифты пропускаются (FR-8.1);
	/// без Regular — ошибка (FR-8.5). Порядок файлов — по имени.
	pub fn load(fonts_dir: &Path, family: &str) -> Result<Self, FontError> {
		let entries =
			fs::read_dir(fonts_dir).map_err(|e| FontError::Io(fonts_dir.to_path_buf(), e))?;
		let mut paths: Vec<PathBuf> = entries
			.filter_map(Result::ok)
			.map(|e| e.path())
			.filter(|p| is_font_file(p))
			.collect();
		paths.sort();
		Self::from_files(&paths, family)
	}

	/// Собрать семейство `family` из заданных файлов: отбор тот же, что у [`Self::load`]
	/// (файл другого семейства или не-шрифт пропускается), порядок файлов сохраняется.
	pub fn from_files(paths: &[PathBuf], family: &str) -> Result<Self, FontError> {
		let mut faces = Vec::new();
		let mut found = Vec::new();
		for path in paths {
			let bytes = fs::read(path).map_err(|e| FontError::Io(path.clone(), e))?;
			let Ok(font) = FontRef::new(&bytes) else {
				continue; // не шрифт — пропускаем, как и файлы других семейств
			};
			let file_family = family_name(&font);
			let file_name = path
				.file_name()
				.and_then(|n| n.to_str())
				.unwrap_or("?")
				.to_owned();
			found.push(format!("{file_family} ({file_name})"));
			if file_family != family {
				continue;
			}
			let style = style_of(&font);
			faces.push(Face {
				path: path.clone(),
				bytes,
				style,
			});
		}
		Self::from_faces(faces, family, found)
	}

	/// Собрать семейство из уже прочитанных файлов (тесты, ручная сборка); `found` — список
	/// просмотренных файлов для сообщения об ошибке. Среди файлов одного начертания
	/// предпочитается тот, чьи глифы несут родные инструкции хинтинга (ADR-0025): его
	/// хинтует интерпретатор, а не автохинтер; среди равных — первый по порядку.
	pub fn from_faces(
		faces: Vec<Face>,
		family: &str,
		found: Vec<String>,
	) -> Result<Self, FontError> {
		let has_instructions: Vec<bool> = faces
			.iter()
			.map(|f| FontRef::new(&f.bytes).is_ok_and(|font| has_glyph_instructions(&font)))
			.collect();
		let index_of = |style: Style| {
			let of_style = || {
				faces
					.iter()
					.enumerate()
					.filter(move |(_, f)| f.style == style)
			};
			of_style()
				.find(|(i, _)| has_instructions[*i])
				.or_else(|| of_style().next())
				.map(|(i, _)| i)
		};
		let Some(regular) = index_of(Style::Regular) else {
			return Err(FontError::NoRegularFace {
				family: family.to_owned(),
				found,
			});
		};
		check_monospace(&faces[regular].bytes, family)?;
		// Цепочка подстановки (ADR-0003 п.3).
		let mut style_source = [Style::Regular; 4];
		let mut face_of_style = [regular; 4];
		for style in Style::ALL {
			let chain: &[Style] = match style {
				Style::Regular => &[Style::Regular],
				Style::Bold => &[Style::Bold, Style::Regular],
				Style::Italic => &[Style::Italic, Style::Regular],
				Style::BoldItalic => &[
					Style::BoldItalic,
					Style::Italic,
					Style::Bold,
					Style::Regular,
				],
			};
			let source = chain
				.iter()
				.copied()
				.find(|&s| index_of(s).is_some())
				.unwrap_or(Style::Regular);
			style_source[style.channel()] = source;
			face_of_style[style.channel()] = index_of(source).unwrap_or(regular);
		}
		let metrics = cell_metrics(&faces[regular].bytes, FONT_SIZE_PX)?;
		Ok(Self {
			faces,
			family: family.to_owned(),
			face_of_style,
			style_source,
			metrics,
			size_px: FONT_SIZE_PX,
		})
	}

	/// Имя семейства (`name` ID 1), как было запрошено при загрузке.
	pub fn family(&self) -> &str {
		&self.family
	}

	/// Файл, из которого берётся начертание (с учётом подстановки).
	pub fn face(&self, style: Style) -> &Face {
		&self.faces[self.face_of_style[style.channel()]]
	}

	/// Начертание-источник для запрошенного (само себя, если файл есть).
	pub fn style_source(&self, style: Style) -> Style {
		self.style_source[style.channel()]
	}

	/// Метрики ячейки при размере `FONT_SIZE_PX`.
	pub fn metrics(&self) -> CellMetrics {
		self.metrics
	}

	/// Размер, для которого собраны `metrics()`.
	pub fn size_px(&self) -> u16 {
		self.size_px
	}

	/// Метрики ячейки при произвольном размере растеризации (ADR-0005 п.2): тот же расчёт, что
	/// в [`cell_metrics`], но по файлу Regular. Ошибка — как при загрузке семейства.
	pub fn metrics_at(&self, size_px: u16) -> Result<CellMetrics, FontError> {
		cell_metrics(&self.face(Style::Regular).bytes, size_px)
	}

	/// Все загруженные файлы семейства.
	pub fn faces(&self) -> &[Face] {
		&self.faces
	}
}

/// Файл шрифта по расширению: `.ttf`/`.otf` в любом регистре.
fn is_font_file(path: &Path) -> bool {
	matches!(
		path.extension()
			.and_then(|e| e.to_str())
			.map(str::to_ascii_lowercase)
			.as_deref(),
		Some("ttf" | "otf")
	)
}

/// Несут ли глифы шрифта родные инструкции хинтинга: `maxp.maxSizeOfInstructions > 0`
/// (ADR-0025). У CFF-шрифтов и неинструктированных TTF (в том числе Nerd-сборок с `prep`, но
/// пустыми глифами) — `false`: их хинтует автохинтер.
pub fn has_glyph_instructions(font: &FontRef<'_>) -> bool {
	font.maxp()
		.ok()
		.and_then(|maxp| maxp.max_size_of_instructions())
		.is_some_and(|size| size > 0)
}

/// Начертание файла по таблицам шрифта, не по имени (FR-8.2): `OS/2.usWeightClass ≥ 600` —
/// Bold, `OS/2.fsSelection` ITALIC — Italic.
fn style_of(font: &FontRef<'_>) -> Style {
	let attributes = font.attributes();
	Style::from_traits(
		attributes.weight.value() >= BOLD_WEIGHT_MIN,
		!matches!(attributes.style, skrifa::attribute::Style::Normal),
	)
}

fn family_name(font: &FontRef<'_>) -> String {
	font.localized_strings(StringId::FAMILY_NAME)
		.english_or_first()
		.map(|s| s.to_string())
		.unwrap_or_default()
}

/// Проверка моноширинности (ADR-0017): advance каждого глифа [`MONOSPACE_PROBE`], который есть в
/// шрифте, равен advance `M` с точностью до пикселя при `FONT_SIZE_PX`. Пропорциональный шрифт в
/// сетке ячеек давал бы обрезанные глифы (FR-9, FR-13) — это не должно проходить молча.
fn check_monospace(ttf: &[u8], family: &str) -> Result<(), FontError> {
	let font =
		FontRef::new(ttf).map_err(|_| FontError::TtfParse("not a valid TrueType/OpenType font"))?;
	let size = Size::new(FONT_SIZE_PX as f32);
	let location = LocationRef::default();
	let charmap = font.charmap();
	let metrics = font.glyph_metrics(size, location);
	let advance_of = |ch: char| charmap.map(ch).and_then(|gid| metrics.advance_width(gid));
	let expected = advance_of('M').ok_or(FontError::TtfParse(
		"font has no 'M' glyph to derive cell width",
	))?;
	for glyph in MONOSPACE_PROBE {
		let Some(advance) = advance_of(glyph) else {
			continue;
		};
		if advance.round() != expected.round() {
			return Err(FontError::NotMonospace {
				family: family.to_owned(),
				glyph,
				advance,
				expected,
			});
		}
	}
	Ok(())
}

/// Метрики ячейки по начертанию: ширина — advance `M`, высота — ascent − descent (descent < 0).
pub fn cell_metrics(ttf: &[u8], size_px: u16) -> Result<CellMetrics, FontError> {
	let font =
		FontRef::new(ttf).map_err(|_| FontError::TtfParse("not a valid TrueType/OpenType font"))?;
	let size = Size::new(size_px as f32);
	let location = LocationRef::default();
	let line = font.metrics(size, location);
	let m_glyph = font.charmap().map('M').ok_or(FontError::TtfParse(
		"font has no 'M' glyph to derive cell width",
	))?;
	let glyph_w = font
		.glyph_metrics(size, location)
		.advance_width(m_glyph)
		.ok_or(FontError::TtfParse("no advance width for 'M'"))?
		.round() as usize;
	let glyph_h = (line.ascent - line.descent).round() as usize;
	if !(1..=255).contains(&glyph_w) || !(1..=255).contains(&glyph_h) {
		return Err(FontError::BadGlyphSize { glyph_w, glyph_h });
	}
	Ok(CellMetrics {
		glyph_w,
		glyph_h,
		baseline: line.ascent.round() as i32,
	})
}
