//! Счётчик нажатий подряд (ADR-0035): поле `click_count` у
//! [`PointerEvent::Input`](super::PointerEvent::Input).
//!
//! Время приходит аргументом, как у тика (`tick.rs`): счётчик не читает часы и проверяется без
//! окна на заданных моментах.

use std::time::{Duration, Instant};

use winit::event::MouseButton;

/// Наибольшая пауза между нажатиями двойного клика; ровно столько — ещё двойной.
const DOUBLE_CLICK_INTERVAL: Duration = Duration::from_millis(500);

/// Последнее нажатие левой кнопки: где, когда и каким по счёту оно было.
#[derive(Clone, Copy, Debug)]
struct Press {
	cell: (i32, i32),
	at: Instant,
	count: u8,
}

#[derive(Debug, Default)]
pub(super) struct ClickCounter {
	/// `None` — последнее нажатие было не левой кнопкой или нажатий ещё не было.
	last_left: Option<Press>,
}

impl ClickCounter {
	/// Нажатие `button` в ячейке `cell` в момент `now`: его номер подряд. Двойной (2) — левая
	/// кнопка в той же ячейке не позже [`DOUBLE_CLICK_INTERVAL`] после одиночного нажатия; после
	/// двойного счёт начинается заново. Нажатие другой кнопки — всегда 1 и прерывает серию левой.
	pub(super) fn press(&mut self, button: MouseButton, cell: (i32, i32), now: Instant) -> u8 {
		if button != MouseButton::Left {
			self.last_left = None;
			return 1;
		}
		let is_double = self.last_left.is_some_and(|last| {
			last.count == 1
				&& last.cell == cell
				&& now.saturating_duration_since(last.at) <= DOUBLE_CLICK_INTERVAL
		});
		let count = if is_double { 2 } else { 1 };
		self.last_left = Some(Press {
			cell,
			at: now,
			count,
		});
		count
	}

	/// Отпускание `button`: номер нажатия, которое оно завершает (у левой — последнего левого),
	/// у других кнопок — 1.
	pub(super) fn release(&self, button: MouseButton) -> u8 {
		match (button, self.last_left) {
			(MouseButton::Left, Some(last)) => last.count,
			_ => 1,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	const CELL: (i32, i32) = (3, 4);

	fn after(start: Instant, ms: u64) -> Instant {
		start + Duration::from_millis(ms)
	}

	#[test]
	fn second_left_press_in_the_same_cell_within_the_interval_is_double() {
		let start = Instant::now();
		let mut counter = ClickCounter::default();
		assert_eq!(counter.press(MouseButton::Left, CELL, start), 1);
		assert_eq!(counter.release(MouseButton::Left), 1);
		assert_eq!(counter.press(MouseButton::Left, CELL, after(start, 200)), 2);
		assert_eq!(counter.release(MouseButton::Left), 2);
	}

	#[test]
	fn the_interval_boundary_is_inclusive() {
		let start = Instant::now();
		let mut counter = ClickCounter::default();
		counter.press(MouseButton::Left, CELL, start);
		assert_eq!(counter.press(MouseButton::Left, CELL, after(start, 500)), 2);

		let mut counter = ClickCounter::default();
		counter.press(MouseButton::Left, CELL, start);
		assert_eq!(counter.press(MouseButton::Left, CELL, after(start, 501)), 1);
		// Запоздавшее нажатие само становится первым новой пары.
		assert_eq!(counter.press(MouseButton::Left, CELL, after(start, 700)), 2);
	}

	#[test]
	fn a_press_in_another_cell_starts_over() {
		let start = Instant::now();
		let mut counter = ClickCounter::default();
		counter.press(MouseButton::Left, CELL, start);
		assert_eq!(
			counter.press(MouseButton::Left, (4, 4), after(start, 100)),
			1
		);
		assert_eq!(
			counter.press(MouseButton::Left, (4, 4), after(start, 200)),
			2
		);
	}

	#[test]
	fn another_button_is_never_double_and_breaks_the_left_series() {
		let start = Instant::now();
		let mut counter = ClickCounter::default();
		assert_eq!(counter.press(MouseButton::Right, CELL, start), 1);
		assert_eq!(
			counter.press(MouseButton::Right, CELL, after(start, 100)),
			1
		);
		assert_eq!(counter.release(MouseButton::Right), 1);

		counter.press(MouseButton::Left, CELL, after(start, 200));
		counter.press(MouseButton::Middle, CELL, after(start, 250));
		assert_eq!(counter.press(MouseButton::Left, CELL, after(start, 300)), 1);
	}

	#[test]
	fn the_press_after_a_double_starts_counting_again() {
		let start = Instant::now();
		let mut counter = ClickCounter::default();
		let counts: Vec<u8> = [0, 100, 200, 300, 400]
			.into_iter()
			.map(|ms| counter.press(MouseButton::Left, CELL, after(start, ms)))
			.collect();
		assert_eq!(counts, [1, 2, 1, 2, 1]);
	}
}
