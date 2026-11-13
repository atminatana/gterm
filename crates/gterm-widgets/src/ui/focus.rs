//! Обход фокуса (ADR-0026): кандидаты — фокусируемые контролы в порядке объявления в глубину,
//! контейнеры прозрачны; `Tab` по кольцу, `Shift+Tab` строго обратно. Флаг лежит в состоянии
//! контрола, поэтому перед любым использованием дерево приводит флаги в порядок: ровно один
//! кандидат под фокусом либо ни одного (кандидатов нет).

use super::Ui;

impl<'a, M: 'a> Ui<'a, M> {
	/// Индексы фокусируемых контролов в порядке обхода.
	fn candidates(&self) -> Vec<usize> {
		self.elements()
			.filter(|(_, element)| element.focusable())
			.map(|(index, _)| index)
			.collect()
	}

	/// Контрол под фокусом: первый кандидат с поднятым флагом.
	pub(super) fn focused(&self) -> Option<usize> {
		self.elements()
			.find(|(_, element)| {
				element.focusable() && element.node().is_some_and(|node| node.focused)
			})
			.map(|(index, _)| index)
	}

	/// Снять флаг со всех контролов, кроме `keep`: устаревшие флаги (контрол стал
	/// нефокусируемым, два флага после перестройки экрана) исчезают.
	fn set_focus(&mut self, keep: Option<usize>) {
		for index in 0..self.nodes.len() {
			let Some(state) = self.element_state_mut(index) else {
				continue;
			};
			state.focused = Some(index) == keep;
		}
	}

	/// Привести флаги в порядок и, если фокуса нет, а кандидаты есть, отдать его первому: так
	/// ведёт себя старт приложения и исчезновение контрола под фокусом из описания.
	pub(super) fn ensure_focus(&mut self) {
		let current = self.focused();
		let target = current.or_else(|| self.candidates().first().copied());
		self.set_focus(target);
	}

	/// Фокус первому кандидату; без кандидатов фокуса нет.
	pub fn focus_first(&mut self) -> &mut Self {
		let first = self.candidates().first().copied();
		self.set_focus(first);
		self
	}

	/// Следующий кандидат по кольцу; без фокуса — первый.
	pub fn focus_next(&mut self) -> &mut Self {
		self.step(1)
	}

	/// Предыдущий кандидат по кольцу (строго обратный порядок); без фокуса — последний.
	pub fn focus_prev(&mut self) -> &mut Self {
		self.step(-1)
	}

	fn step(&mut self, direction: isize) -> &mut Self {
		let candidates = self.candidates();
		if candidates.is_empty() {
			self.set_focus(None);
			return self;
		}
		let count = candidates.len() as isize;
		let position = self
			.focused()
			.and_then(|focused| candidates.iter().position(|&c| c == focused));
		let next = match position {
			Some(p) => (p as isize + direction).rem_euclid(count),
			None if direction > 0 => 0,
			None => count - 1,
		};
		self.set_focus(Some(candidates[next as usize]));
		self
	}

	/// Область контрола под фокусом после раскладки.
	pub fn focused_area(&self) -> Option<super::Rect> {
		self.focused()
			.and_then(|index| self.areas.get(index).copied())
	}

	pub(super) fn element_state_mut(
		&mut self,
		index: usize,
	) -> Option<&mut crate::control::NodeState> {
		match &mut self.nodes[index].kind {
			super::Kind::Element(element) => element.node_mut(),
			super::Kind::Container { .. } => None,
		}
	}
}
