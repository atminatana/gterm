//! Тема без файлов (ADR-0037, ТЗ К3, К6): доступ по имени, наследование по точке, запасные
//! цепочки, свои области приложения, сборка в коде и встроенная тема.

use gterm::cell::{CellFlags, Color};
use gterm_widgets::{Style, Theme};

const INK: Color = Color::from_rgb(0xC0, 0xC0, 0xC0);
const GROUND: Color = Color::from_rgb(0x10, 0x10, 0x10);
const ACCENT: Color = Color::from_rgb(0xFF, 0x80, 0x00);

/// Области, которых нет ни в одной теме.
const MISSING_SCOPE: &str = "gterm.test.missing";
const MISSING_COLOR: &str = "gterm-test-missing";

fn theme() -> Theme {
	let mut theme = Theme::new("test");
	theme
		.set("ui.text", Style::new().fg(INK))
		.set("ui.background", Style::new().bg(GROUND))
		.set("ui.menu", Style::new().fg(INK).bg(GROUND))
		.set("myapp.toolbar", Style::new().fg(ACCENT).bold())
		.set_color("accent", ACCENT);
	theme
}

#[test]
fn empty_theme_answers_every_name_with_an_empty_style() {
	let theme = Theme::new("empty");
	assert_eq!(theme.name(), "empty");
	assert_eq!(theme.get(MISSING_SCOPE), None);
	assert_eq!(theme.style(MISSING_SCOPE), Style::new());
	assert_eq!(theme.style(""), Style::new());
	assert_eq!(theme.first(&[MISSING_SCOPE, "ui.text"]), Style::new());
	assert_eq!(theme.first(&[]), Style::new());
	assert_eq!(theme.color(MISSING_COLOR), None);
	assert_eq!(theme.base(), Style::new());
	assert_eq!(theme.scopes().count(), 0);
	assert!(theme.warnings().is_empty());
}

#[test]
fn style_inherits_by_dot_and_get_is_exact() {
	let theme = theme();
	let menu = Style::new().fg(INK).bg(GROUND);
	assert_eq!(theme.get("ui.menu"), Some(menu));
	assert_eq!(theme.get("ui.menu.selected"), None);
	assert_eq!(theme.style("ui.menu.selected"), menu);
	assert_eq!(theme.style("ui.menu.selected.deep"), menu);
	// Выше по точке ничего нет: `ui` областью не объявлена.
	assert_eq!(theme.style("ui.other"), Style::new());
	// Имя, продолжающее чужое без точки, — другая область.
	assert_eq!(theme.style("ui.menubar"), Style::new());
}

#[test]
fn first_takes_the_first_existing_scope_without_dot_inheritance() {
	let theme = theme();
	assert_eq!(
		theme.first(&["ui.button", "ui.menu", "ui.text"]),
		Style::new().fg(INK).bg(GROUND)
	);
	// `ui.menu.selected` нет: цепочка не поднимается к `ui.menu`, а идёт дальше по перечню.
	assert_eq!(
		theme.first(&["ui.menu.selected", "ui.text"]),
		Style::new().fg(INK)
	);
}

#[test]
fn application_scopes_and_palette_are_read_like_the_built_in_ones() {
	let theme = theme();
	let toolbar = theme.style("myapp.toolbar.button");
	assert_eq!(toolbar.fg, Some(ACCENT));
	assert_eq!(toolbar.add, CellFlags::BOLD);
	assert_eq!(theme.color("accent"), Some(ACCENT));
	let names: Vec<&str> = theme.scopes().map(|(name, _)| name).collect();
	assert_eq!(
		names,
		["myapp.toolbar", "ui.background", "ui.menu", "ui.text"]
	);
}

#[test]
fn set_replaces_a_scope_and_a_color() {
	let mut theme = theme();
	theme
		.set("ui.text", Style::new().fg(ACCENT))
		.set_color("accent", INK);
	assert_eq!(theme.style("ui.text"), Style::new().fg(ACCENT));
	assert_eq!(theme.color("accent"), Some(INK));
	assert_eq!(theme.scopes().count(), 4);
}

#[test]
fn base_style_is_text_foreground_on_background() {
	assert_eq!(theme().base(), Style::new().fg(INK).bg(GROUND));
	// Без `ui.text` цвет символа берётся из `ui.background`, если он там задан.
	let mut theme = Theme::new("background-only");
	theme.set("ui.background", Style::new().fg(ACCENT).bg(GROUND));
	assert_eq!(theme.base(), Style::new().fg(ACCENT).bg(GROUND));
}

#[test]
fn built_in_theme_needs_no_files() {
	let theme = Theme::default();
	assert_eq!(theme.name(), "default");
	assert!(theme.warnings().is_empty());
	let base = theme.base();
	assert!(base.fg.is_some() && base.bg.is_some());
	assert_ne!(base.fg, base.bg);
	for scope in [
		"ui.window",
		"ui.window.focus",
		"ui.popup",
		"ui.menu",
		"ui.menu.selected",
		"ui.selection",
		"ui.button",
		"ui.button.focus",
		"ui.button.pressed",
		"ui.button.disabled",
		"ui.input",
	] {
		assert!(theme.get(scope).is_some(), "нет области {scope}");
	}
	assert!(theme.color("accent").is_some());
}
