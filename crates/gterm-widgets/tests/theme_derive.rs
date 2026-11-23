//! Вывод недостающих областей темы (ADR-0038, ТЗ К1–К4): основа, тона по формулам со
//! значениями, посчитанными вручную, ссылки областей виджетов, приоритет заданного над
//! выведенным, ручное описание темы целиком, повторный вывод, кадр дерева на теме из одной
//! основы. Темы каталога `examples/themes/` — в конце.

mod common;

use common::{Msg, screen};
use gterm::buffer::ScreenBuffer;
use gterm::cell::{CellFlags, Color};
use gterm_widgets::widgets::{Button, ButtonState};
use gterm_widgets::{Constraint, Layout, Rect, Style, Theme, Ui};

const fn gray(value: u8) -> Color {
	Color::from_rgb(value, value, value)
}

// Тёмная основа: фон 16 (сумма каналов 48 — тёмная), текст 208, акцент (240, 160, 0).
const DARK_BG: Color = gray(16);
const DARK_TEXT: Color = gray(208);
const ACCENT: Color = Color::from_rgb(240, 160, 0);
const ON_ACCENT: Color = gray(0);
/// Фон элемента управления: 16 + 38.
const DARK_CONTROL: Color = gray(54);
/// Приглушённый текст: (208 + 16 + 1) / 2.
const DARK_DIM: Color = gray(112);
/// Фон слоя: (16 + 54 + 1) / 2.
const DARK_POPUP: Color = gray(35);
/// Выделение — фон с третью акцента: ((16·2 + 240 + 1) / 3, (16·2 + 160 + 1) / 3, (16·2 + 0 + 1) / 3).
const DARK_SELECTION: Color = Color::from_rgb(91, 64, 11);
/// Поле под фокусом — фон элемента с четвертью акцента: ((54·3 + 240 + 2) / 4, (54·3 + 160 + 2) / 4, (54·3 + 2) / 4).
const DARK_CONTROL_FOCUS: Color = Color::from_rgb(101, 81, 41);
/// Нажатый элемент — пополам с акцентом: ((54 + 240 + 1) / 2, (54 + 160 + 1) / 2, (54 + 0 + 1) / 2).
const DARK_CONTROL_PRESSED: Color = Color::from_rgb(147, 107, 27);
const ERROR_ON_DARK: Color = Color::from_rgb(0xFF, 0x66, 0x66);

// Светлая основа: фон 240 (сумма 720 — светлая), текст 32, акцента нет.
const LIGHT_BG: Color = gray(240);
const LIGHT_TEXT: Color = gray(32);
const LIGHT_FOCUS: Color = Color::from_rgb(0, 80, 192);
/// Фон элемента управления: 240 − 38.
const LIGHT_CONTROL: Color = gray(202);
/// Приглушённый текст: (32 + 240 + 1) / 2.
const LIGHT_DIM: Color = gray(136);
/// Выделение: ((240·2 + 0 + 1) / 3, (240·2 + 80 + 1) / 3, (240·2 + 192 + 1) / 3).
const LIGHT_SELECTION: Color = Color::from_rgb(160, 187, 224);
const ERROR_ON_LIGHT: Color = Color::from_rgb(0xB3, 0x00, 0x00);

/// Цвет, которого нет ни в одной основе: заданное вручную.
const CUSTOM: Color = Color::from_rgb(1, 2, 3);
const CUSTOM_OTHER: Color = Color::from_rgb(4, 5, 6);

const TONES: [&str; 10] = [
	"ui.text.inactive",
	"ui.text.focus",
	"ui.window",
	"ui.window.focus",
	"ui.popup",
	"ui.selection",
	"ui.control",
	"ui.control.focus",
	"ui.control.pressed",
	"ui.control.disabled",
];

const WIDGET_SCOPES: [&str; 23] = [
	"ui.button",
	"ui.button.focus",
	"ui.button.pressed",
	"ui.button.disabled",
	"ui.checkbox.focus",
	"ui.checkbox.pressed",
	"ui.checkbox.disabled",
	"ui.input",
	"ui.input.focus",
	"ui.input.selection",
	"ui.input.placeholder",
	"ui.combo",
	"ui.combo.focus",
	"ui.combo.selected",
	"ui.list.selected",
	"ui.table.selected",
	"ui.table.header",
	"ui.tabs.selected",
	"ui.tabs.focus",
	"ui.scrollbar",
	"ui.scrollbar.thumb",
	"ui.gauge.filled",
	"ui.gauge.unfilled",
];

/// Тёмная тема из одной основы, вывод ещё не выполнен.
fn dark_base() -> Theme {
	let mut theme = Theme::new("dark");
	theme
		.set("ui.background", Style::new().bg(DARK_BG))
		.set("ui.text", Style::new().fg(DARK_TEXT))
		.set("ui.accent", Style::new().fg(ON_ACCENT).bg(ACCENT));
	theme
}

fn dark() -> Theme {
	let mut theme = dark_base();
	theme.derive();
	theme
}

fn light_base() -> Theme {
	let mut theme = Theme::new("light");
	theme
		.set("ui.background", Style::new().bg(LIGHT_BG))
		.set("ui.text", Style::new().fg(LIGHT_TEXT));
	theme
}

fn scope(theme: &Theme, name: &str) -> Style {
	theme
		.get(name)
		.unwrap_or_else(|| panic!("области {name} нет"))
}

fn fg(color: Color) -> Style {
	Style::new().fg(color)
}

#[test]
fn nothing_is_derived_without_background_or_text() {
	let mut no_text = Theme::new("no-text");
	no_text.set("ui.background", Style::new().bg(DARK_BG));
	let mut no_background = Theme::new("no-background");
	no_background.set("ui.text", Style::new().fg(DARK_TEXT));
	// Область есть, но нужного цвета в ней нет: фон без `bg`.
	let mut wrong_half = Theme::new("wrong-half");
	wrong_half
		.set("ui.background", Style::new().fg(DARK_BG))
		.set("ui.text", Style::new().fg(DARK_TEXT));
	for theme in [&mut no_text, &mut no_background, &mut wrong_half] {
		let before = theme.clone();
		theme.derive();
		assert_eq!(*theme, before, "тема {}", theme.name());
		assert!(!theme.is_derived("ui.control"));
	}
}

#[test]
fn dark_tones_follow_the_formulas() {
	let theme = dark();
	assert_eq!(scope(&theme, "ui.text.inactive"), fg(DARK_DIM));
	assert_eq!(scope(&theme, "ui.text.focus"), fg(ACCENT));
	assert_eq!(scope(&theme, "ui.window"), fg(DARK_DIM));
	assert_eq!(scope(&theme, "ui.window.focus"), fg(ACCENT));
	assert_eq!(scope(&theme, "ui.popup"), Style::new().bg(DARK_POPUP));
	assert_eq!(
		scope(&theme, "ui.selection"),
		fg(DARK_TEXT).bg(DARK_SELECTION)
	);
	assert_eq!(scope(&theme, "ui.control"), fg(DARK_TEXT).bg(DARK_CONTROL));
	assert_eq!(
		scope(&theme, "ui.control.focus"),
		fg(DARK_TEXT).bg(DARK_CONTROL_FOCUS)
	);
	assert_eq!(
		scope(&theme, "ui.control.pressed"),
		fg(DARK_TEXT).bg(DARK_CONTROL_PRESSED)
	);
	assert_eq!(
		scope(&theme, "ui.control.disabled"),
		fg(DARK_DIM).bg(DARK_POPUP)
	);
	assert_eq!(scope(&theme, "error"), fg(ERROR_ON_DARK));
	for name in TONES {
		assert!(theme.is_derived(name), "{name}");
	}
	for name in ["ui.background", "ui.text", "ui.accent"] {
		assert!(!theme.is_derived(name), "{name}");
	}
}

#[test]
fn light_theme_shifts_the_control_down_and_takes_the_accent_by_fallback_steps() {
	// Акцента нет: первая ступень — цвет текста под фокусом.
	let mut theme = light_base();
	theme.set("ui.text.focus", fg(LIGHT_FOCUS).bold());
	theme.derive();
	assert_eq!(scope(&theme, "ui.accent"), fg(LIGHT_BG).bg(LIGHT_FOCUS));
	assert!(theme.is_derived("ui.accent"));
	assert_eq!(
		scope(&theme, "ui.control"),
		fg(LIGHT_TEXT).bg(LIGHT_CONTROL)
	);
	assert_eq!(scope(&theme, "ui.text.inactive"), fg(LIGHT_DIM));
	assert_eq!(
		scope(&theme, "ui.selection"),
		fg(LIGHT_TEXT).bg(LIGHT_SELECTION)
	);
	assert_eq!(scope(&theme, "error"), fg(ERROR_ON_LIGHT));

	// Вторая ступень — фон выделения.
	let mut theme = light_base();
	theme.set("ui.selection", Style::new().bg(CUSTOM));
	theme.derive();
	assert_eq!(scope(&theme, "ui.accent"), fg(LIGHT_BG).bg(CUSTOM));

	// Последняя — цвет текста.
	let mut theme = light_base();
	theme.derive();
	assert_eq!(scope(&theme, "ui.accent"), fg(LIGHT_BG).bg(LIGHT_TEXT));
}

#[test]
fn dim_and_error_take_helix_scopes_when_the_theme_has_them() {
	let mut theme = dark_base();
	theme
		.set("ui.virtual", fg(CUSTOM))
		.set("diagnostic.error", fg(CUSTOM_OTHER));
	theme.derive();
	assert_eq!(scope(&theme, "ui.text.inactive"), fg(CUSTOM));
	assert_eq!(scope(&theme, "ui.window"), fg(CUSTOM));
	assert_eq!(scope(&theme, "error"), fg(CUSTOM_OTHER));
}

#[test]
fn darkness_boundary_and_channel_saturation() {
	let control_of = |background: Color| {
		let mut theme = Theme::new("edge");
		theme
			.set("ui.background", Style::new().bg(background))
			.set("ui.text", fg(DARK_TEXT));
		theme.derive();
		scope(&theme, "ui.control").bg
	};
	// Сумма каналов 382 — ещё тёмная тема, 383 — уже светлая.
	assert_eq!(
		control_of(Color::from_rgb(128, 127, 127)),
		Some(Color::from_rgb(166, 165, 165))
	);
	assert_eq!(
		control_of(Color::from_rgb(128, 128, 127)),
		Some(Color::from_rgb(90, 90, 89))
	);
	// Канал не переполняется: 250 + 38 → 255, а в светлой теме 20 − 38 → 0.
	assert_eq!(
		control_of(Color::from_rgb(250, 10, 10)),
		Some(Color::from_rgb(255, 48, 48))
	);
	assert_eq!(
		control_of(Color::from_rgb(255, 255, 20)),
		Some(Color::from_rgb(217, 217, 0))
	);
}

#[test]
fn widget_scopes_are_links_to_tones() {
	let mut theme = dark_base();
	// Текст под фокусом задан жирным: ссылка «целиком» несёт начертание, ссылка «только цвет
	// символа» — нет.
	theme.set("ui.text.focus", fg(CUSTOM).bold());
	theme.derive();
	let links = [
		("ui.button", "ui.control"),
		("ui.button.focus", "ui.accent"),
		("ui.button.pressed", "ui.control.pressed"),
		("ui.button.disabled", "ui.control.disabled"),
		("ui.checkbox.focus", "ui.text.focus"),
		("ui.checkbox.pressed", "ui.selection"),
		("ui.checkbox.disabled", "ui.text.inactive"),
		("ui.input", "ui.control"),
		("ui.input.focus", "ui.control.focus"),
		("ui.input.selection", "ui.selection"),
		("ui.input.placeholder", "ui.text.inactive"),
		("ui.combo", "ui.input"),
		("ui.combo.focus", "ui.input.focus"),
		("ui.combo.selected", "ui.list.selected"),
		("ui.list.selected", "ui.selection"),
		("ui.table.selected", "ui.selection"),
		("ui.tabs.focus", "ui.text.focus"),
	];
	for (widget, tone) in links {
		assert_eq!(
			scope(&theme, widget),
			scope(&theme, tone),
			"{widget} ← {tone}"
		);
		assert!(theme.is_derived(widget), "{widget}");
	}
	for name in ["ui.table.header", "ui.tabs.selected", "ui.scrollbar.thumb"] {
		assert_eq!(scope(&theme, name), fg(CUSTOM), "{name}");
		assert_eq!(scope(&theme, name).add, CellFlags::empty(), "{name}");
	}
	for name in ["ui.scrollbar", "ui.gauge.unfilled"] {
		assert_eq!(scope(&theme, name), fg(DARK_DIM), "{name}");
	}
	assert_eq!(
		scope(&theme, "ui.gauge.filled"),
		fg(ACCENT).bg(DARK_CONTROL)
	);
	// Перечень теста покрывает все области виджетов.
	let checked = links.len() + 6;
	assert_eq!(checked, WIDGET_SCOPES.len());
}

#[test]
fn accent_given_as_background_only_is_completed_with_the_text_on_it() {
	let mut theme = dark_base();
	theme.set("ui.accent", Style::new().bg(ACCENT));
	theme.derive();
	// Заданная область остаётся как есть, а кнопка под фокусом получает полную пару.
	assert_eq!(scope(&theme, "ui.accent"), Style::new().bg(ACCENT));
	assert_eq!(scope(&theme, "ui.button.focus"), fg(DARK_BG).bg(ACCENT));
}

#[test]
fn a_tone_given_in_the_theme_reaches_every_widget_that_links_to_it() {
	let mut theme = dark_base();
	theme.set("ui.control", fg(CUSTOM).bg(gray(100)));
	theme.derive();
	assert!(!theme.is_derived("ui.control"));
	for name in ["ui.button", "ui.input", "ui.combo"] {
		assert_eq!(scope(&theme, name), fg(CUSTOM).bg(gray(100)), "{name}");
	}
	// Состояния считаются от заданного тона: (100·3 + 240 + 2) / 4, (100·3 + 160 + 2) / 4, (100·3 + 2) / 4.
	assert_eq!(
		scope(&theme, "ui.control.focus"),
		fg(CUSTOM).bg(Color::from_rgb(135, 115, 75))
	);
	assert_eq!(
		scope(&theme, "ui.input.focus"),
		scope(&theme, "ui.control.focus")
	);
	// Неактивный: фон (16 + 100 + 1) / 2.
	assert_eq!(
		scope(&theme, "ui.button.disabled"),
		fg(DARK_DIM).bg(gray(58))
	);
	assert_eq!(scope(&theme, "ui.gauge.filled"), fg(ACCENT).bg(gray(100)));
}

#[test]
fn a_widget_scope_given_in_the_theme_is_stronger_than_the_tone() {
	let mut theme = dark_base();
	theme
		.set("ui.button", fg(CUSTOM))
		.set("ui.input", Style::new().bg(CUSTOM_OTHER));
	theme.derive();
	assert_eq!(scope(&theme, "ui.button"), fg(CUSTOM));
	assert!(!theme.is_derived("ui.button"));
	// Соседние области того же виджета по-прежнему выводятся.
	assert_eq!(
		scope(&theme, "ui.button.pressed"),
		scope(&theme, "ui.control.pressed")
	);
	// Выпадающий список ссылается на поле — заданное, а не на тон.
	assert_eq!(scope(&theme, "ui.combo"), Style::new().bg(CUSTOM_OTHER));
}

/// Тема, описанная вручную целиком: основа и все тона заданы — вывод не приносит ни одного
/// цвета, которого нет в заданных областях (К3а).
#[test]
fn a_theme_described_by_hand_gets_no_computed_color() {
	let mut theme = dark_base();
	theme.set("error", fg(CUSTOM));
	for (index, name) in TONES.into_iter().enumerate() {
		let shade = gray(60 + index as u8);
		theme.set(name, Style::new().fg(shade).bg(gray(160 + index as u8)));
	}
	let given: Vec<Color> = theme
		.scopes()
		.flat_map(|(_, style)| [style.fg, style.bg])
		.flatten()
		.collect();
	theme.derive();
	for name in WIDGET_SCOPES {
		let style = scope(&theme, name);
		for color in [style.fg, style.bg].into_iter().flatten() {
			assert!(
				given.contains(&color),
				"{name}: цвет {color:?} не задан в теме"
			);
		}
		assert!(theme.is_derived(name), "{name}");
	}
	for name in TONES {
		assert!(!theme.is_derived(name), "{name}");
	}
}

#[test]
fn derive_is_repeatable_and_set_makes_a_derived_scope_given() {
	let mut theme = dark();
	let derived_once = theme.clone();
	theme.derive();
	assert_eq!(theme, derived_once, "повторный вывод ничего не меняет");

	// Правка основы видна после нового вывода.
	theme.set("ui.accent", fg(ON_ACCENT).bg(CUSTOM));
	assert_eq!(scope(&theme, "ui.button.focus"), fg(ON_ACCENT).bg(ACCENT));
	theme.derive();
	assert_eq!(scope(&theme, "ui.button.focus"), fg(ON_ACCENT).bg(CUSTOM));
	assert_eq!(scope(&theme, "ui.text.focus"), fg(CUSTOM));

	// Выведенная область, заданная через `set`, переживает вывод.
	assert!(theme.is_derived("ui.list.selected"));
	theme.set("ui.list.selected", fg(CUSTOM_OTHER));
	assert!(!theme.is_derived("ui.list.selected"));
	theme.derive();
	assert_eq!(scope(&theme, "ui.list.selected"), fg(CUSTOM_OTHER));
	assert_eq!(scope(&theme, "ui.combo.selected"), fg(CUSTOM_OTHER));
}

fn colors(buf: &ScreenBuffer, x: usize) -> (Color, Color) {
	let cell = buf.cell(x, 0);
	(cell.fg, cell.bg)
}

/// Кадр дерева на теме из одной основы (К4): кнопки берут выведенные области, явный стиль
/// виджета сильнее.
#[test]
fn buttons_of_a_base_only_theme_are_painted_with_derived_scopes() {
	let theme = dark();
	let explicit = Style::new().bg(CUSTOM);
	let (mut focused, mut plain, mut pressed, mut styled) = (
		ButtonState::new(),
		ButtonState::new(),
		ButtonState::new(),
		ButtonState::new(),
	);
	let mut disabled = ButtonState::disabled();
	pressed.pressed = true;
	let mut buf = screen(20, 1);
	Ui::<Msg>::split(
		Layout::horizontal([Constraint::Length(4); 5]),
		vec![
			Ui::stateful(Button::new("A"), &mut focused),
			Ui::stateful(Button::new("B"), &mut plain),
			Ui::stateful(Button::new("C"), &mut pressed),
			Ui::stateful(Button::new("D"), &mut disabled),
			Ui::stateful(Button::new("E").style(explicit), &mut styled),
		],
	)
	.theme(&theme)
	.render(Rect::new(0, 0, 20, 1), &mut buf);
	assert_eq!(colors(&buf, 0), (ON_ACCENT, ACCENT), "под фокусом");
	assert_eq!(colors(&buf, 4), (DARK_TEXT, DARK_CONTROL), "обычная");
	assert_eq!(colors(&buf, 8), (DARK_TEXT, DARK_CONTROL_PRESSED), "нажата");
	assert_eq!(colors(&buf, 12), (DARK_DIM, DARK_POPUP), "неактивная");
	assert_eq!(colors(&buf, 16), (DARK_TEXT, CUSTOM), "явный стиль");
}

/// Встроенная тема выведена, но выглядит как раньше: её области виджетов заданы явно, а
/// недостающие берут те же тона, что и запасные цепочки ADR-0037.
#[test]
fn built_in_theme_is_derived_and_keeps_its_given_widget_scopes() {
	let theme = Theme::default();
	assert!(theme.is_derived("ui.control"));
	assert!(!theme.is_derived("ui.accent"));
	for name in [
		"ui.button",
		"ui.input",
		"ui.list.selected",
		"ui.tabs.selected",
	] {
		assert!(!theme.is_derived(name), "{name}");
	}
	for (widget, tone) in [
		("ui.checkbox.focus", "ui.text.focus"),
		("ui.tabs.focus", "ui.text.focus"),
		("ui.checkbox.pressed", "ui.selection"),
		("ui.input.selection", "ui.selection"),
		("ui.input.placeholder", "ui.text.inactive"),
		("ui.combo", "ui.input"),
		("ui.combo.selected", "ui.list.selected"),
	] {
		assert_eq!(scope(&theme, widget), scope(&theme, tone), "{widget}");
	}
}

#[cfg(feature = "theme-toml")]
mod files {
	use std::path::PathBuf;

	use super::{ACCENT, DARK_BG, DARK_CONTROL, DARK_TEXT, TONES, WIDGET_SCOPES, fg, scope};
	use gterm_widgets::{Style, Theme};

	/// Темы репозитория в `examples/themes/`: две свои и 22 темы sniffnet.
	const REPOSITORY_THEMES: [&str; 24] = [
		"gterm_dark",
		"gterm_light",
		"sniffnet_a11y_dark",
		"sniffnet_a11y_light",
		"sniffnet_catppuccin",
		"sniffnet_deep_cosmos",
		"sniffnet_deep_sea",
		"sniffnet_dracula_dark",
		"sniffnet_dracula_light",
		"sniffnet_electric_wave",
		"sniffnet_goleador",
		"sniffnet_gruvbox_dark",
		"sniffnet_gruvbox_light",
		"sniffnet_heat_waves",
		"sniffnet_light_bubbles",
		"sniffnet_mon_amour",
		"sniffnet_nord_dark",
		"sniffnet_nord_light",
		"sniffnet_seasons",
		"sniffnet_solarized_dark",
		"sniffnet_solarized_light",
		"sniffnet_toucan",
		"sniffnet_yeti_dark",
		"sniffnet_yeti_light",
	];

	fn example_themes_dir() -> PathBuf {
		PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/themes")
	}

	#[test]
	fn a_loaded_theme_is_derived() {
		let text = r##"
			"ui.background" = { bg = "#101010" }
			"ui.text" = "#D0D0D0"
			"ui.accent" = { fg = "#000000", bg = "#F0A000" }
			"ui.button.pressed" = { bg = "#010203" }
		"##;
		let theme = Theme::from_toml_str(text).expect("тема из текста");
		assert_eq!(scope(&theme, "ui.button"), fg(DARK_TEXT).bg(DARK_CONTROL));
		assert!(theme.is_derived("ui.button"));
		assert!(!theme.is_derived("ui.button.pressed"));
		assert_eq!(theme.base(), Style::new().fg(DARK_TEXT).bg(DARK_BG));
		assert_eq!(scope(&theme, "ui.text.focus"), fg(ACCENT));
		assert!(theme.warnings().is_empty());
	}

	/// Все темы каталога примеров — свои и sniffnet (К6): загружаются без предупреждений и
	/// после вывода несут основу, все тона и все области виджетов.
	#[test]
	fn every_example_theme_loads_clean_and_is_complete() {
		let dir = example_themes_dir();
		let names = Theme::list(&[&dir]).expect("перечень тем примеров");
		// Каталог может держать и темы, сохранённые из примера `theme`: проверяется наличие
		// тем репозитория, а не число файлов.
		for expected in REPOSITORY_THEMES {
			assert!(
				names.iter().any(|name| name == expected),
				"нет темы {expected}"
			);
		}
		for name in names {
			let theme = Theme::load(&[&dir], &name).unwrap_or_else(|e| panic!("{name}: {e}"));
			assert!(
				theme.warnings().is_empty(),
				"{name}: {:?}",
				theme.warnings()
			);
			assert!(
				!theme.is_derived("ui.accent"),
				"{name}: акцент задан в файле"
			);
			for scope_name in TONES.into_iter().chain(WIDGET_SCOPES) {
				assert!(theme.get(scope_name).is_some(), "{name}: нет {scope_name}");
			}
		}
	}
}
