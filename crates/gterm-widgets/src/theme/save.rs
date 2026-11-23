//! Полная запись темы в текст формата helix (поправка ADR-0038 от 2026-11-23): все области —
//! заданные и выведенные — записаны явно, по уровням темы, выведенные помечены комментарием.
//! Такой файл можно править или взять за основу новой темы: загрузка примет его как тему,
//! описанную вручную, а строку, удалённую из тонов или областей виджетов, выведет заново.
//!
//! Цвет записывается именем палитры, если он в ней есть, иначе — `#RRGGBB` (`#RRGGBBAA` при
//! неполной альфе). Снятие флагов (`Style::remove`) в формате helix не выражается и в запись не
//! попадает; у тем из файлов его нет.

use std::collections::BTreeSet;

use gterm::cell::{CellFlags, Color};

use super::Theme;
use super::derive::{BASE_SCOPES, TONE_SCOPES, widget_scopes};
use crate::style::Style;

/// Пометка области, созданной выводом.
const DERIVED_COMMENT: &str = "  # выведено";

const MODIFIER_NAMES: [(CellFlags, &str); 3] = [
	(CellFlags::BOLD, "bold"),
	(CellFlags::ITALIC, "italic"),
	(CellFlags::UNDERLINE, "underlined"),
];

impl Theme {
	/// Текст темы в формате helix со всеми областями, включая выведенные. Загрузка этого текста
	/// ([`Theme::from_toml_str`], [`Theme::load`]) даёт те же области и палитру; выведенных
	/// среди них уже нет — все заданы.
	pub fn to_toml_string(&self) -> String {
		let mut text = format!(
			"# Тема «{}» — полная запись: заданы все области, в том числе выведенные из основы\n\
			 # (помечены «выведено»). Файл можно править или взять за основу новой темы; строка,\n\
			 # удалённая из тонов или областей виджетов, при загрузке выводится заново (ADR-0038).\n",
			self.name.replace(['\n', '\r'], " ")
		);
		let mut written = BTreeSet::new();
		let widgets: Vec<&str> = widget_scopes().collect();
		for (title, names) in [
			("Основа", &BASE_SCOPES[..]),
			("Тона", &TONE_SCOPES[..]),
			("Области виджетов", &widgets[..]),
		] {
			let present: Vec<&str> = names
				.iter()
				.copied()
				.filter(|name| self.scopes.contains_key(*name))
				.collect();
			self.push_section(&mut text, title, &present);
			written.extend(present);
		}
		let rest: Vec<&str> = self
			.scopes
			.keys()
			.map(String::as_str)
			.filter(|name| !written.contains(name))
			.collect();
		self.push_section(&mut text, "Остальные области", &rest);
		if !self.palette.is_empty() {
			text.push_str("\n[palette]\n");
			for (name, color) in &self.palette {
				text.push_str(&format!("{} = \"{}\"\n", key(name), hex(*color)));
			}
		}
		text
	}

	/// Раздел файла: заголовок-комментарий и по строке на область; пустой раздел не пишется.
	fn push_section(&self, text: &mut String, title: &str, names: &[&str]) {
		if names.is_empty() {
			return;
		}
		text.push_str(&format!("\n# {title}\n"));
		for name in names {
			let style = self.scopes[*name];
			let comment = if self.is_derived(name) {
				DERIVED_COMMENT
			} else {
				""
			};
			text.push_str(&format!(
				"{} = {}{comment}\n",
				quoted(name),
				self.style_value(style)
			));
		}
	}

	/// Значение области: цвет символа строкой либо таблица `{ fg, bg, modifiers }`.
	fn style_value(&self, style: Style) -> String {
		let modifiers: Vec<String> = MODIFIER_NAMES
			.iter()
			.filter(|(flag, _)| style.add.contains(*flag))
			.map(|(_, name)| quoted(name))
			.collect();
		if let (Some(fg), None, true) = (style.fg, style.bg, modifiers.is_empty()) {
			return quoted(&self.color_name(fg));
		}
		let mut parts = Vec::new();
		if let Some(fg) = style.fg {
			parts.push(format!("fg = {}", quoted(&self.color_name(fg))));
		}
		if let Some(bg) = style.bg {
			parts.push(format!("bg = {}", quoted(&self.color_name(bg))));
		}
		if !modifiers.is_empty() {
			parts.push(format!("modifiers = [{}]", modifiers.join(", ")));
		}
		if parts.is_empty() {
			return "{}".to_owned();
		}
		format!("{{ {} }}", parts.join(", "))
	}

	/// Имя цвета в палитре (первое по алфавиту, если их несколько), иначе запись цвета.
	fn color_name(&self, color: Color) -> String {
		self.palette
			.iter()
			.find(|(_, known)| **known == color)
			.map_or_else(|| hex(color), |(name, _)| name.clone())
	}
}

/// Запись цвета: `#RRGGBB`, с неполной альфой — `#RRGGBBAA`.
fn hex(color: Color) -> String {
	let rgb = format!("#{:02X}{:02X}{:02X}", color.r(), color.g(), color.b());
	if color.a() == u8::MAX {
		return rgb;
	}
	format!("{rgb}{:02X}", color.a())
}

/// Строка TOML в двойных кавычках.
fn quoted(text: &str) -> String {
	let mut result = String::with_capacity(text.len() + 2);
	result.push('"');
	for ch in text.chars() {
		match ch {
			'"' => result.push_str("\\\""),
			'\\' => result.push_str("\\\\"),
			control if control.is_control() => {
				result.push_str(&format!("\\u{:04X}", u32::from(control)));
			}
			other => result.push(other),
		}
	}
	result.push('"');
	result
}

/// Ключ таблицы TOML: без кавычек, если состоит из букв ASCII, цифр, `_` и `-`.
fn key(name: &str) -> String {
	let is_bare = !name.is_empty()
		&& name
			.chars()
			.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-');
	if is_bare {
		name.to_owned()
	} else {
		quoted(name)
	}
}
