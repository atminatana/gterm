//! Полная запись темы (`Theme::to_toml_string`, поправка ADR-0038): формат строк и разделов,
//! пометка выведенных областей, круговая проверка «записать → загрузить» на собранной теме,
//! встроенной, всех темах примеров и — если задан каталог — темах helix; правка записи.

#![cfg(feature = "theme-toml")]

use std::path::PathBuf;

use gterm::cell::Color;
use gterm_widgets::{Style, Theme};

const INK: Color = Color::from_rgb(0xD0, 0xD0, 0xD0);
const GROUND: Color = Color::from_rgb(0x10, 0x10, 0x10);
const ACCENT: Color = Color::from_rgb(0xF0, 0xA0, 0x00);
const ON_ACCENT: Color = Color::from_rgb(0, 0, 0);
/// Цвет с неполной альфой: записывается восемью цифрами.
const GLASS: Color = Color::from_rgba(1, 2, 3, 0x80);

/// Переменная окружения с каталогом тем helix (как в `theme_load.rs`).
const HELIX_THEMES_VAR: &str = "GTERM_HELIX_THEMES";

const SECTION_TITLES: [&str; 4] = [
	"# Основа",
	"# Тона",
	"# Области виджетов",
	"# Остальные области",
];

/// Тема из основы с палитрой и своими областями приложения, выведенная.
fn sample() -> Theme {
	let mut theme = Theme::new("sample");
	theme.set_color("ink", INK).set_color("ground", GROUND);
	theme
		.set("ui.background", Style::new().bg(GROUND))
		.set("ui.text", Style::new().fg(INK))
		.set("ui.accent", Style::new().fg(ON_ACCENT).bg(ACCENT))
		.set("my.app", Style::new().fg(INK).bold().underlined())
		.set("my.glass", Style::new().bg(GLASS))
		.set("my.empty", Style::new())
		.set("we\"ird\\name", Style::new().fg(INK));
	theme.derive();
	theme
}

fn scopes(theme: &Theme) -> Vec<(String, Style)> {
	theme
		.scopes()
		.map(|(name, style)| (name.to_owned(), style))
		.collect()
}

fn reload(theme: &Theme) -> Theme {
	let text = theme.to_toml_string();
	Theme::from_toml_str(&text).unwrap_or_else(|e| panic!("запись не загрузилась: {e}\n{text}"))
}

/// Запись и загрузка возвращают те же области; все они теперь заданы; предупреждений нет.
fn assert_round_trip(theme: &Theme, label: &str) {
	let loaded = reload(theme);
	assert_eq!(scopes(&loaded), scopes(theme), "{label}: области");
	assert!(
		loaded.warnings().is_empty(),
		"{label}: {:?}",
		loaded.warnings()
	);
	for (name, _) in theme.scopes() {
		assert!(!loaded.is_derived(name), "{label}: {name} выведена");
	}
	// Палитра: повторная запись загруженной темы — тот же текст, кроме пометок «выведено».
	let without_marks = theme.to_toml_string().replace("  # выведено", "");
	let header_end = without_marks.find("\n\n").unwrap_or(0);
	let resaved = loaded.to_toml_string();
	let resaved_header_end = resaved.find("\n\n").unwrap_or(0);
	assert_eq!(
		&resaved[resaved_header_end..],
		&without_marks[header_end..],
		"{label}: повторная запись"
	);
}

#[test]
fn record_writes_every_scope_once_by_levels_and_marks_the_derived_ones() {
	let theme = sample();
	let text = theme.to_toml_string();
	for line in [
		// Основа: цвет палитры — именем, цвет вне палитры — записью.
		"\"ui.background\" = { bg = \"ground\" }",
		"\"ui.text\" = \"ink\"",
		"\"ui.accent\" = { fg = \"#000000\", bg = \"#F0A000\" }",
		"\"error\" = \"#FF6666\"  # выведено",
		// Тон и область виджета, выведенные из основы.
		"\"ui.control\" = { fg = \"ink\", bg = \"#363636\" }  # выведено",
		"\"ui.button\" = { fg = \"ink\", bg = \"#363636\" }  # выведено",
		"\"ui.table.header\" = \"#F0A000\"  # выведено",
		// Области приложения: начертание, альфа, пустая область, имя с кавычкой и чертой.
		"\"my.app\" = { fg = \"ink\", modifiers = [\"bold\", \"underlined\"] }",
		"\"my.glass\" = { bg = \"#01020380\" }",
		"\"my.empty\" = {}",
		"\"we\\\"ird\\\\name\" = \"ink\"",
	] {
		assert_eq!(
			text.lines().filter(|found| *found == line).count(),
			1,
			"строка {line}\n{text}"
		);
	}
	assert!(text.ends_with("\n[palette]\nground = \"#101010\"\nink = \"#D0D0D0\"\n"));
	// Разделы идут по уровням темы, палитра — последней.
	let positions: Vec<usize> = SECTION_TITLES
		.iter()
		.map(|title| {
			text.find(title)
				.unwrap_or_else(|| panic!("нет раздела {title}"))
		})
		.collect();
	assert!(positions.is_sorted(), "{positions:?}");
	assert!(positions[3] < text.find("[palette]").expect("палитра"));
	// Каждая область записана ровно одной строкой.
	let scope_lines = text.lines().filter(|line| line.starts_with('"')).count();
	assert_eq!(scope_lines, theme.scopes().count());
	let marked = text
		.lines()
		.filter(|line| line.ends_with("# выведено"))
		.count();
	let derived = theme
		.scopes()
		.filter(|(name, _)| theme.is_derived(name))
		.count();
	assert_eq!(marked, derived);
	assert!(text.starts_with("# Тема «sample» — полная запись"));
}

#[test]
fn loading_the_record_gives_the_same_theme_with_every_scope_given() {
	let theme = sample();
	assert_round_trip(&theme, "sample");
	let loaded = reload(&theme);
	assert_eq!(loaded.color("ink"), Some(INK));
	assert_eq!(loaded.color("ground"), Some(GROUND));
	assert_round_trip(&Theme::default(), "встроенная");
}

#[test]
fn theme_without_base_and_empty_theme_are_recorded_too() {
	let empty = Theme::new("empty");
	let text = empty.to_toml_string();
	assert!(text.lines().all(|line| line.starts_with('#')), "{text}");
	assert_eq!(reload(&empty).scopes().count(), 0);

	// Основы нет — выводить не из чего; раздел один, палитры нет.
	let mut plain = Theme::new("plain");
	plain.set("ui.menu", Style::new().fg(INK).bg(GROUND).italic());
	let text = plain.to_toml_string();
	assert!(text.contains("\n# Остальные области\n\"ui.menu\" = { fg = \"#D0D0D0\", bg = \"#101010\", modifiers = [\"italic\"] }\n"));
	assert!(!text.contains("[palette]"));
	assert!(!text.contains("# Основа"));
	assert_round_trip(&plain, "plain");
}

/// Запись — основа для доработки: строка, удалённая из тонов, при загрузке выводится заново, а
/// исправленная — берётся как задано.
#[test]
fn a_line_removed_from_the_record_is_derived_again_and_an_edited_one_is_kept() {
	let theme = sample();
	let text = theme.to_toml_string();
	let without_control: String = text
		.lines()
		.filter(|line| !line.starts_with("\"ui.control\" ="))
		.map(|line| format!("{line}\n"))
		.collect();
	let loaded = Theme::from_toml_str(&without_control).expect("запись без строки тона");
	assert!(loaded.is_derived("ui.control"));
	assert_eq!(loaded.get("ui.control"), theme.get("ui.control"));
	assert!(!loaded.is_derived("ui.button"));

	let edited = text.replace(
		"\"ui.button\" = { fg = \"ink\", bg = \"#363636\" }  # выведено",
		"\"ui.button\" = { fg = \"ground\", bg = \"ink\" }",
	);
	let loaded = Theme::from_toml_str(&edited).expect("исправленная запись");
	assert_eq!(
		loaded.get("ui.button"),
		Some(Style::new().fg(GROUND).bg(INK))
	);
}

#[test]
fn every_example_theme_survives_the_round_trip() {
	let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/themes");
	let names = Theme::list(&[&dir]).expect("перечень тем примеров");
	assert!(!names.is_empty());
	for name in names {
		let theme = Theme::load(&[&dir], &name).unwrap_or_else(|e| panic!("{name}: {e}"));
		assert_round_trip(&theme, &name);
	}
}

/// Сквозная проверка на темах helix: каталог берётся из переменной окружения; без неё проверка
/// печатает причину и проходит (как `every_helix_theme_loads`).
#[test]
fn every_helix_theme_survives_the_round_trip() {
	let Ok(dir) = std::env::var(HELIX_THEMES_VAR) else {
		eprintln!(
			"every_helix_theme_survives_the_round_trip: {HELIX_THEMES_VAR} не задана — пропуск"
		);
		return;
	};
	let dir = PathBuf::from(dir);
	let names = Theme::list(&[&dir]).unwrap_or_else(|e| panic!("каталог {}: {e}", dir.display()));
	assert!(!names.is_empty(), "в каталоге {} нет тем", dir.display());
	for name in &names {
		let theme = Theme::load(&[&dir], name).unwrap_or_else(|e| panic!("{name}: {e}"));
		assert_round_trip(&theme, name);
	}
	eprintln!(
		"every_helix_theme_survives_the_round_trip: тем {}",
		names.len()
	);
}
