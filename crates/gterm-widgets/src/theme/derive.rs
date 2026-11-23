//! Вывод недостающих областей темы (ADR-0038). Тема читается на трёх уровнях, и у каждого
//! элемента есть имя — область: основа (`ui.background`, `ui.text`, `ui.accent`, `error`), тона
//! (`ui.control*`, `ui.selection`, `ui.popup`, …) и области виджетов (`ui.button*`, `ui.input*`,
//! …). Заданное в теме берётся как есть; недостающее выводится из уровня выше и записывается в
//! тему под тем же именем — поэтому виджеты находят его первым звеном своих цепочек, а автор
//! темы может поправить один тон или описать всё вручную.
//!
//! Арифметика целочисленная; жирное начертание не выводится — `BOLD` осветляет цвет (ADR-0003).

use gterm::cell::Color;

use super::{Theme, fg_only, scope};
use crate::style::Style;

/// Новые области библиотеки: акцент с текстом на нём и тона элемента управления.
const ACCENT: &str = "ui.accent";
const CONTROL: &str = "ui.control";
const CONTROL_FOCUS: &str = "ui.control.focus";
const CONTROL_PRESSED: &str = "ui.control.pressed";
const CONTROL_DISABLED: &str = "ui.control.disabled";

/// Области основы — по ним запись темы делит файл на разделы.
pub(super) const BASE_SCOPES: [&str; 4] = [scope::BACKGROUND, scope::TEXT, ACCENT, ERROR];

/// Тона: выводятся из основы, на них ссылаются области виджетов.
pub(super) const TONE_SCOPES: [&str; 10] = [
	scope::TEXT_INACTIVE,
	scope::TEXT_FOCUS,
	scope::WINDOW,
	scope::WINDOW_FOCUS,
	scope::POPUP,
	scope::SELECTION,
	CONTROL,
	CONTROL_FOCUS,
	CONTROL_PRESSED,
	CONTROL_DISABLED,
];

/// Области helix с цветом ошибки.
const ERROR: &str = "error";
const DIAGNOSTIC_ERROR: &str = "diagnostic.error";

/// Тема тёмная, если сумма каналов фона не больше этой (половина от 3 × 255).
const DARK_SUM_MAX: u16 = 382;
/// На сколько каждый канал фона сдвигается для фона элемента управления (15 % от 255).
const CONTROL_SHIFT: u8 = 38;

/// Цвет ошибки для темы, которая его не задаёт: на тёмном фоне и на светлом.
const ERROR_ON_DARK: Color = Color::from_rgb(0xFF, 0x66, 0x66);
const ERROR_ON_LIGHT: Color = Color::from_rgb(0xB3, 0x00, 0x00);

/// Доля второго цвета в смеси: `part` из `whole`.
#[derive(Clone, Copy)]
struct Share {
	part: u32,
	whole: u32,
}

const HALF: Share = Share { part: 1, whole: 2 };
const THIRD: Share = Share { part: 1, whole: 3 };
const QUARTER: Share = Share { part: 1, whole: 4 };

/// Откуда берётся недостающая область виджета.
#[derive(Clone, Copy)]
enum Link {
	/// Стиль другой области целиком — тона или области виджета выше по перечню.
	Scope(&'static str),
	/// Только цвет символа: цвет текста под фокусом.
	FocusColor,
	/// Только цвет символа: цвет рамки.
	BorderColor,
	/// Заливка индикатора: акцент на фоне элемента управления.
	GaugeFill,
}

/// Области виджетов и их источники. Порядок важен: область ниже может ссылаться на область выше
/// (`ui.combo` — на `ui.input`). Там, где свои темы библиотеки область не задают, ссылка
/// повторяет запасную цепочку ADR-0037 — их вид вывод не меняет.
const WIDGET_LINKS: [(&str, Link); 23] = [
	("ui.button", Link::Scope(CONTROL)),
	("ui.button.focus", Link::Scope(ACCENT)),
	("ui.button.pressed", Link::Scope(CONTROL_PRESSED)),
	("ui.button.disabled", Link::Scope(CONTROL_DISABLED)),
	("ui.checkbox.focus", Link::Scope(scope::TEXT_FOCUS)),
	("ui.checkbox.pressed", Link::Scope(scope::SELECTION)),
	("ui.checkbox.disabled", Link::Scope(scope::TEXT_INACTIVE)),
	("ui.input", Link::Scope(CONTROL)),
	("ui.input.focus", Link::Scope(CONTROL_FOCUS)),
	("ui.input.selection", Link::Scope(scope::SELECTION)),
	("ui.input.placeholder", Link::Scope(scope::TEXT_INACTIVE)),
	("ui.list.selected", Link::Scope(scope::SELECTION)),
	("ui.table.selected", Link::Scope(scope::SELECTION)),
	("ui.table.header", Link::FocusColor),
	("ui.combo", Link::Scope("ui.input")),
	("ui.combo.focus", Link::Scope("ui.input.focus")),
	("ui.combo.selected", Link::Scope("ui.list.selected")),
	("ui.tabs.selected", Link::FocusColor),
	("ui.tabs.focus", Link::Scope(scope::TEXT_FOCUS)),
	("ui.scrollbar", Link::BorderColor),
	("ui.scrollbar.thumb", Link::FocusColor),
	("ui.gauge.filled", Link::GaugeFill),
	("ui.gauge.unfilled", Link::BorderColor),
];

/// Имена областей виджетов в порядке перечня ссылок.
pub(super) fn widget_scopes() -> impl Iterator<Item = &'static str> {
	WIDGET_LINKS.iter().map(|(name, _)| *name)
}

/// Цвета основы: заданные в теме либо взятые по запасным ступеням.
struct Base {
	background: Color,
	text: Color,
	accent: Color,
	/// Текст на акценте.
	on_accent: Color,
}

impl Theme {
	/// Вывести недостающие области из основы (ADR-0038). Заданные области не трогаются;
	/// выведенные прежним вызовом отбрасываются и считаются заново — после правки основы или
	/// тона через [`Theme::set`] вызов даёт согласованную тему. Без `bg` у `ui.background` или
	/// `fg` у `ui.text` ничего не выводится. При загрузке из файла и во встроенной теме вывод
	/// уже выполнен.
	pub fn derive(&mut self) -> &mut Self {
		for name in std::mem::take(&mut self.derived) {
			self.scopes.remove(&name);
		}
		let Some(base) = self.base_colors() else {
			return self;
		};
		self.derive_base(&base);
		self.derive_tones(&base);
		self.derive_widget_scopes(&base);
		self
	}

	/// Область создана выводом, а не задана в файле или через [`Theme::set`].
	pub fn is_derived(&self, scope: &str) -> bool {
		self.derived.contains(scope)
	}

	fn base_colors(&self) -> Option<Base> {
		let background = self.bg_of(scope::BACKGROUND)?;
		let text = self.fg_of(scope::TEXT)?;
		let accent = self
			.bg_of(ACCENT)
			.or_else(|| self.fg_of(scope::TEXT_FOCUS))
			.or_else(|| self.bg_of(scope::SELECTION))
			.unwrap_or(text);
		Some(Base {
			background,
			text,
			accent,
			on_accent: self.fg_of(ACCENT).unwrap_or(background),
		})
	}

	fn derive_base(&mut self, base: &Base) {
		self.put(ACCENT, accent_pair(base));
		let error = self
			.fg_of(DIAGNOSTIC_ERROR)
			.unwrap_or(if is_dark(base.background) {
				ERROR_ON_DARK
			} else {
				ERROR_ON_LIGHT
			});
		self.put(ERROR, Style::new().fg(error));
	}

	fn derive_tones(&mut self, base: &Base) {
		let dim = self.dim_color(base);
		let shifted = shift(base.background);
		self.put(scope::TEXT_INACTIVE, Style::new().fg(dim));
		self.put(scope::TEXT_FOCUS, Style::new().fg(base.accent));
		self.put(scope::WINDOW, Style::new().fg(dim));
		self.put(scope::WINDOW_FOCUS, Style::new().fg(base.accent));
		self.put(
			scope::POPUP,
			Style::new().bg(mix(base.background, shifted, HALF)),
		);
		self.put(
			scope::SELECTION,
			Style::new()
				.fg(base.text)
				.bg(mix(base.background, base.accent, THIRD)),
		);
		self.put(CONTROL, Style::new().fg(base.text).bg(shifted));
		// Состояния считаются от тона `ui.control` — заданного или только что выведенного:
		// поправка одного тона в файле доходит до его состояний.
		let control_fg = self.fg_of(CONTROL).unwrap_or(base.text);
		let control_bg = self.bg_of(CONTROL).unwrap_or(shifted);
		self.put(
			CONTROL_FOCUS,
			Style::new()
				.fg(control_fg)
				.bg(mix(control_bg, base.accent, QUARTER)),
		);
		self.put(
			CONTROL_PRESSED,
			Style::new()
				.fg(control_fg)
				.bg(mix(control_bg, base.accent, HALF)),
		);
		self.put(
			CONTROL_DISABLED,
			Style::new()
				.fg(dim)
				.bg(mix(base.background, control_bg, HALF)),
		);
	}

	fn derive_widget_scopes(&mut self, base: &Base) {
		let focus_color = self.fg_of(scope::TEXT_FOCUS).unwrap_or(base.accent);
		let border_color = self
			.fg_of(scope::WINDOW)
			.unwrap_or_else(|| self.dim_color(base));
		let control_bg = self
			.bg_of(CONTROL)
			.unwrap_or_else(|| shift(base.background));
		for (name, link) in WIDGET_LINKS {
			let style = match link {
				// Заданный `ui.accent` может нести только фон: пара дополняется текстом на нём.
				Link::Scope(ACCENT) => accent_pair(base).patch(self.first(&[ACCENT])),
				Link::Scope(source) => self.first(&[source]),
				Link::FocusColor => Style::new().fg(focus_color),
				Link::BorderColor => Style::new().fg(border_color),
				Link::GaugeFill => Style::new().fg(base.accent).bg(control_bg),
			};
			self.put(name, style);
		}
	}

	/// Цвет приглушённого текста: заданный, из `ui.virtual` либо середина между текстом и фоном.
	fn dim_color(&self, base: &Base) -> Color {
		fg_only(self.first(&[scope::TEXT_INACTIVE, scope::VIRTUAL]))
			.fg
			.unwrap_or_else(|| mix(base.text, base.background, HALF))
	}

	/// Записать выведенную область, если такой в теме нет.
	fn put(&mut self, name: &str, style: Style) {
		if self.scopes.contains_key(name) {
			return;
		}
		self.scopes.insert(name.to_owned(), style);
		self.derived.insert(name.to_owned());
	}

	fn fg_of(&self, scope: &str) -> Option<Color> {
		self.get(scope)?.fg
	}

	fn bg_of(&self, scope: &str) -> Option<Color> {
		self.get(scope)?.bg
	}
}

/// Акцент и текст на нём.
fn accent_pair(base: &Base) -> Style {
	Style::new().fg(base.on_accent).bg(base.accent)
}

fn is_dark(background: Color) -> bool {
	let sum = u16::from(background.r()) + u16::from(background.g()) + u16::from(background.b());
	sum <= DARK_SUM_MAX
}

/// Фон элемента управления: фон темы, сдвинутый к середине — светлее в тёмной теме, темнее в
/// светлой.
fn shift(background: Color) -> Color {
	let channel = |value: u8| {
		if is_dark(background) {
			value.saturating_add(CONTROL_SHIFT)
		} else {
			value.saturating_sub(CONTROL_SHIFT)
		}
	};
	Color::from_rgba(
		channel(background.r()),
		channel(background.g()),
		channel(background.b()),
		background.a(),
	)
}

/// Поканальная смесь: `share` — доля цвета `other`; округление до ближайшего.
fn mix(color: Color, other: Color, share: Share) -> Color {
	let Share { part, whole } = share;
	let channel = |from: u8, to: u8| {
		let mixed = (u32::from(from) * (whole - part) + u32::from(to) * part + whole / 2) / whole;
		// Взвешенное среднее двух байтов не выходит за байт.
		mixed as u8
	};
	Color::from_rgba(
		channel(color.r(), other.r()),
		channel(color.g(), other.g()),
		channel(color.b(), other.b()),
		channel(color.a(), other.a()),
	)
}
