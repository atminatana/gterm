//! Периодический тик приложения (ADR-0033): период и срок следующего вызова
//! [`AppHandler::on_tick`](super::AppHandler::on_tick).
//!
//! Время приходит аргументом: отсчёт ведётся от момента, который назвал вызывающий, поэтому
//! пропущенные тики не догоняются — следующий срок всегда «сейчас плюс период».

use std::time::{Duration, Instant};

/// Наименьший период: более короткий приводится к нему, чтобы цикл не крутился вхолостую.
const MIN_PERIOD: Duration = Duration::from_millis(1);

#[derive(Debug)]
pub(super) struct Ticker {
	period: Option<Duration>,
	/// Срок следующего тика; `None`, пока период не задан или отсчёт ещё не начат.
	deadline: Option<Instant>,
}

impl Ticker {
	/// Отсчёт не начинается здесь: до создания окна тиков нет, его запускает [`Ticker::start`].
	pub(super) fn new(period: Option<Duration>) -> Self {
		Self {
			period: normalized(period),
			deadline: None,
		}
	}

	/// Сменить или выключить период; новый отсчёт идёт от `now`.
	pub(super) fn set_period(&mut self, period: Option<Duration>, now: Instant) {
		self.period = normalized(period);
		self.deadline = None;
		self.start(now);
	}

	/// Начать отсчёт от `now`, если период задан, а срока ещё нет.
	pub(super) fn start(&mut self, now: Instant) {
		if self.deadline.is_none() {
			self.deadline = self.period.map(|period| now + period);
		}
	}

	/// Назначить следующий тик через период от `now`.
	pub(super) fn restart(&mut self, now: Instant) {
		self.deadline = self.period.map(|period| now + period);
	}

	pub(super) fn is_due(&self, now: Instant) -> bool {
		self.deadline.is_some_and(|deadline| now >= deadline)
	}

	pub(super) fn deadline(&self) -> Option<Instant> {
		self.deadline
	}
}

fn normalized(period: Option<Duration>) -> Option<Duration> {
	period.map(|period| period.max(MIN_PERIOD))
}
