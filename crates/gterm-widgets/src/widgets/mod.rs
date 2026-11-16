//! Трейты виджетов (форма — как у ratatui, ADR-0023) и сами виджеты части 1: [`Block`],
//! [`Paragraph`], [`List`], [`Table`], [`Popup`]; контролы (ADR-0026): [`Button`], состояния списка и таблицы; флажок [`Checkbox`] (ADR-0031);
//! часть 2: [`Clear`], [`Gauge`], [`LineGauge`], [`Scrollbar`], контрол [`Tabs`] (ADR-0032);
//! индикатор уровня [`Meter`] (ADR-0034); контрол-ползунок [`Slider`] (ADR-0036).

use gterm::buffer::ScreenBuffer;

use crate::geometry::Rect;

mod block;
mod button;
mod checkbox;
mod clear;
mod combo_box;
mod gauge;
mod list;
mod meter;
mod paragraph;
mod popup;
mod scrollbar;
mod selection;
mod slider;
mod table;
mod tabs;
mod text_input;

pub use block::{Block, BorderType, Borders, Padding};
pub use button::{Button, ButtonState};
pub use checkbox::{Checkbox, CheckboxState};
pub use clear::Clear;
pub use combo_box::{ComboBox, ComboBoxState};
pub use gauge::{Gauge, LineGauge};
pub use list::{List, ListItem, ListState};
pub use meter::Meter;
pub use paragraph::{Paragraph, Wrap};
pub use popup::Popup;
pub use scrollbar::{Scrollbar, ScrollbarOrientation, ScrollbarState};
pub use slider::{Slider, SliderState};
pub use table::{Row, Table, TableState};
pub use tabs::{Tabs, TabsState};
pub use text_input::{TextInput, TextInputState};

/// Виджет без состояния: потребляется отрисовкой.
pub trait Widget {
	fn render(self, area: Rect, buf: &mut ScreenBuffer);
}

/// Виджет с состоянием, которое хранит приложение (выделение, смещение прокрутки).
pub trait StatefulWidget {
	type State;
	fn render(self, area: Rect, buf: &mut ScreenBuffer, state: &mut Self::State);
}
