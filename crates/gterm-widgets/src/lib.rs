//! Виджеты для `gterm` по образцу ratatui (ADR-0023): немедленный режим — виджет один раз
//! рисует себя в `ScreenBuffer` внутри прямоугольника `Rect`; состояние (выделение, смещение)
//! живёт у приложения. Раскладка — [`layout::Layout`] по ограничениям, текст —
//! [`text::Span`] → [`text::Line`] → [`text::Text`] со стилями [`style::Style`]. Одна кодовая
//! точка — одна ячейка (модель `gterm`), графем и двухшириных символов нет.

/// Прямоугольник на сетке ячеек.
pub mod geometry;

/// Стиль ячейки: цвета (с альфой) и флаги, накладываемые на существующие.
pub mod style;

/// Стилизованный текст: фрагмент, строка, текст; выравнивание и перенос.
pub mod text;

/// Рисование в `ScreenBuffer` с обрезкой по прямоугольнику.
pub mod buffer;

/// Раскладка по ограничениям.
pub mod layout;

/// Трейты виджетов и сами виджеты.
pub mod widgets;

/// Сообщения ввода, результат обработки и очередь сообщений приложения (ADR-0026).
pub mod input;

/// Контрол — трейт на состоянии: фокус, область, захват, обработчики (ADR-0026).
pub mod control;

/// Буфер обмена и команды правки для контрола под фокусом (ADR-0030).
pub mod clipboard;

/// Карта «сочетание → сообщение» для описания экрана и модального слоя (ADR-0029).
pub mod hotkeys;

/// Описание экрана: временное дерево контейнеров и контролов, раскладка, отрисовка, фокус,
/// маршрутизация ввода (ADR-0026).
pub mod ui;

pub use buffer::BufferExt;
#[cfg(feature = "system-clipboard")]
pub use clipboard::SystemClipboard;
pub use clipboard::{Clipboard, EditCommand, MemoryClipboard};
pub use control::{Control, Drawable, NodeState, WidgetEvent, WidgetEvents};
pub use geometry::Rect;
pub use hotkeys::HotkeyMap;
pub use input::{Handled, KeyMsg, Messages, MouseKind, MouseMsg, Routed};
pub use layout::{Constraint, Direction, Layout};
pub use style::Style;
pub use text::{Alignment, Line, Span, Text};
pub use ui::{Modal, Ui};
pub use widgets::{StatefulWidget, Widget};
