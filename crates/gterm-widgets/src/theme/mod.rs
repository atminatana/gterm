//! Тема оформления (ADR-0037): области «имя → стиль» и палитра «имя → цвет» по образцу тем
//! редактора helix. Тема — значение в состоянии приложения: подключается к описанию экрана
//! ([`Ui::theme`](crate::ui::Ui::theme)) и неявно доходит до каждого виджета, а области своей
//! раскраски знает сам виджет (`apply_theme` у трейтов виджетов). Хранятся все области файла, и
//! неизвестные библиотеке: приложение читает свои теми же вызовами [`Theme::style`] и
//! [`Theme::color`].

use std::collections::{BTreeMap, BTreeSet};

use gterm::cell::Color;

use crate::style::Style;

mod derive;
#[cfg(feature = "theme-toml")]
mod load;
mod save;

#[cfg(feature = "theme-toml")]
pub use load::ThemeError;

/// Области helix, общие для нескольких виджетов; свои области виджет держит у себя.
pub(crate) mod scope {
	pub const BACKGROUND: &str = "ui.background";
	pub const TEXT: &str = "ui.text";
	pub const TEXT_FOCUS: &str = "ui.text.focus";
	pub const TEXT_INACTIVE: &str = "ui.text.inactive";
	pub const VIRTUAL: &str = "ui.virtual";
	pub const WINDOW: &str = "ui.window";
	pub const WINDOW_FOCUS: &str = "ui.window.focus";
	pub const POPUP: &str = "ui.popup";
	pub const MENU: &str = "ui.menu";
	pub const MENU_SELECTED: &str = "ui.menu.selected";
	pub const MENU_SCROLL: &str = "ui.menu.scroll";
	pub const SELECTION: &str = "ui.selection";
}

/// Тема: области, палитра и то, что при загрузке отобразить не удалось.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
	name: String,
	scopes: BTreeMap<String, Style>,
	/// Имена областей, созданных выводом (ADR-0038), а не заданных.
	derived: BTreeSet<String>,
	palette: BTreeMap<String, Color>,
	warnings: Vec<String>,
}

impl Theme {
	/// Пустая тема: ни одной области — виджеты рисуются так же, как без темы.
	pub fn new<N: Into<String>>(name: N) -> Self {
		Self {
			name: name.into(),
			scopes: BTreeMap::new(),
			derived: BTreeSet::new(),
			palette: BTreeMap::new(),
			warnings: Vec::new(),
		}
	}

	pub fn name(&self) -> &str {
		&self.name
	}

	/// Задать или заменить область. Область, созданная выводом, становится заданной; зависящие
	/// от неё выведенные области пересчитывает следующий [`Theme::derive`].
	pub fn set<N: Into<String>>(&mut self, scope: N, style: Style) -> &mut Self {
		let name = scope.into();
		self.derived.remove(&name);
		self.scopes.insert(name, style);
		self
	}

	/// Задать или заменить цвет палитры.
	pub fn set_color<N: Into<String>>(&mut self, name: N, color: Color) -> &mut Self {
		self.palette.insert(name.into(), color);
		self
	}

	/// Стиль области с точно таким именем.
	pub fn get(&self, scope: &str) -> Option<Style> {
		self.scopes.get(scope).copied()
	}

	/// Стиль области с наследованием по точке, как в helix: нет `a.b.c` — берётся `a.b`, затем
	/// `a`; ничего не найдено — пустой стиль. Память не выделяет.
	pub fn style(&self, scope: &str) -> Style {
		let mut name = scope;
		loop {
			if let Some(style) = self.scopes.get(name) {
				return *style;
			}
			match name.rfind('.') {
				Some(dot) => name = &name[..dot],
				None => return Style::new(),
			}
		}
	}

	/// Стиль первой существующей области из перечня (имена точные, без наследования по точке);
	/// ни одной нет — пустой стиль. Так виджеты описывают запасные цепочки: своя область, затем
	/// области helix.
	pub fn first(&self, scopes: &[&str]) -> Style {
		self.find(scopes).unwrap_or_default()
	}

	/// То же, что [`Theme::first`], но отсутствие всех областей перечня — `None`.
	pub fn find(&self, scopes: &[&str]) -> Option<Style> {
		scopes.iter().find_map(|scope| self.get(scope))
	}

	/// Цвет палитры по имени.
	pub fn color(&self, name: &str) -> Option<Color> {
		self.palette.get(name).copied()
	}

	/// Все области по алфавиту — заданные и выведенные ([`Theme::is_derived`]).
	pub fn scopes(&self) -> impl Iterator<Item = (&str, Style)> {
		self.scopes
			.iter()
			.map(|(name, style)| (name.as_str(), *style))
	}

	/// Что при загрузке пропущено: модификаторы, которых нет у ячейки, области с неизвестным
	/// цветом или значением чужого типа.
	pub fn warnings(&self) -> &[String] {
		&self.warnings
	}

	/// Базовый стиль экрана: цвет текста из `ui.text`, фон из `ui.background`. Его `bg` — цвет
	/// для поля вокруг сетки (`ScreenBuffer::set_background`).
	pub fn base(&self) -> Style {
		let text = self.style(scope::TEXT);
		let background = self.style(scope::BACKGROUND);
		Style {
			fg: text.fg.or(background.fg),
			bg: background.bg,
			..Style::new()
		}
	}

	/// Стиль рамки: только цвет символа `ui.window`. Фон этой области в темах helix — полоса
	/// разделителя окон; на рамке из линий он дал бы сплошные бруски.
	pub(crate) fn border(&self) -> Style {
		fg_only(self.first(&[scope::WINDOW]))
	}

	/// Стиль рамки под фокусом: `ui.window.focus`, иначе цвет символа из `ui.text.focus`.
	pub(crate) fn focus_border(&self) -> Style {
		self.get(scope::WINDOW_FOCUS)
			.unwrap_or_else(|| fg_only(self.first(&[scope::TEXT_FOCUS])))
	}

	/// Стиль неактивного элемента или подсказки: своя область `own`, иначе приглушённый текст.
	pub(crate) fn inactive(&self, own: &str) -> Style {
		self.first(&[own, scope::TEXT_INACTIVE, scope::VIRTUAL])
	}
}

/// Только цвет символа стиля.
pub(crate) fn fg_only(style: Style) -> Style {
	Style {
		fg: style.fg,
		..Style::new()
	}
}

/// Встроенная тёмная тема: задана в коде, файлов не требует (ADR-0017). Области виджетов
/// заданы явно — образец точечной настройки; тона и остальное выведены (ADR-0038).
impl Default for Theme {
	fn default() -> Self {
		const BACKGROUND: Color = Color::from_rgb(0x10, 0x14, 0x1C);
		const TEXT: Color = Color::from_rgb(0xD8, 0xD8, 0xD8);
		const DIM: Color = Color::from_rgb(0x80, 0x80, 0x80);
		const ACCENT: Color = Color::from_rgb(0x80, 0xC0, 0xFF);
		const FIELD: Color = Color::from_rgb(0x20, 0x28, 0x38);
		const FOCUS: Color = Color::from_rgb(0x2A, 0x4A, 0x7A);
		const POPUP: Color = Color::from_rgb(0x18, 0x20, 0x2C);

		let mut theme = Theme::new("default");
		for (name, color) in [
			("background", BACKGROUND),
			("text", TEXT),
			("dim", DIM),
			("accent", ACCENT),
			("field", FIELD),
			("focus", FOCUS),
			("popup", POPUP),
		] {
			theme.set_color(name, color);
		}
		let style = Style::new();
		for (name, value) in [
			(scope::BACKGROUND, style.bg(BACKGROUND)),
			(scope::TEXT, style.fg(TEXT)),
			("ui.accent", style.fg(BACKGROUND).bg(ACCENT)),
			(scope::TEXT_FOCUS, style.fg(ACCENT).bold()),
			(scope::TEXT_INACTIVE, style.fg(DIM)),
			(scope::WINDOW, style.fg(DIM)),
			(scope::WINDOW_FOCUS, style.fg(ACCENT)),
			(scope::POPUP, style.bg(POPUP)),
			(scope::MENU, style.fg(TEXT).bg(FIELD)),
			(scope::MENU_SELECTED, style.fg(BACKGROUND).bg(ACCENT)),
			(scope::MENU_SCROLL, style.fg(ACCENT).bg(FIELD)),
			(scope::SELECTION, style.bg(FOCUS)),
			("ui.button", style.fg(BACKGROUND).bg(DIM)),
			("ui.button.focus", style.bg(ACCENT)),
			("ui.button.pressed", style.fg(TEXT).bg(FOCUS)),
			("ui.button.disabled", style.fg(DIM).bg(FIELD)),
			("ui.input", style.bg(FIELD)),
			("ui.input.focus", style.bg(FOCUS)),
			("ui.list.selected", style.bg(FOCUS).bold()),
			("ui.table.header", style.fg(ACCENT).bold()),
			("ui.table.selected", style.bg(FOCUS).bold()),
			("ui.tabs.selected", style.fg(ACCENT)),
			("ui.scrollbar", style.fg(DIM)),
			("ui.scrollbar.thumb", style.fg(ACCENT)),
			("ui.gauge.filled", style.fg(ACCENT).bg(FIELD)),
		] {
			theme.set(name, value);
		}
		theme.derive();
		theme
	}
}
