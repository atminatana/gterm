//! Общее для тестов виджетов: экран и его текстовый снимок.

#![allow(dead_code)]

use gterm::buffer::ScreenBuffer;
use gterm::cell::Color;

pub const FG: Color = Color::from_rgb(0xC0, 0xC0, 0xC0);
pub const BG: Color = Color::from_rgb(0x10, 0x10, 0x10);

pub fn screen(cols: usize, rows: usize) -> ScreenBuffer {
	ScreenBuffer::with_default_colors(cols, rows, FG, BG)
}

/// Строки экрана: пустая ячейка — пробел.
pub fn rows(screen: &ScreenBuffer) -> Vec<String> {
	(0..screen.rows())
		.map(|row| {
			(0..screen.cols())
				.map(|col| match screen.cell(col, row).ch {
					'\0' => ' ',
					ch => ch,
				})
				.collect()
		})
		.collect()
}

/// Сравнить снимок экрана с ожидаемыми строками (короче экрана — дополняются пробелами).
pub fn assert_screen(screen: &ScreenBuffer, expected: &[&str]) {
	let actual = rows(screen);
	let expected: Vec<String> = expected
		.iter()
		.map(|line| format!("{line:<width$}", width = screen.cols()))
		.collect();
	assert_eq!(
		actual,
		expected,
		"\nполучено:\n{}\nожидалось:\n{}",
		actual.join("\n"),
		expected.join("\n")
	);
}

// ---- Заглушки контролов для тестов дерева (ADR-0026) ----

use std::cell::RefCell;
use std::rc::Rc;

use gterm::cell::CellFlags;
use gterm_widgets::input::{Handled, KeyMsg, Messages, MouseKind, MouseMsg};
use gterm_widgets::{Control, Drawable, NodeState, Rect};
use winit::event::MouseButton;
use winit::keyboard::{Key, ModifiersState};

/// Сообщения тестового приложения.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
	Picked(usize),
	Pressed,
	Custom(&'static str),
	Text(String),
}

/// Журнал вызовов обработчиков: `"<имя>:key:<клавиша>"`, `"<имя>:mouse:<вид>@<col>,<row>"`.
pub type CallLog = Rc<RefCell<Vec<String>>>;

pub fn call_log() -> CallLog {
	Rc::new(RefCell::new(Vec::new()))
}

/// Свой контрол: пишет вызовы в журнал, отвечает заданным `Handled`, по желанию публикует
/// сообщение; рисует область символом `fill` (под фокусом — заглавным).
pub struct Stub {
	pub name: &'static str,
	pub node: NodeState,
	pub focusable: bool,
	pub answer: Handled,
	pub publish: Option<&'static str>,
	pub fill: char,
	pub log: CallLog,
}

impl Stub {
	pub fn new(name: &'static str, focusable: bool, log: &CallLog) -> Self {
		Self {
			name,
			node: NodeState::default(),
			focusable,
			answer: Handled::No,
			publish: None,
			fill: name.chars().next().unwrap_or('?').to_ascii_lowercase(),
			log: Rc::clone(log),
		}
	}

	pub fn answering(mut self, answer: Handled) -> Self {
		self.answer = answer;
		self
	}

	pub fn publishing(mut self, message: &'static str) -> Self {
		self.publish = Some(message);
		self
	}

	fn note(&self, entry: String) {
		self.log.borrow_mut().push(entry);
	}
}

impl Control<Msg> for Stub {
	fn node(&self) -> &NodeState {
		&self.node
	}

	fn node_mut(&mut self) -> &mut NodeState {
		&mut self.node
	}

	fn focusable(&self) -> bool {
		self.focusable
	}

	fn key(&mut self, msg: &KeyMsg<'_>, out: &mut Messages<Msg>) -> Handled {
		self.note(format!("{}:key:{:?}", self.name, msg.key));
		if let Some(message) = self.publish {
			out.push(Msg::Custom(message));
		}
		self.answer
	}

	fn mouse(&mut self, msg: &MouseMsg, out: &mut Messages<Msg>) -> Handled {
		self.note(format!(
			"{}:mouse:{:?}@{},{}",
			self.name, msg.kind, msg.col, msg.row
		));
		if let Some(message) = self.publish {
			out.push(Msg::Custom(message));
		}
		self.answer
	}

	fn text(&mut self, text: &str, out: &mut Messages<Msg>) -> Handled {
		self.note(format!("{}:text:{}", self.name, text));
		if let Some(message) = self.publish {
			out.push(Msg::Custom(message));
		}
		self.answer
	}
}

impl Drawable for Stub {
	fn draw(&mut self, area: Rect, buf: &mut ScreenBuffer) {
		let ch = if self.node.focused {
			self.fill.to_ascii_uppercase()
		} else {
			self.fill
		};
		buf.fill_rect(
			usize::from(area.x),
			usize::from(area.y),
			usize::from(area.width),
			usize::from(area.height),
			ch,
			FG,
			BG,
			CellFlags::empty(),
		);
	}
}

pub fn key_msg<'a>(key: &'a Key, modifiers: ModifiersState) -> KeyMsg<'a> {
	KeyMsg {
		key,
		text: None,
		modifiers,
	}
}

pub fn mouse(kind: MouseKind, button: Option<MouseButton>, col: i32, row: i32) -> MouseMsg {
	MouseMsg {
		kind,
		button,
		col,
		row,
		modifiers: ModifiersState::empty(),
	}
}

pub fn left_down(col: i32, row: i32) -> MouseMsg {
	mouse(MouseKind::Down, Some(MouseButton::Left), col, row)
}

pub fn left_up(col: i32, row: i32) -> MouseMsg {
	mouse(MouseKind::Up, Some(MouseButton::Left), col, row)
}

pub fn moved(col: i32, row: i32) -> MouseMsg {
	mouse(MouseKind::Move, None, col, row)
}
