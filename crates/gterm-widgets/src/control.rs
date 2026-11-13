//! Контрол — трейт на состоянии (ADR-0026): фокус, область последней раскладки и захват мыши
//! лежат в состоянии контрола, поведение — в обработчиках клавиши и мыши. Встроенные состояния
//! (`ListState`, `TableState`, `ButtonState`) и типы приложения реализуют его одинаково; вид у
//! встроенных по-прежнему задаёт виджет-значение (ADR-0023), свой контрол рисует себя сам
//! ([`Drawable`]).

use std::time::Instant;

use gterm::buffer::ScreenBuffer;

use crate::clipboard::{Clipboard, EditCommand};
use crate::geometry::Rect;
use crate::input::{Handled, KeyMsg, Messages, MouseMsg};

/// Общий блок состояния узла дерева: пишет и читает дерево (`ui`), контрол лишь хранит.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NodeState {
	/// Контрол под фокусом клавиатуры.
	pub focused: bool,
	/// Область последней раскладки; попадание мыши считается по ней, поэтому до первого кадра
	/// мышь контрол не находит.
	pub area: Rect,
	/// Контрол принял нажатие и получает движение и отпускание, пока кнопка не отпущена или
	/// указатель не покинул окно.
	pub captured: bool,
}

/// Контрол с состоянием: фокус, клавиши, мышь, сообщения приложения типа `M`.
///
/// Реализация у состояния приложения — одно поле `node: NodeState` и методы доступа; остальное
/// по умолчанию: не фокусируется, ввод не принимает.
pub trait Control<M> {
	fn node(&self) -> &NodeState;
	fn node_mut(&mut self) -> &mut NodeState;

	/// Может ли получить фокус по `Tab` и клику. Неактивный контрол возвращает `false`.
	fn focusable(&self) -> bool {
		false
	}

	/// Клавиша, когда контрол под фокусом. `Tab`/`Shift+Tab` сюда не доходят — их берёт дерево.
	fn key(&mut self, msg: &KeyMsg<'_>, out: &mut Messages<M>) -> Handled {
		let _ = (msg, out);
		Handled::No
	}

	/// Сообщение мыши в системе координат дерева; адресат — контрол под указателем или под
	/// захватом. Ответ `Yes` на `Down` даёт захват до `Up`/`Cancel`.
	fn mouse(&mut self, msg: &MouseMsg, out: &mut Messages<M>) -> Handled {
		let _ = (msg, out);
		Handled::No
	}

	/// Текст из `AppHandler::on_text` (IME, `Ime::Commit`), когда контрол под фокусом
	/// (ADR-0027). Символьные клавиши в `key` текстом не считаются — иначе ввод удвоится.
	fn text(&mut self, text: &str, out: &mut Messages<M>) -> Handled {
		let _ = (text, out);
		Handled::No
	}

	/// Команда правки (ADR-0030), когда контрол под фокусом: дерево распознаёт её по физической
	/// клавише и вызывает до `key`; `clipboard` — буфер приложения либо пустой, если приложение
	/// буфер не подключило. Ответ `No` — нажатие идёт обычным путём (`key`, текст, карта
	/// сочетаний).
	fn edit(
		&mut self,
		command: EditCommand,
		clipboard: &mut dyn Clipboard,
		out: &mut Messages<M>,
	) -> Handled {
		let _ = (command, clipboard, out);
		Handled::No
	}

	/// Такт времени контролу, держащему захват мыши (поправка ADR-0032): приложение зовёт
	/// `Ui::tick` из `AppHandler::on_frame`. Ответ `Yes` — контрол в состоянии, зависящем от
	/// времени (кнопка удержана на стрелке полосы прокрутки), и просит следующий такт; `No` —
	/// время ему не нужно. Таймеров у виджетов нет: время приносит только этот вызов.
	fn tick(&mut self, now: Instant, out: &mut Messages<M>) -> Handled {
		let _ = (now, out);
		Handled::No
	}

	/// Слой, в котором живёт контрол, закрывают кликом мимо (`Modal::dismiss_on_click_outside`,
	/// ADR-0027): контрол приводит состояние в порядок (выпадающий список сворачивается).
	fn dismiss(&mut self) {}
}

/// Контрол, который рисует себя сам (свой контрол приложения): вид и поведение в одном типе.
/// Стиль фокуса — по `self.node().focused`.
pub trait Drawable {
	fn draw(&mut self, area: Rect, buf: &mut ScreenBuffer);
}

/// Что произошло в состоянии встроенного контрола за одну обработку: дерево превращает это в
/// сообщение приложения по привязке из описания (`on_select`, `on_activate`, `on_change`,
/// `on_submit`, `on_toggle`), сам контрол о типе сообщений не знает.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WidgetEvent {
	/// Выделение изменилось на указанный индекс.
	Selected(usize),
	/// Контрол активирован (кнопка).
	Activated,
	/// Значение поля изменилось (ADR-0027).
	Changed(String),
	/// Значение поля подтверждено `Enter` (ADR-0027).
	Submitted(String),
	/// Флажок переключён пользователем; значение — новое (ADR-0031).
	Toggled(bool),
}

/// Состояние, сообщающее дереву о своих событиях: записывает их в обработчике, дерево забирает
/// сразу после него.
pub trait WidgetEvents {
	fn take_event(&mut self) -> Option<WidgetEvent>;
}
