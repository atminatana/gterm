//! Загрузка темы формата helix (ADR-0037): TOML, где ключ — область, значение — цвет символа
//! строкой либо таблица `{ fg, bg, modifiers, underline }`; `[palette]` — имена цветов,
//! `inherits` — тема-родитель из тех же каталогов. Таблицы родителя и потомка объединяются до
//! разрешения цветов, поэтому области родителя берут палитру потомка — как в helix.
//!
//! Чего ячейка не умеет (ADR-0012), не ошибка: `reversed` заменяется обменом цветов, остальное
//! пропускается и перечисляется в [`Theme::warnings`].

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use gterm::cell::Color;
use toml::de::{DeTable, DeValue};

use super::{Theme, scope};
use crate::style::Style;

/// Сколько тем может стоять в цепочке `inherits`, считая саму тему.
const MAX_INHERITS_DEPTH: usize = 8;

/// Расширение файла темы.
const THEME_EXTENSION: &str = "toml";

const KEY_INHERITS: &str = "inherits";
const KEY_PALETTE: &str = "palette";

/// Имена цветов ANSI в темах helix; значения — палитра xterm.
const ANSI_COLORS: [(&str, Color); 16] = [
	("black", Color::from_rgb(0x00, 0x00, 0x00)),
	("red", Color::from_rgb(0xCD, 0x00, 0x00)),
	("green", Color::from_rgb(0x00, 0xCD, 0x00)),
	("yellow", Color::from_rgb(0xCD, 0xCD, 0x00)),
	("blue", Color::from_rgb(0x00, 0x00, 0xEE)),
	("magenta", Color::from_rgb(0xCD, 0x00, 0xCD)),
	("cyan", Color::from_rgb(0x00, 0xCD, 0xCD)),
	("gray", Color::from_rgb(0x7F, 0x7F, 0x7F)),
	("light-red", Color::from_rgb(0xFF, 0x00, 0x00)),
	("light-green", Color::from_rgb(0x00, 0xFF, 0x00)),
	("light-yellow", Color::from_rgb(0xFF, 0xFF, 0x00)),
	("light-blue", Color::from_rgb(0x5C, 0x5C, 0xFF)),
	("light-magenta", Color::from_rgb(0xFF, 0x00, 0xFF)),
	("light-cyan", Color::from_rgb(0x00, 0xFF, 0xFF)),
	("light-gray", Color::from_rgb(0xE5, 0xE5, 0xE5)),
	("white", Color::from_rgb(0xFF, 0xFF, 0xFF)),
];

/// Имя цвета helix «как у терминала»: цвет остаётся незаданным.
const COLOR_DEFAULT: &str = "default";

/// Значение TOML в объёме, нужном теме: владеющая копия разбора — таблицы разных файлов
/// объединяются, а разбор `toml` заимствует текст своего файла.
enum Value {
	String(String),
	Array(Vec<Value>),
	Table(Table),
	/// Число, дата, логическое: в темах значения не имеют.
	Other,
}

type Table = BTreeMap<String, Value>;

impl Value {
	fn from_parsed(value: &DeValue<'_>) -> Self {
		match value {
			DeValue::String(text) => Self::String(text.to_string()),
			DeValue::Array(items) => Self::Array(
				items
					.iter()
					.map(|item| Self::from_parsed(item.get_ref()))
					.collect(),
			),
			DeValue::Table(table) => Self::Table(table_from_parsed(table)),
			_ => Self::Other,
		}
	}

	fn as_str(&self) -> Option<&str> {
		match self {
			Self::String(text) => Some(text),
			_ => None,
		}
	}

	/// Значение ключа, если это таблица.
	fn get(&self, key: &str) -> Option<&Value> {
		match self {
			Self::Table(table) => table.get(key),
			_ => None,
		}
	}
}

fn table_from_parsed(table: &DeTable<'_>) -> Table {
	table
		.iter()
		.map(|(key, value)| {
			(
				key.get_ref().to_string(),
				Value::from_parsed(value.get_ref()),
			)
		})
		.collect()
}

/// Почему тему не удалось загрузить.
#[derive(Debug)]
pub enum ThemeError {
	/// Файла `<name>.toml` нет ни в одном из каталогов.
	NotFound { name: String },
	/// Файл темы или каталог тем есть в перечне, но не прочитан.
	Io {
		path: PathBuf,
		source: std::io::Error,
	},
	/// Текст — не TOML; строка и колонка считаются с единицы.
	Syntax {
		file: String,
		line: usize,
		column: usize,
		message: String,
	},
	/// `inherits` замкнулся: цепочка имён от темы до повторившегося родителя.
	InheritsCycle { chain: Vec<String> },
	/// Цепочка `inherits` длиннее допустимой.
	InheritsTooDeep { chain: Vec<String> },
}

impl fmt::Display for ThemeError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::NotFound { name } => write!(f, "theme `{name}` not found"),
			Self::Io { path, source } => write!(f, "theme path {}: {source}", path.display()),
			Self::Syntax {
				file,
				line,
				column,
				message,
			} => write!(f, "theme {file}:{line}:{column}: {message}"),
			Self::InheritsCycle { chain } => {
				write!(f, "theme inherits itself: {}", chain.join(" -> "))
			}
			Self::InheritsTooDeep { chain } => {
				write!(
					f,
					"theme inherits chain is too deep: {}",
					chain.join(" -> ")
				)
			}
		}
	}
}

impl std::error::Error for ThemeError {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match self {
			Self::Io { source, .. } => Some(source),
			_ => None,
		}
	}
}

impl Theme {
	/// Тема из текста TOML. Родителя взять неоткуда: `inherits` в тексте — ошибка `NotFound` с
	/// именем родителя.
	pub fn from_toml_str(text: &str) -> Result<Self, ThemeError> {
		let mut table = parse(text, "<text>")?;
		match take_inherits(&mut table) {
			Some(parent) => Err(ThemeError::NotFound { name: parent }),
			None => Ok(build("", table)),
		}
	}

	/// Тема `<name>.toml` из первого каталога перечня, где она есть; родители по `inherits`
	/// ищутся там же.
	pub fn load<P: AsRef<Path>>(dirs: &[P], name: &str) -> Result<Self, ThemeError> {
		let mut chain = vec![name.to_string()];
		let mut merged = read(dirs, name)?;
		while let Some(parent) = take_inherits(&mut merged) {
			let is_cycle = chain.contains(&parent);
			chain.push(parent);
			if is_cycle {
				return Err(ThemeError::InheritsCycle { chain });
			}
			if chain.len() > MAX_INHERITS_DEPTH {
				return Err(ThemeError::InheritsTooDeep { chain });
			}
			let parent_table = read(dirs, &chain[chain.len() - 1])?;
			merged = merge(parent_table, merged);
		}
		Ok(build(name, merged))
	}

	/// Имена тем в каталогах перечня: файлы `*.toml` без расширения, по алфавиту и без повторов —
	/// то, что принимает [`Theme::load`] с теми же каталогами. Файлы не читаются: годна ли тема,
	/// выясняет загрузка. Каталог, который не удалось прочитать, — ошибка `Io` с его путём.
	pub fn list<P: AsRef<Path>>(dirs: &[P]) -> Result<Vec<String>, ThemeError> {
		let mut names = Vec::new();
		for dir in dirs {
			let dir = dir.as_ref();
			let io_error = |source| ThemeError::Io {
				path: dir.to_path_buf(),
				source,
			};
			for entry in std::fs::read_dir(dir).map_err(io_error)? {
				let path = entry.map_err(io_error)?.path();
				if let Some(name) = theme_name(&path) {
					names.push(name);
				}
			}
		}
		names.sort();
		names.dedup();
		Ok(names)
	}
}

/// Имя темы по пути файла: `<имя>.toml`; каталог, другое расширение или имя не в UTF-8 — не тема.
fn theme_name(path: &Path) -> Option<String> {
	let is_theme_file =
		path.is_file() && path.extension().is_some_and(|ext| ext == THEME_EXTENSION);
	if !is_theme_file {
		return None;
	}
	path.file_stem()?.to_str().map(str::to_owned)
}

/// Имя родителя; ключ из таблицы убирается, чтобы не стать областью.
fn take_inherits(table: &mut Table) -> Option<String> {
	match table.remove(KEY_INHERITS) {
		Some(Value::String(parent)) => Some(parent),
		_ => None,
	}
}

fn read<P: AsRef<Path>>(dirs: &[P], name: &str) -> Result<Table, ThemeError> {
	let file = format!("{name}.{THEME_EXTENSION}");
	let path = dirs
		.iter()
		.map(|dir| dir.as_ref().join(&file))
		.find(|path| path.is_file())
		.ok_or_else(|| ThemeError::NotFound {
			name: name.to_string(),
		})?;
	let text = std::fs::read_to_string(&path).map_err(|source| ThemeError::Io {
		path: path.clone(),
		source,
	})?;
	parse(&text, &file)
}

fn parse(text: &str, file: &str) -> Result<Table, ThemeError> {
	let parsed = DeTable::parse(text).map_err(|error| {
		let offset = error.span().map_or(0, |span| span.start);
		let (line, column) = position(text, offset);
		ThemeError::Syntax {
			file: file.to_string(),
			line,
			column,
			message: error.message().to_string(),
		}
	})?;
	Ok(table_from_parsed(parsed.get_ref()))
}

/// Строка и колонка (в символах, с единицы) по смещению в байтах.
fn position(text: &str, offset: usize) -> (usize, usize) {
	let before = &text.as_bytes()[..offset.min(text.len())];
	let line_start = before
		.iter()
		.rposition(|&byte| byte == b'\n')
		.map_or(0, |index| index + 1);
	let line = before.iter().filter(|&&byte| byte == b'\n').count() + 1;
	let column = String::from_utf8_lossy(&before[line_start..])
		.chars()
		.count()
		+ 1;
	(line, column)
}

/// Потомок поверх родителя: области заменяются целиком, палитры объединяются по именам.
fn merge(mut parent: Table, child: Table) -> Table {
	for (key, value) in child {
		match (key.as_str(), value, parent.get_mut(&key)) {
			(KEY_PALETTE, Value::Table(colors), Some(Value::Table(parent_colors))) => {
				parent_colors.extend(colors);
			}
			(_, value, _) => {
				parent.insert(key, value);
			}
		}
	}
	parent
}

/// Область до обмена цветов.
struct Parsed {
	style: Style,
	is_reversed: bool,
}

fn build(name: &str, mut table: Table) -> Theme {
	let mut theme = Theme::new(name);
	if let Some(palette) = table.remove(KEY_PALETTE) {
		theme.palette = palette_colors(palette, &mut theme.warnings);
	}
	let mut parsed = BTreeMap::new();
	for (scope_name, value) in table {
		match parse_scope(&scope_name, &value, &theme.palette, &mut theme.warnings) {
			Ok(scope_style) => {
				parsed.insert(scope_name, scope_style);
			}
			Err(reason) => theme.warnings.push(format!("{scope_name}: {reason}")),
		}
	}
	let text = parsed.get(scope::TEXT).and_then(|p| p.style.fg);
	let background = parsed.get(scope::BACKGROUND).and_then(|p| p.style.bg);
	for (
		scope_name,
		Parsed {
			mut style,
			is_reversed,
		},
	) in parsed
	{
		if is_reversed {
			match (style.fg.or(text), style.bg.or(background)) {
				(Some(fg), Some(bg)) => {
					style.fg = Some(bg);
					style.bg = Some(fg);
				}
				_ => theme.warnings.push(format!(
					"{scope_name}: modifier `reversed` skipped, colors are unknown"
				)),
			}
		}
		theme.scopes.insert(scope_name, style);
	}
	theme.derive();
	theme
}

fn palette_colors(palette: Value, warnings: &mut Vec<String>) -> BTreeMap<String, Color> {
	let mut colors = BTreeMap::new();
	let Value::Table(palette) = palette else {
		warnings.push("palette: not a table".to_string());
		return colors;
	};
	for (name, value) in palette {
		match value.as_str().map(literal_color) {
			Some(Ok(Some(color))) => {
				colors.insert(name, color);
			}
			Some(Ok(None)) => {}
			_ => warnings.push(format!("palette.{name}: not a color")),
		}
	}
	colors
}

/// Цвет без палитры: `#RRGGBB`, `#RRGGBBAA` или имя ANSI. `Ok(None)` — `default`: цвет не задан.
fn literal_color(text: &str) -> Result<Option<Color>, String> {
	if text == COLOR_DEFAULT {
		return Ok(None);
	}
	if let Some(hex) = text.strip_prefix('#') {
		return hex_color(hex)
			.map(Some)
			.ok_or_else(|| format!("bad color `{text}`"));
	}
	ANSI_COLORS
		.iter()
		.find(|(name, _)| *name == text)
		.map(|&(_, color)| Some(color))
		.ok_or_else(|| format!("unknown color `{text}`"))
}

fn hex_color(hex: &str) -> Option<Color> {
	if !hex.is_ascii() || (hex.len() != 6 && hex.len() != 8) {
		return None;
	}
	let channel = |index: usize| u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).ok();
	let alpha = if hex.len() == 8 { channel(3)? } else { 0xFF };
	Some(Color::from_rgba(
		channel(0)?,
		channel(1)?,
		channel(2)?,
		alpha,
	))
}

/// Цвет области: сначала имя палитры темы, затем запись цвета.
fn scope_color(value: &Value, palette: &BTreeMap<String, Color>) -> Result<Option<Color>, String> {
	let text = value
		.as_str()
		.ok_or_else(|| "color is not a string".to_string())?;
	match palette.get(text) {
		Some(color) => Ok(Some(*color)),
		None => literal_color(text),
	}
}

fn parse_scope(
	scope_name: &str,
	value: &Value,
	palette: &BTreeMap<String, Color>,
	warnings: &mut Vec<String>,
) -> Result<Parsed, String> {
	let mut style = Style::new();
	let mut is_reversed = false;
	let table = match value {
		Value::String(_) => {
			style.fg = scope_color(value, palette)?;
			return Ok(Parsed { style, is_reversed });
		}
		Value::Table(table) => table,
		_ => return Err("neither a color nor a table".to_string()),
	};
	for (key, item) in table {
		match key.as_str() {
			"fg" => style.fg = scope_color(item, palette)?,
			"bg" => style.bg = scope_color(item, palette)?,
			"modifiers" => {
				let Value::Array(modifiers) = item else {
					return Err("modifiers is not an array".to_string());
				};
				for modifier in modifiers {
					match modifier.as_str() {
						Some("bold") => style = style.bold(),
						Some("italic") => style = style.italic(),
						Some("underlined") => style = style.underlined(),
						Some("reversed") => is_reversed = true,
						other => warnings.push(format!(
							"{scope_name}: modifier `{}` skipped",
							other.unwrap_or("<not a string>")
						)),
					}
				}
			}
			"underline" => {
				let kind = item.get("style").and_then(Value::as_str);
				if kind.is_some_and(|kind| kind != "none") {
					style = style.underlined();
				}
				if item.get("color").is_some() || kind.is_some_and(|kind| kind != "line") {
					warnings.push(format!("{scope_name}: underline color and shape skipped"));
				}
			}
			other => warnings.push(format!("{scope_name}: key `{other}` skipped")),
		}
	}
	Ok(Parsed { style, is_reversed })
}
