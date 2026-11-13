//! Комбинации клавиш (ADR-0028): раскладко-независимая клавиша [`Key`] по физическому коду,
//! дерево удержаний [`Hotkey`] с записью в строку и разбором (`Ctrl+K`, `Ctrl+[K,U]`,
//! `Space+[Ctrl+[K,U]]`, образцы `K|U`) и трекер [`HotkeyTracker`], который приложение кормит
//! событиями `on_key` (нажатия и отпускания) и получает сигнал на каждое отпускание.
//!
//! Библиотека ввод не интерпретирует (ADR-0004): модуль не подключён к циклу событий, окно ему
//! не нужно, `unsafe` нет.

use std::fmt::{self, Display};
use std::str::FromStr;

use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/// Раскладко-независимая клавиша: модификаторы без различения левого и правого и прочие
/// клавиши по физическому коду `winit` (имена позиций US-раскладки).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
	Ctrl,
	Alt,
	Shift,
	Super,
	Code(KeyCode),
}

/// Таблица имён: одна на запись и разбор. Буквы, цифры, стрелки и `Esc` — короткие псевдонимы,
/// остальное — имя варианта `KeyCode`.
macro_rules! key_names {
	($($code:ident => $name:literal),* $(,)?) => {
		fn code_name(code: KeyCode) -> &'static str {
			match code {
				$(KeyCode::$code => $name,)*
				// `KeyCode` — `#[non_exhaustive]`: клавиша будущего `winit` без имени.
				_ => "?",
			}
		}

		/// Принимается и псевдоним, и имя варианта `KeyCode` (`1` и `Digit1`, `Esc` и `Escape`).
		fn code_from_name(name: &str) -> Option<KeyCode> {
			$(if name.eq_ignore_ascii_case($name) || name.eq_ignore_ascii_case(stringify!($code)) {
				return Some(KeyCode::$code);
			})*
			None
		}
	};
}

key_names! {
	Backquote => "Backquote", Backslash => "Backslash", BracketLeft => "BracketLeft",
	BracketRight => "BracketRight", Comma => "Comma", Digit0 => "0", Digit1 => "1",
	Digit2 => "2", Digit3 => "3", Digit4 => "4", Digit5 => "5", Digit6 => "6", Digit7 => "7",
	Digit8 => "8", Digit9 => "9", Equal => "Equal", IntlBackslash => "IntlBackslash",
	IntlRo => "IntlRo", IntlYen => "IntlYen", KeyA => "A", KeyB => "B", KeyC => "C", KeyD => "D",
	KeyE => "E", KeyF => "F", KeyG => "G", KeyH => "H", KeyI => "I", KeyJ => "J", KeyK => "K",
	KeyL => "L", KeyM => "M", KeyN => "N", KeyO => "O", KeyP => "P", KeyQ => "Q", KeyR => "R",
	KeyS => "S", KeyT => "T", KeyU => "U", KeyV => "V", KeyW => "W", KeyX => "X", KeyY => "Y",
	KeyZ => "Z", Minus => "Minus", Period => "Period", Quote => "Quote", Semicolon => "Semicolon",
	Slash => "Slash", Backspace => "Backspace", CapsLock => "CapsLock",
	ContextMenu => "ContextMenu", Enter => "Enter", Space => "Space", Tab => "Tab",
	Convert => "Convert", KanaMode => "KanaMode", Lang1 => "Lang1", Lang2 => "Lang2",
	Lang3 => "Lang3", Lang4 => "Lang4", Lang5 => "Lang5", NonConvert => "NonConvert",
	Delete => "Delete", End => "End", Help => "Help", Home => "Home", Insert => "Insert",
	PageDown => "PageDown", PageUp => "PageUp", ArrowDown => "Down", ArrowLeft => "Left",
	ArrowRight => "Right", ArrowUp => "Up", NumLock => "NumLock", Numpad0 => "Numpad0",
	Numpad1 => "Numpad1", Numpad2 => "Numpad2", Numpad3 => "Numpad3", Numpad4 => "Numpad4",
	Numpad5 => "Numpad5", Numpad6 => "Numpad6", Numpad7 => "Numpad7", Numpad8 => "Numpad8",
	Numpad9 => "Numpad9", NumpadAdd => "NumpadAdd", NumpadBackspace => "NumpadBackspace",
	NumpadClear => "NumpadClear", NumpadClearEntry => "NumpadClearEntry",
	NumpadComma => "NumpadComma", NumpadDecimal => "NumpadDecimal",
	NumpadDivide => "NumpadDivide", NumpadEnter => "NumpadEnter", NumpadEqual => "NumpadEqual",
	NumpadHash => "NumpadHash", NumpadMemoryAdd => "NumpadMemoryAdd",
	NumpadMemoryClear => "NumpadMemoryClear", NumpadMemoryRecall => "NumpadMemoryRecall",
	NumpadMemoryStore => "NumpadMemoryStore", NumpadMemorySubtract => "NumpadMemorySubtract",
	NumpadMultiply => "NumpadMultiply", NumpadParenLeft => "NumpadParenLeft",
	NumpadParenRight => "NumpadParenRight", NumpadStar => "NumpadStar",
	NumpadSubtract => "NumpadSubtract", Escape => "Esc", Fn => "Fn", FnLock => "FnLock",
	PrintScreen => "PrintScreen", ScrollLock => "ScrollLock", Pause => "Pause",
	BrowserBack => "BrowserBack", BrowserFavorites => "BrowserFavorites",
	BrowserForward => "BrowserForward", BrowserHome => "BrowserHome",
	BrowserRefresh => "BrowserRefresh", BrowserSearch => "BrowserSearch",
	BrowserStop => "BrowserStop", Eject => "Eject", LaunchApp1 => "LaunchApp1",
	LaunchApp2 => "LaunchApp2", LaunchMail => "LaunchMail", MediaPlayPause => "MediaPlayPause",
	MediaSelect => "MediaSelect", MediaStop => "MediaStop", MediaTrackNext => "MediaTrackNext",
	MediaTrackPrevious => "MediaTrackPrevious", Power => "Power", Sleep => "Sleep",
	AudioVolumeDown => "AudioVolumeDown", AudioVolumeMute => "AudioVolumeMute",
	AudioVolumeUp => "AudioVolumeUp", WakeUp => "WakeUp", Meta => "Meta", Hyper => "Hyper",
	Turbo => "Turbo", Abort => "Abort", Resume => "Resume", Suspend => "Suspend", Again => "Again",
	Copy => "Copy", Cut => "Cut", Find => "Find", Open => "Open", Paste => "Paste", Props => "Props",
	Select => "Select", Undo => "Undo", Hiragana => "Hiragana", Katakana => "Katakana",
	F1 => "F1", F2 => "F2", F3 => "F3", F4 => "F4", F5 => "F5", F6 => "F6", F7 => "F7", F8 => "F8",
	F9 => "F9", F10 => "F10", F11 => "F11", F12 => "F12", F13 => "F13", F14 => "F14", F15 => "F15",
	F16 => "F16", F17 => "F17", F18 => "F18", F19 => "F19", F20 => "F20", F21 => "F21", F22 => "F22",
	F23 => "F23", F24 => "F24", F25 => "F25", F26 => "F26", F27 => "F27", F28 => "F28", F29 => "F29",
	F30 => "F30", F31 => "F31", F32 => "F32", F33 => "F33", F34 => "F34", F35 => "F35",
}

impl Key {
	/// Клавиша события по физическому коду; `Unidentified` — `None` (событие не учитывается).
	pub fn from_event(event: &KeyEvent) -> Option<Self> {
		match event.physical_key {
			PhysicalKey::Code(code) => Some(Self::from_code(code)),
			PhysicalKey::Unidentified(_) => None,
		}
	}

	/// Левый и правый модификаторы сводятся к одному.
	pub fn from_code(code: KeyCode) -> Self {
		match code {
			KeyCode::ControlLeft | KeyCode::ControlRight => Key::Ctrl,
			KeyCode::AltLeft | KeyCode::AltRight => Key::Alt,
			KeyCode::ShiftLeft | KeyCode::ShiftRight => Key::Shift,
			KeyCode::SuperLeft | KeyCode::SuperRight => Key::Super,
			other => Key::Code(other),
		}
	}

	pub fn is_modifier(self) -> bool {
		!matches!(self, Key::Code(_))
	}

	/// Имя в записи комбинации.
	pub fn name(self) -> &'static str {
		match self {
			Key::Ctrl => "Ctrl",
			Key::Alt => "Alt",
			Key::Shift => "Shift",
			Key::Super => "Super",
			Key::Code(code) => code_name(code),
		}
	}

	/// Клавиша по имени; регистр не важен.
	pub fn parse_name(name: &str) -> Option<Self> {
		if name.eq_ignore_ascii_case("Ctrl") {
			return Some(Key::Ctrl);
		}
		if name.eq_ignore_ascii_case("Alt") {
			return Some(Key::Alt);
		}
		if name.eq_ignore_ascii_case("Shift") {
			return Some(Key::Shift);
		}
		if name.eq_ignore_ascii_case("Super") {
			return Some(Key::Super);
		}
		code_from_name(name).map(Key::Code)
	}

	/// Канонический порядок модификаторов (`Ctrl`, `Alt`, `Shift`, `Super`).
	fn rank(self) -> u8 {
		match self {
			Key::Ctrl => 0,
			Key::Alt => 1,
			Key::Shift => 2,
			Key::Super => 3,
			Key::Code(_) => 4,
		}
	}
}

impl fmt::Display for Key {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(self.name())
	}
}

/// Последовательность узлов одного варианта.
type Seq = Vec<Node>;

/// Узел дерева: клавиша и то, что нажималось, пока она была удержана.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Node {
	key: Key,
	inner: Hotkey,
}

/// Комбинация клавиш — дерево удержаний. Записанная комбинация — один вариант
/// последовательности; образец может содержать альтернативы (`K|U`) на любом уровне. Значение
/// всегда в каноническом виде: подряд удержанные модификаторы упорядочены `Ctrl`, `Alt`,
/// `Shift`, `Super`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Hotkey {
	alts: Vec<Seq>,
}

impl Hotkey {
	/// Пустая комбинация (ничего не нажато).
	pub fn empty() -> Self {
		Self::default()
	}

	/// Одна клавиша.
	pub fn key(key: Key) -> Self {
		Self::chain(&[key])
	}

	/// Цепочка удержаний в порядке нажатия: `[Ctrl, K]` → `Ctrl+K`.
	pub fn chain(keys: &[Key]) -> Self {
		let mut inner = Hotkey::default();
		for &key in keys.iter().rev() {
			inner = Hotkey::single(Node { key, inner });
		}
		inner.canonical()
	}

	fn single(node: Node) -> Self {
		Self {
			alts: vec![vec![node]],
		}
	}

	pub fn is_empty(&self) -> bool {
		self.alts.iter().all(Vec::is_empty)
	}

	/// Один узел без альтернатив — пишется без скобок после `+`.
	fn is_single_node(&self) -> bool {
		self.alts.len() == 1 && self.alts[0].len() == 1
	}

	/// Все клавиши комбинации (с повторами, в порядке записи).
	pub fn keys(&self) -> Vec<Key> {
		let mut keys = Vec::new();
		for seq in &self.alts {
			for node in seq {
				keys.push(node.key);
				keys.extend(node.inner.keys());
			}
		}
		keys
	}

	/// Совпадает ли записанная комбинация с образцом: та же структура; альтернатива образца
	/// совпадает, если совпадает любой её вариант; модификаторы сравниваются в каноническом виде.
	pub fn matches(&self, pattern: &Hotkey) -> bool {
		let recorded = self.alts.first().map_or(&[][..], Vec::as_slice);
		if pattern.alts.is_empty() {
			return recorded.is_empty();
		}
		pattern.alts.iter().any(|alt| {
			alt.len() == recorded.len()
				&& alt
					.iter()
					.zip(recorded)
					.all(|(p, r)| p.key == r.key && r.inner.matches(&p.inner))
		})
	}

	/// Канонический вид: подряд удержанные модификаторы упорядочены.
	fn canonical(self) -> Self {
		Self {
			alts: self
				.alts
				.into_iter()
				.map(|seq| seq.into_iter().map(Node::canonical).collect())
				.collect(),
		}
	}

	/// Корневая последовательность записанной комбинации (создаётся при первом обращении).
	fn root_mut(&mut self) -> &mut Seq {
		if self.alts.is_empty() {
			self.alts.push(Vec::new());
		}
		&mut self.alts[0]
	}

	fn node_mut(&mut self, path: &[usize]) -> &mut Node {
		let (first, rest) = path.split_first().expect("путь к узлу не пуст");
		let mut node = &mut self.alts[0][*first];
		for &index in rest {
			node = &mut node.inner.alts[0][index];
		}
		node
	}

	fn fmt_seq(seq: &Seq, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		for (index, node) in seq.iter().enumerate() {
			if index > 0 {
				f.write_str(",")?;
			}
			Display::fmt(node, f)?;
		}
		Ok(())
	}
}

impl Node {
	/// Канонический вид: подряд удержанные модификаторы в начале цепочки упорядочены; хвост
	/// цепочки (клавиша-не-модификатор или группа) канонизируется рекурсивно.
	fn canonical(self) -> Self {
		let mut modifiers = Vec::new();
		let mut node = self;
		// Хвост: либо узел-не-модификатор, либо то, что удерживал последний модификатор.
		let tail: Hotkey = loop {
			if !node.key.is_modifier() {
				break Hotkey::single(Node {
					key: node.key,
					inner: node.inner.canonical(),
				});
			}
			modifiers.push(node.key);
			if node.inner.is_single_node() {
				node = node.inner.alts.into_iter().next().unwrap().pop().unwrap();
			} else {
				break node.inner.canonical();
			}
		};
		if modifiers.is_empty() {
			return tail.alts.into_iter().next().unwrap().pop().unwrap();
		}
		modifiers.sort_by_key(|key| key.rank());
		let mut inner = tail;
		for key in modifiers.into_iter().rev() {
			inner = Hotkey::single(Node { key, inner });
		}
		inner.alts.into_iter().next().unwrap().pop().unwrap()
	}

	/// Линейная цепочка одиночных вложений (`Ctrl+K+U`): пишется без скобок.
	fn is_linear(&self) -> bool {
		self.inner.is_empty() || self.inner.is_single_node() && self.inner.alts[0][0].is_linear()
	}
}

impl fmt::Display for Node {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.key)?;
		if self.inner.is_empty() {
			return Ok(());
		}
		f.write_str("+")?;
		// Без скобок — продолжение ряда модификаторов (`Ctrl+Shift+[K,U]`) или линейная цепочка
		// клавиш (`Ctrl+K+U`); иначе группа в скобках: `Space+[Ctrl+[K,U]]`.
		if self.inner.is_single_node() {
			let next = &self.inner.alts[0][0];
			if (self.key.is_modifier() && next.key.is_modifier()) || next.is_linear() {
				return Display::fmt(next, f);
			}
		}
		write!(f, "[{}]", self.inner)
	}
}

impl fmt::Display for Hotkey {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		for (index, seq) in self.alts.iter().enumerate() {
			if index > 0 {
				f.write_str("|")?;
			}
			Self::fmt_seq(seq, f)?;
		}
		Ok(())
	}
}

/// Ошибка разбора записи комбинации: позиция в байтах и причина.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
	pub position: usize,
	pub kind: ParseErrorKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseErrorKind {
	/// Имя клавиши не известно.
	UnknownKey(String),
	/// Ожидалось имя клавиши (пустой элемент, `Ctrl+` в конце, `,,`, `[]`).
	ExpectedKey,
	/// Открытая скобка без закрывающей.
	UnclosedBracket,
	/// Лишний символ (закрывающая скобка без открывающей, два знака подряд).
	UnexpectedChar(char),
}

impl fmt::Display for ParseError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match &self.kind {
			ParseErrorKind::UnknownKey(name) => {
				write!(f, "позиция {}: неизвестная клавиша «{name}»", self.position)
			}
			ParseErrorKind::ExpectedKey => {
				write!(f, "позиция {}: ожидалось имя клавиши", self.position)
			}
			ParseErrorKind::UnclosedBracket => {
				write!(f, "позиция {}: не закрыта скобка «[»", self.position)
			}
			ParseErrorKind::UnexpectedChar(ch) => {
				write!(f, "позиция {}: неожиданный символ «{ch}»", self.position)
			}
		}
	}
}

impl std::error::Error for ParseError {}

/// Рекурсивный спуск по грамматике `expr := seq ('|' seq)* ; seq := item (',' item)* ;
/// item := NAME ('+' (item | '[' expr ']'))?`. Пробелы между лексемами игнорируются.
struct Parser<'a> {
	text: &'a str,
	position: usize,
}

impl<'a> Parser<'a> {
	fn new(text: &'a str) -> Self {
		Self { text, position: 0 }
	}

	fn skip_spaces(&mut self) {
		while self.text[self.position..].starts_with(' ') {
			self.position += 1;
		}
	}

	fn peek(&mut self) -> Option<char> {
		self.skip_spaces();
		self.text[self.position..].chars().next()
	}

	fn error(&self, kind: ParseErrorKind) -> ParseError {
		ParseError {
			position: self.position,
			kind,
		}
	}

	fn parse_expr(&mut self) -> Result<Hotkey, ParseError> {
		let mut alts = vec![self.parse_seq()?];
		while self.peek() == Some('|') {
			self.position += 1;
			alts.push(self.parse_seq()?);
		}
		Ok(Hotkey { alts })
	}

	fn parse_seq(&mut self) -> Result<Seq, ParseError> {
		let mut seq = vec![self.parse_item()?];
		while self.peek() == Some(',') {
			self.position += 1;
			seq.push(self.parse_item()?);
		}
		Ok(seq)
	}

	fn parse_item(&mut self) -> Result<Node, ParseError> {
		let key = self.parse_key()?;
		if self.peek() != Some('+') {
			return Ok(Node {
				key,
				inner: Hotkey::default(),
			});
		}
		self.position += 1;
		let inner = if self.peek() == Some('[') {
			let open = self.position;
			self.position += 1;
			let inner = self.parse_expr()?;
			if self.peek() != Some(']') {
				return Err(ParseError {
					position: open,
					kind: ParseErrorKind::UnclosedBracket,
				});
			}
			self.position += 1;
			inner
		} else {
			Hotkey::single(self.parse_item()?)
		};
		Ok(Node { key, inner })
	}

	fn parse_key(&mut self) -> Result<Key, ParseError> {
		self.skip_spaces();
		let start = self.position;
		let rest = &self.text[start..];
		let len = rest
			.find(|ch: char| !ch.is_alphanumeric())
			.unwrap_or(rest.len());
		if len == 0 {
			// Знак грамматики или конец там, где ждали имя, — пустой элемент; иной символ — лишний.
			return Err(match rest.chars().next() {
				Some(ch) if !"+,[]|".contains(ch) => self.error(ParseErrorKind::UnexpectedChar(ch)),
				_ => self.error(ParseErrorKind::ExpectedKey),
			});
		}
		let name = &rest[..len];
		self.position += len;
		Key::parse_name(name).ok_or_else(|| ParseError {
			position: start,
			kind: ParseErrorKind::UnknownKey(name.to_owned()),
		})
	}
}

impl FromStr for Hotkey {
	type Err = ParseError;

	fn from_str(text: &str) -> Result<Self, Self::Err> {
		let mut parser = Parser::new(text);
		let hotkey = parser.parse_expr()?;
		if let Some(ch) = parser.peek() {
			return Err(parser.error(ParseErrorKind::UnexpectedChar(ch)));
		}
		Ok(hotkey.canonical())
	}
}

/// Сигнал трекера на отпускание клавиши.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HotkeyEvent {
	/// Цепочка удержанных клавиш на момент перед отпусканием (включая отпускаемую); при
	/// отпускании последней — полная комбинация за время жизни.
	pub hotkey: Hotkey,
	/// Отпущена последняя клавиша: `hotkey` — полная комбинация, трекер пуст.
	pub complete: bool,
}

/// Удержанная клавиша и путь к её узлу в `full`.
#[derive(Clone, Debug)]
struct Held {
	key: Key,
	path: Vec<usize>,
}

/// Трекер комбинации: живёт от первого нажатия до отпускания всех клавиш. Кормится из
/// `AppHandler::on_key` обоими состояниями; автоповтор, повторные нажатия удержанной клавиши и
/// отпускания неизвестных клавиш игнорируются.
#[derive(Clone, Debug, Default)]
pub struct HotkeyTracker {
	/// История с первого нажатия; не канонизирована — по ней ходят пути `held`.
	full: Hotkey,
	/// Нажатые сейчас, в порядке нажатия.
	held: Vec<Held>,
}

impl HotkeyTracker {
	pub fn new() -> Self {
		Self::default()
	}

	/// Хоть одна клавиша удержана.
	pub fn is_active(&self) -> bool {
		!self.held.is_empty()
	}

	/// Полная комбинация живой последовательности (пустая, если ничего не нажато).
	pub fn full(&self) -> Hotkey {
		self.full.clone().canonical()
	}

	/// Цепочка удержанных сейчас клавиш в порядке нажатия.
	pub fn current(&self) -> Hotkey {
		let keys: Vec<Key> = self.held.iter().map(|held| held.key).collect();
		Hotkey::chain(&keys)
	}

	/// Забыть всё (например, после потери фокуса окна — отпускания до нас не дойдут).
	pub fn reset(&mut self) {
		self.full = Hotkey::default();
		self.held.clear();
	}

	/// Событие `winit` из `on_key`; `Unidentified` не учитывается.
	pub fn feed(&mut self, event: &KeyEvent) -> Option<HotkeyEvent> {
		let key = Key::from_event(event)?;
		self.feed_key(key, event.state, event.repeat)
	}

	/// То же по клавише (точка входа тестов: `KeyEvent` без окна не сконструировать).
	pub fn feed_key(&mut self, key: Key, state: ElementState, repeat: bool) -> Option<HotkeyEvent> {
		if repeat {
			return None;
		}
		match state {
			ElementState::Pressed => {
				self.press(key);
				None
			}
			ElementState::Released => self.release(key),
		}
	}

	fn press(&mut self, key: Key) {
		if self.held.iter().any(|held| held.key == key) {
			return;
		}
		if self.held.is_empty() {
			self.full = Hotkey::default();
		}
		let node = Node {
			key,
			inner: Hotkey::default(),
		};
		let path = match self.held.last() {
			None => {
				let root = self.full.root_mut();
				root.push(node);
				vec![root.len() - 1]
			}
			Some(parent) => {
				let mut path = parent.path.clone();
				let seq = self.full.node_mut(&parent.path).inner.root_mut();
				seq.push(node);
				path.push(seq.len() - 1);
				path
			}
		};
		self.held.push(Held { key, path });
	}

	fn release(&mut self, key: Key) -> Option<HotkeyEvent> {
		let position = self.held.iter().position(|held| held.key == key)?;
		let hotkey = self.current();
		self.held.remove(position);
		if self.held.is_empty() {
			let full = std::mem::take(&mut self.full).canonical();
			return Some(HotkeyEvent {
				hotkey: full,
				complete: true,
			});
		}
		Some(HotkeyEvent {
			hotkey,
			complete: false,
		})
	}
}
