//! Перевод событий `gterm` в сообщения ввода контролов (ADR-0026, ТЗ К3/К4): ячейка из события,
//! колесо в строках, уход указателя — отмена, модификаторы из `Ctx`.

use gterm::app::{Ctx, FrameStats, PointerEvent, Request};
use gterm::buffer::ScreenBuffer;
use gterm_widgets::{MouseKind, MouseMsg};
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton, MouseScrollDelta};
use winit::keyboard::ModifiersState;

/// Размер ячейки, с которым переводится попиксельная прокрутка.
const CELL_PX: (u32, u32) = (10, 20);

fn with_ctx<T>(modifiers: ModifiersState, f: impl FnOnce(&Ctx<'_>) -> T) -> T {
	let mut screen = ScreenBuffer::new(4, 2);
	let mut request = Request::default();
	let ctx = Ctx {
		screen: &mut screen,
		request: &mut request,
		stats: FrameStats::default(),
		modifiers,
		cell_px: CELL_PX,
	};
	f(&ctx)
}

#[test]
fn moved_is_move_at_the_event_cell_without_button() {
	let event = PointerEvent::Moved {
		position: PhysicalPosition::new(37.0, 55.0),
		delta: PhysicalPosition::new(1.0, 1.0),
		cell: (3, 2),
	};
	let msg = with_ctx(ModifiersState::empty(), |ctx| {
		MouseMsg::from_pointer(&event, ctx)
	});
	assert_eq!(msg.kind, MouseKind::Move);
	assert_eq!(msg.button, None);
	assert_eq!((msg.col, msg.row), (3, 2));
}

#[test]
fn input_maps_pressed_and_released_with_button_and_modifiers() {
	let down = PointerEvent::Input {
		state: ElementState::Pressed,
		button: MouseButton::Left,
		cell: (1, 0),
		click_count: 1,
	};
	let up = PointerEvent::Input {
		state: ElementState::Released,
		button: MouseButton::Right,
		cell: (-1, 7),
		click_count: 1,
	};
	let (down, up) = with_ctx(ModifiersState::SHIFT, |ctx| {
		(
			MouseMsg::from_pointer(&down, ctx),
			MouseMsg::from_pointer(&up, ctx),
		)
	});
	assert_eq!(down.kind, MouseKind::Down);
	assert_eq!(down.button, Some(MouseButton::Left));
	assert!(down.is_left_down());
	assert!(down.modifiers.shift_key());
	assert_eq!(up.kind, MouseKind::Up);
	assert_eq!(up.button, Some(MouseButton::Right));
	assert!(!up.is_left_down());
	// Ячейка вне сетки передаётся как есть — решает дерево.
	assert_eq!((up.col, up.row), (-1, 7));
}

#[test]
fn click_count_passes_through_and_marks_a_left_double_click() {
	let input = |state, button, click_count| PointerEvent::Input {
		state,
		button,
		cell: (2, 1),
		click_count,
	};
	let moved = PointerEvent::Moved {
		position: PhysicalPosition::new(0.0, 0.0),
		delta: PhysicalPosition::new(0.0, 0.0),
		cell: (2, 1),
	};
	let msgs = with_ctx(ModifiersState::empty(), |ctx| {
		[
			input(ElementState::Pressed, MouseButton::Left, 1),
			input(ElementState::Pressed, MouseButton::Left, 2),
			input(ElementState::Released, MouseButton::Left, 2),
			input(ElementState::Pressed, MouseButton::Right, 2),
			moved,
			PointerEvent::Left,
		]
		.map(|event| MouseMsg::from_pointer(&event, ctx))
	});
	let counts = msgs.map(|msg| (msg.click_count, msg.is_double_click()));
	assert_eq!(
		counts,
		[
			(1, false),
			(2, true),
			// Двойной клик — только нажатие левой кнопки.
			(2, false),
			(2, false),
			// У движения и отмены число нажатий — 0.
			(0, false),
			(0, false),
		]
	);
}

#[test]
fn wheel_lines_pass_through_and_pixels_divide_by_cell_height() {
	let lines = MouseScrollDelta::LineDelta(0.0, -2.0);
	let pixels = MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, 50.0));
	let by_lines = PointerEvent::Wheel {
		delta: &lines,
		cell: (0, 0),
	};
	let by_pixels = PointerEvent::Wheel {
		delta: &pixels,
		cell: (0, 0),
	};
	let (a, b) = with_ctx(ModifiersState::empty(), |ctx| {
		(
			MouseMsg::from_pointer(&by_lines, ctx),
			MouseMsg::from_pointer(&by_pixels, ctx),
		)
	});
	assert_eq!(a.kind, MouseKind::Wheel { lines: -2.0 });
	// 50 px при ячейке 20 px высотой — 2,5 строки, знак как у `winit`.
	assert_eq!(b.kind, MouseKind::Wheel { lines: 2.5 });
	assert_eq!(a.button, None);
}

#[test]
fn left_is_cancel_outside_the_grid() {
	let msg = with_ctx(ModifiersState::empty(), |ctx| {
		MouseMsg::from_pointer(&PointerEvent::Left, ctx)
	});
	assert_eq!(msg.kind, MouseKind::Cancel);
	assert_eq!(msg.button, None);
	assert_eq!((msg.col, msg.row), (-1, -1));
}

#[test]
fn translated_shifts_into_a_local_frame() {
	let msg = MouseMsg {
		kind: MouseKind::Down,
		button: Some(MouseButton::Left),
		col: 12,
		row: 5,
		modifiers: ModifiersState::empty(),
		click_count: 1,
	};
	let local = msg.translated(10, 6);
	assert_eq!((local.col, local.row), (2, -1));
	assert_eq!(local.kind, MouseKind::Down);
}
