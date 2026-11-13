//! Раскладка по ограничениям (ADR-0023): таблица случаев жадного алгоритма.

use gterm_widgets::{Constraint, Direction, Layout, Rect};

fn widths(constraints: &[Constraint], width: u16) -> Vec<u16> {
	Layout::horizontal(constraints.iter().copied())
		.split(Rect::new(0, 0, width, 1))
		.iter()
		.map(|r| r.width)
		.collect()
}

#[test]
fn length_and_fill() {
	assert_eq!(
		widths(&[Constraint::Length(3), Constraint::Fill(1)], 10),
		[3, 7]
	);
	assert_eq!(
		widths(&[Constraint::Fill(1), Constraint::Fill(3)], 10),
		[3, 7],
		"веса 1:3 — остаток 10 → 2,5 и 7,5 → целые доли 2 и 7, остаток 1 первому"
	);
	assert_eq!(
		widths(
			&[
				Constraint::Fill(1),
				Constraint::Fill(1),
				Constraint::Fill(1)
			],
			10
		),
		[4, 3, 3],
		"остаток деления — первым элементам"
	);
}

#[test]
fn percentage_min_max() {
	assert_eq!(
		widths(&[Constraint::Percentage(25), Constraint::Fill(1)], 20),
		[5, 15]
	);
	assert_eq!(
		widths(&[Constraint::Min(4), Constraint::Length(2)], 10),
		[8, 2],
		"без Fill свободное место получает Min"
	);
	assert_eq!(
		widths(&[Constraint::Min(4), Constraint::Fill(1)], 10),
		[4, 6],
		"с Fill — Min остаётся на минимуме"
	);
	assert_eq!(
		widths(&[Constraint::Max(3), Constraint::Fill(1)], 10),
		[3, 7],
		"Max берёт до предела раньше Fill"
	);
	assert_eq!(
		widths(&[Constraint::Max(30), Constraint::Max(30)], 10),
		[5, 5],
		"Max делят поровну"
	);
}

#[test]
fn shortage_cuts_from_the_end_and_never_overflows() {
	assert_eq!(
		widths(&[Constraint::Length(6), Constraint::Length(6)], 10),
		[6, 4]
	);
	assert_eq!(
		widths(
			&[
				Constraint::Length(6),
				Constraint::Length(6),
				Constraint::Length(6)
			],
			10
		),
		[6, 4, 0]
	);
	assert_eq!(
		widths(
			&[Constraint::Percentage(80), Constraint::Percentage(80)],
			10
		),
		[8, 2]
	);
	for width in 0..=12u16 {
		let sum: u16 = widths(
			&[
				Constraint::Length(5),
				Constraint::Min(3),
				Constraint::Fill(1),
			],
			width,
		)
		.iter()
		.sum();
		assert!(sum <= width, "ширина {width}: сумма {sum}");
	}
}

#[test]
fn spacing_margin_and_positions() {
	let rects = Layout::vertical([Constraint::Length(2), Constraint::Fill(1)])
		.margin(1)
		.spacing(1)
		.split(Rect::new(0, 0, 10, 10));
	assert_eq!(rects, [Rect::new(1, 1, 8, 2), Rect::new(1, 4, 8, 5)]);
	let rects = Layout::new(Direction::Horizontal, [Constraint::Fill(1); 2])
		.spacing(2)
		.split(Rect::new(3, 7, 12, 4));
	assert_eq!(rects, [Rect::new(3, 7, 5, 4), Rect::new(10, 7, 5, 4)]);
	assert!(Layout::vertical([]).split(Rect::new(0, 0, 5, 5)).is_empty());
	let empty = Layout::vertical([Constraint::Fill(1)]).split(Rect::default());
	assert_eq!(empty, [Rect::default()]);
}
