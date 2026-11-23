//! Список элементов темы: по строке на область — заданную в теме или выведенную из основы
//! (ADR-0038, с пометкой), и каждая строка оформлена стилем своей области —
//! так, как область выглядит на экране этой темы (поверх базового стиля `ui.text` на
//! `ui.background`).

use gterm::cell::{CellFlags, Color};
use gterm_widgets::widgets::ListItem;
use gterm_widgets::{Line, Style, Theme};

/// Ширина строки пункта: с запасом на любую ширину окна, лишнее обрезает список. Строка
/// дополнена пробелами, чтобы фон области шёл до правого края.
const ROW_WIDTH: usize = 256;

/// Пометки происхождения области: выведена из основы темы или задана в ней.
const DERIVED_MARK: &str = "≈";
const GIVEN_MARK: &str = " ";

const FLAG_NAMES: [(CellFlags, &str); 3] = [
	(CellFlags::BOLD, "bold"),
	(CellFlags::ITALIC, "italic"),
	(CellFlags::UNDERLINE, "underlined"),
];

/// Пункты списка: пометка происхождения, имя области и то, что она задаёт. Стиль лежит на тексте строки, а не на
/// пункте: стиль текста сильнее стиля выделения, и выделенная строка сохраняет вид области —
/// выделение показывает символ слева.
pub fn items(theme: &Theme) -> Vec<ListItem<'static>> {
	let name_width = theme
		.scopes()
		.map(|(name, _)| name.chars().count())
		.max()
		.unwrap_or(0);
	theme
		.scopes()
		.map(|(name, style)| {
			let origin = if theme.is_derived(name) {
				DERIVED_MARK
			} else {
				GIVEN_MARK
			};
			let text = format!(" {origin} {name:<name_width$}  {}", describe(style));
			let row = format!("{text:<ROW_WIDTH$}");
			ListItem::new(Line::styled(row, shown_style(theme, style)))
		})
		.collect()
}

/// Стиль области на экране темы: незаданные цвета — из базового стиля; флаги, которых у области
/// нет, сняты явно, иначе их добавил бы стиль выделения списка.
fn shown_style(theme: &Theme, scope: Style) -> Style {
	let shown = theme.base().patch(scope);
	shown.remove(CellFlags::all().difference(shown.add))
}

/// Что задаёт область: цвета и начертание; пустая область — прочерк.
fn describe(style: Style) -> String {
	let mut parts = Vec::new();
	if let Some(fg) = style.fg {
		parts.push(format!("fg {}", hex(fg)));
	}
	if let Some(bg) = style.bg {
		parts.push(format!("bg {}", hex(bg)));
	}
	for (flag, name) in FLAG_NAMES {
		if style.add.contains(flag) {
			parts.push(name.to_owned());
		}
	}
	if parts.is_empty() {
		return "—".to_owned();
	}
	parts.join("  ")
}

/// Запись цвета, как в файле темы: `#RRGGBB`, с неполной альфой — `#RRGGBBAA`.
fn hex(color: Color) -> String {
	let rgb = format!("#{:02X}{:02X}{:02X}", color.r(), color.g(), color.b());
	if color.a() == u8::MAX {
		return rgb;
	}
	format!("{rgb}{:02X}", color.a())
}
