//! Раскладка по ограничениям (ADR-0023): жадный целочисленный алгоритм без солвера.
//!
//! Порядок: `Length`, `Percentage` и `Min` задают стартовые длины (нехватка места урезается с
//! конца); свободный остаток получают сначала `Max` — до своих пределов поровну, затем `Fill` —
//! по весам, а если `Fill` нет — `Min` поровну. Остаток целочисленного деления достаётся первым
//! элементам. Результат детерминирован, сумма длин не больше доступной.

use crate::geometry::Rect;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Direction {
	Horizontal,
	#[default]
	Vertical,
}

/// Ограничение на длину одного участка вдоль направления раскладки.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Constraint {
	/// Ровно столько ячеек (при нехватке — меньше).
	Length(u16),
	/// Доля от всей длины области, в процентах.
	Percentage(u16),
	/// Не меньше; растягивается, если свободное место не разбирают `Fill`.
	Min(u16),
	/// Не больше; забирает свободное место до предела раньше `Fill`.
	Max(u16),
	/// Свободное место по весу (0 — как 1).
	Fill(u16),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Layout {
	direction: Direction,
	constraints: Vec<Constraint>,
	margin: u16,
	spacing: u16,
}

impl Layout {
	pub fn new<I: IntoIterator<Item = Constraint>>(direction: Direction, constraints: I) -> Self {
		Self {
			direction,
			constraints: constraints.into_iter().collect(),
			margin: 0,
			spacing: 0,
		}
	}

	pub fn vertical<I: IntoIterator<Item = Constraint>>(constraints: I) -> Self {
		Self::new(Direction::Vertical, constraints)
	}

	pub fn horizontal<I: IntoIterator<Item = Constraint>>(constraints: I) -> Self {
		Self::new(Direction::Horizontal, constraints)
	}

	pub fn margin(mut self, margin: u16) -> Self {
		self.margin = margin;
		self
	}

	pub fn spacing(mut self, spacing: u16) -> Self {
		self.spacing = spacing;
		self
	}

	/// Разбить область на участки — по одному на ограничение, в порядке ограничений.
	pub fn split(&self, area: Rect) -> Vec<Rect> {
		let count = self.constraints.len();
		if count == 0 {
			return Vec::new();
		}
		let inner = area.inner(crate::geometry::Margin {
			horizontal: self.margin,
			vertical: self.margin,
		});
		let total = match self.direction {
			Direction::Horizontal => inner.width,
			Direction::Vertical => inner.height,
		};
		let gaps = self.spacing.saturating_mul((count - 1) as u16);
		let available = usize::from(total.saturating_sub(gaps));
		let sizes = solve(&self.constraints, usize::from(total), available);
		let mut rects = Vec::with_capacity(count);
		let mut offset = 0usize;
		for size in sizes {
			let size = size.min(u16::MAX as usize) as u16;
			let rect = match self.direction {
				Direction::Horizontal => Rect::new(
					inner.x.saturating_add(offset as u16),
					inner.y,
					size,
					inner.height,
				),
				Direction::Vertical => Rect::new(
					inner.x,
					inner.y.saturating_add(offset as u16),
					inner.width,
					size,
				),
			};
			rects.push(rect);
			offset += usize::from(size) + usize::from(self.spacing);
		}
		rects
	}
}

/// Длины участков по ограничениям: `total` — вся длина области (для процентов), `available` —
/// длина без зазоров.
fn solve(constraints: &[Constraint], total: usize, available: usize) -> Vec<usize> {
	let mut sizes: Vec<usize> = constraints
		.iter()
		.map(|c| match *c {
			Constraint::Length(n) => usize::from(n),
			Constraint::Percentage(p) => total * usize::from(p.min(100)) / 100,
			Constraint::Min(n) => usize::from(n),
			Constraint::Max(_) | Constraint::Fill(_) => 0,
		})
		.collect();
	// Нехватка места: урезаем с конца.
	let mut used: usize = sizes.iter().sum();
	for size in sizes.iter_mut().rev() {
		if used <= available {
			break;
		}
		let cut = (used - available).min(*size);
		*size -= cut;
		used -= cut;
	}
	let mut free = available.saturating_sub(used);
	// `Max` — до предела, поровну.
	let max_items: Vec<usize> = constraints
		.iter()
		.enumerate()
		.filter_map(|(i, c)| matches!(c, Constraint::Max(_)).then_some(i))
		.collect();
	if !max_items.is_empty() && free > 0 {
		let share = free / max_items.len();
		let mut extra = free % max_items.len();
		for &i in &max_items {
			let Constraint::Max(limit) = constraints[i] else {
				continue;
			};
			let want = share + usize::from(extra > 0);
			extra = extra.saturating_sub(1);
			let give = want.min(usize::from(limit)).min(free);
			sizes[i] += give;
			free -= give;
		}
	}
	// `Fill` по весам; без `Fill` — `Min` поровну.
	let fills: Vec<(usize, usize)> = constraints
		.iter()
		.enumerate()
		.filter_map(|(i, c)| match *c {
			Constraint::Fill(w) => Some((i, usize::from(w.max(1)))),
			_ => None,
		})
		.collect();
	let flexible: Vec<(usize, usize)> = if fills.is_empty() {
		constraints
			.iter()
			.enumerate()
			.filter_map(|(i, c)| matches!(c, Constraint::Min(_)).then_some((i, 1)))
			.collect()
	} else {
		fills
	};
	let weight_sum: usize = flexible.iter().map(|(_, w)| *w).sum();
	if weight_sum > 0 && free > 0 {
		// Целые доли по весам; остаток деления — первым элементам по одному.
		let shares: Vec<usize> = flexible
			.iter()
			.map(|(_, w)| free * w / weight_sum)
			.collect();
		let mut remainder = free - shares.iter().sum::<usize>();
		for (&(i, _), share) in flexible.iter().zip(shares) {
			let bonus = usize::from(remainder > 0);
			remainder -= bonus;
			sizes[i] += share + bonus;
		}
	}
	sizes
}
