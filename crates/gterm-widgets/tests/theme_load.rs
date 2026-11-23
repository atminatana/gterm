//! Загрузка тем формата helix (ADR-0037, ТЗ К1, К2, К4, К7): формы значений, палитра, имена
//! ANSI, модификаторы, `reversed`, `inherits`, ошибки с позицией и предупреждения. Тестовые
//! темы — `tests/data/themes/`.

#![cfg(feature = "theme-toml")]

use std::fs;
use std::path::PathBuf;

use gterm::cell::{CellFlags, Color};
use gterm_widgets::{Style, Theme, ThemeError};

const INK: Color = Color::from_rgb(0xC0, 0xC0, 0xC0);
const GROUND: Color = Color::from_rgb(0x10, 0x10, 0x10);
const ACCENT: Color = Color::from_rgb(0xFF, 0x80, 0x00);
const CHILD_ACCENT: Color = Color::from_rgb(0x00, 0xFF, 0x00);

/// Темы с таким именем нет ни в одном каталоге.
const MISSING_THEME: &str = "gterm_missing_theme";
/// Родитель темы `orphan.toml`, которого нет.
const MISSING_PARENT: &str = "gterm_missing_parent";
/// Каталога с таким именем нет.
const MISSING_DIR: &str = "gterm_missing_dir";
/// Темы каталога `tests/data/themes/` по алфавиту.
const THEMES_DIR_NAMES: [&str; 6] = ["base", "broken", "child", "cycle_a", "cycle_b", "orphan"];
/// Переменная окружения с каталогом тем helix для сквозной проверки (К7).
const HELIX_THEMES_VAR: &str = "GTERM_HELIX_THEMES";

fn themes_dir() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/themes")
}

fn extra_dir() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/themes_extra")
}

fn load(name: &str) -> Result<Theme, ThemeError> {
	Theme::load(&[themes_dir()], name)
}

fn base() -> Theme {
	load("base").expect("тестовая тема base")
}

fn has_warning(theme: &Theme, needle: &str) -> bool {
	theme.warnings().iter().any(|w| w.contains(needle))
}

#[test]
fn string_is_foreground_and_table_carries_colors_and_modifiers() {
	let theme = base();
	assert_eq!(theme.name(), "base");
	assert_eq!(theme.get("ui.text"), Some(Style::new().fg(INK)));
	assert_eq!(theme.get("ui.background"), Some(Style::new().bg(GROUND)));
	let focus = theme.get("ui.text.focus").expect("ui.text.focus");
	assert_eq!(focus.fg, Some(ACCENT));
	assert_eq!(focus.add, CellFlags::BOLD | CellFlags::ITALIC);
	assert_eq!(theme.base(), Style::new().fg(INK).bg(GROUND));
}

#[test]
fn colors_come_from_palette_hex_with_alpha_and_ansi_names() {
	let theme = base();
	assert_eq!(theme.color("accent"), Some(ACCENT));
	let menu = theme.get("ui.menu").expect("ui.menu");
	assert_eq!(menu.fg, Some(INK));
	assert_eq!(menu.bg, Some(Color::from_rgba(0x20, 0x28, 0x38, 0x80)));
	let toolbar = theme.get("myapp.toolbar").expect("myapp.toolbar");
	assert_eq!(toolbar.fg, Some(Color::from_rgb(0, 0, 0)));
	assert_eq!(toolbar.bg, Some(ACCENT));
	// Своя область приложения с наследованием по точке.
	assert_eq!(
		theme.style("myapp.toolbar.button.pressed").fg,
		Some(Color::WHITE)
	);
	// `default` — цвет не задан, остальное в области действует.
	let plain = theme.get("ui.plain").expect("ui.plain");
	assert_eq!(plain.fg, None);
	assert_eq!(plain.add, CellFlags::UNDERLINE);
}

#[test]
fn reversed_swaps_colors_taking_missing_ones_from_text_and_background() {
	let theme = base();
	// Своих цветов у области нет: берутся `ui.text` и `ui.background`.
	assert_eq!(
		theme.get("ui.menu.selected"),
		Some(Style::new().fg(GROUND).bg(INK))
	);
	assert_eq!(
		theme.get("ui.cursor"),
		Some(Style::new().fg(GROUND).bg(ACCENT))
	);

	let theme = Theme::from_toml_str(r#""a" = { modifiers = ["reversed"] }"#).expect("тема");
	assert_eq!(theme.get("a"), Some(Style::new()));
	assert!(has_warning(&theme, "a: modifier `reversed` skipped"));
}

#[test]
fn what_a_cell_cannot_show_is_skipped_with_a_warning() {
	let theme = base();
	assert!(has_warning(&theme, "ui.cursor: modifier `dim` skipped"));
	// Подчёркивание остаётся, его цвет и вид — нет.
	let link = theme.get("ui.link").expect("ui.link");
	assert_eq!(link.add, CellFlags::UNDERLINE);
	assert!(has_warning(
		&theme,
		"ui.link: underline color and shape skipped"
	));
}

#[test]
fn bad_scope_is_dropped_and_the_theme_still_loads() {
	let theme = base();
	assert_eq!(theme.get("ui.broken.color"), None);
	assert_eq!(theme.get("ui.broken.type"), None);
	assert!(has_warning(
		&theme,
		"ui.broken.color: unknown color `no-such-color`"
	));
	assert!(has_warning(&theme, "ui.broken.type"));
	assert_eq!(theme.warnings().len(), 4, "{:?}", theme.warnings());
}

#[test]
fn child_overrides_scopes_and_parent_scopes_use_the_merged_palette() {
	let theme = load("child").expect("тестовая тема child");
	assert_eq!(theme.name(), "child");
	assert_eq!(theme.color("accent"), Some(CHILD_ACCENT));
	assert_eq!(theme.color("ink"), Some(INK));
	// Область заменена потомком.
	assert_eq!(theme.get("ui.text"), Some(Style::new().fg(CHILD_ACCENT)));
	// Область родителя разрешена по палитре потомка.
	assert_eq!(theme.style("ui.text.focus").fg, Some(CHILD_ACCENT));
	// Своя область потомка и нетронутая область родителя.
	assert_eq!(theme.get("myapp.status"), Some(Style::new().fg(INK)));
	assert_eq!(theme.get("ui.background"), Some(Style::new().bg(GROUND)));
	// `inherits` областью не становится.
	assert_eq!(theme.get("inherits"), None);
}

#[test]
fn inherits_cycle_is_an_error_with_the_chain() {
	match load("cycle_a") {
		Err(ThemeError::InheritsCycle { chain }) => {
			assert_eq!(chain, ["cycle_a", "cycle_b", "cycle_a"]);
		}
		other => panic!("ожидался цикл, получено {other:?}"),
	}
}

#[test]
fn inherits_chain_longer_than_the_limit_is_an_error() {
	let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("theme_deep_chain");
	fs::create_dir_all(&dir).expect("каталог цепочки");
	const LENGTH: usize = 12;
	for index in 0..LENGTH {
		let text = if index + 1 < LENGTH {
			format!("inherits = \"deep{}\"\n", index + 1)
		} else {
			"\"ui.text\" = \"white\"\n".to_string()
		};
		fs::write(dir.join(format!("deep{index}.toml")), text).expect("файл цепочки");
	}
	match Theme::load(&[&dir], "deep0") {
		Err(ThemeError::InheritsTooDeep { chain }) => assert_eq!(chain[0], "deep0"),
		other => panic!("ожидалась слишком длинная цепочка, получено {other:?}"),
	}
	// Короткая цепочка из того же каталога загружается.
	let theme = Theme::load(&[&dir], "deep8").expect("цепочка из четырёх тем");
	assert_eq!(theme.get("ui.text"), Some(Style::new().fg(Color::WHITE)));
}

#[test]
fn missing_theme_and_missing_parent_are_not_found() {
	match load(MISSING_THEME) {
		Err(ThemeError::NotFound { name }) => assert_eq!(name, MISSING_THEME),
		other => panic!("ожидалось NotFound, получено {other:?}"),
	}
	match load("orphan") {
		Err(ThemeError::NotFound { name }) => assert_eq!(name, MISSING_PARENT),
		other => panic!("ожидалось NotFound, получено {other:?}"),
	}
	// Тексту родителя взять неоткуда.
	match Theme::from_toml_str("inherits = \"base\"\n") {
		Err(ThemeError::NotFound { name }) => assert_eq!(name, "base"),
		other => panic!("ожидалось NotFound, получено {other:?}"),
	}
	let no_dirs: [PathBuf; 0] = [];
	assert!(matches!(
		Theme::load(&no_dirs, "base"),
		Err(ThemeError::NotFound { .. })
	));
}

#[test]
fn syntax_error_carries_file_line_and_column() {
	match load("broken") {
		Err(ThemeError::Syntax {
			file,
			line,
			column,
			message,
		}) => {
			assert_eq!(file, "broken.toml");
			assert_eq!((line, column), (2, 13));
			assert!(!message.is_empty());
		}
		other => panic!("ожидалась ошибка синтаксиса, получено {other:?}"),
	}
	let error = load("broken").expect_err("ошибка синтаксиса");
	assert!(error.to_string().starts_with("theme broken.toml:2:13: "));
}

#[test]
fn theme_is_taken_from_the_first_directory_that_has_it() {
	let dirs = [themes_dir(), extra_dir()];
	let theme = Theme::load(&dirs, "extra").expect("тема из второго каталога");
	assert_eq!(
		theme.get("ui.text"),
		Some(Style::new().fg(Color::from_rgb(1, 2, 3)))
	);
	assert!(Theme::load(&dirs, "base").is_ok());
}

#[test]
fn list_gives_file_stems_in_alphabetical_order() {
	let names = Theme::list(&[themes_dir()]).expect("перечень тестовых тем");
	assert_eq!(names, THEMES_DIR_NAMES);
}

/// В `themes_extra/` рядом с темой лежат `readme.txt` и каталог `nested.toml` — не темы.
#[test]
fn list_merges_directories_without_repeats_and_skips_what_is_not_a_theme_file() {
	let names =
		Theme::list(&[extra_dir(), themes_dir(), extra_dir()]).expect("перечень двух каталогов");
	let mut expected: Vec<&str> = THEMES_DIR_NAMES.to_vec();
	expected.push("extra");
	expected.sort_unstable();
	assert_eq!(names, expected);
	assert_eq!(Theme::list(&[extra_dir()]).expect("перечень"), ["extra"]);
}

#[test]
fn list_of_no_directories_is_empty_and_missing_directory_is_an_io_error() {
	let no_dirs: [PathBuf; 0] = [];
	assert!(Theme::list(&no_dirs).expect("пустой перечень").is_empty());
	let missing = themes_dir().join(MISSING_DIR);
	match Theme::list(&[themes_dir(), missing.clone()]) {
		Err(ThemeError::Io { path, .. }) => assert_eq!(path, missing),
		other => panic!("ожидалась ошибка чтения каталога, получено {other:?}"),
	}
}

/// Каждое имя перечня — то, что находит `load`: ошибка загрузки, если она есть, — не «тема не
/// найдена» с этим именем (у `orphan` не найден родитель, у `broken` — синтаксис).
#[test]
fn every_listed_name_is_found_by_load() {
	for name in Theme::list(&[themes_dir()]).expect("перечень") {
		if let Err(ThemeError::NotFound { name: missing }) = load(&name) {
			assert_ne!(missing, name);
		}
	}
}

#[test]
fn empty_text_is_an_empty_theme() {
	let theme = Theme::from_toml_str("").expect("пустая тема");
	assert_eq!(theme.scopes().count(), 0);
	assert!(theme.warnings().is_empty());
}

/// Темы-образцы примера (`examples/themes/`) загружаются без единого предупреждения и несут
/// области виджетов и область приложения.
#[test]
fn example_themes_load_clean() {
	let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/themes");
	for name in ["gterm_dark", "gterm_light"] {
		let theme = Theme::load(&[&dir], name).unwrap_or_else(|e| panic!("{name}: {e}"));
		assert!(
			theme.warnings().is_empty(),
			"{name}: {:?}",
			theme.warnings()
		);
		let base = theme.base();
		assert!(
			base.fg.is_some() && base.bg.is_some(),
			"{name}: базовый стиль"
		);
		for scope in ["ui.button.focus", "ui.input", "widget_edit.badge"] {
			assert!(theme.get(scope).is_some(), "{name}: нет области {scope}");
		}
	}
}

/// Все темы каталога helix загружаются без ошибки (К7). Каталог задаёт переменная окружения;
/// без неё проверка печатает причину и проходит.
#[test]
fn every_helix_theme_loads() {
	let Some(dir) = std::env::var_os(HELIX_THEMES_VAR).map(PathBuf::from) else {
		eprintln!("every_helix_theme_loads: {HELIX_THEMES_VAR} не задана — каталога тем helix нет");
		return;
	};
	let entries = fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
	let mut names: Vec<String> = entries
		.filter_map(|entry| entry.ok().map(|entry| entry.path()))
		.filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
		.filter_map(|path| path.file_stem()?.to_str().map(str::to_string))
		.collect();
	names.sort();
	assert!(!names.is_empty(), "в {} нет тем", dir.display());
	let mut warnings = 0;
	for name in &names {
		let theme = Theme::load(&[&dir], name).unwrap_or_else(|e| panic!("{name}: {e}"));
		assert!(theme.scopes().count() > 0, "{name}: ни одной области");
		warnings += theme.warnings().len();
	}
	eprintln!(
		"every_helix_theme_loads: тем {}, предупреждений {warnings}",
		names.len()
	);
}
