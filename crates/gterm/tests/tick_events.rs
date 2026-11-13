//! Тик приложения (ADR-0033; ТЗ `docs/spec/2026-11-13-tick-meter.md`, К1–К5) на настоящем окне:
//! период из `AppOptions`, отсутствие тиков без периода, смена и выключение через `Request`,
//! кадр только по `ctx.redraw()`, отставание не догоняется.
//!
//! `winit` разрешает один цикл событий на процесс, а сценариев несколько, поэтому бинарник без
//! аргумента сценария запускает сам себя по разу на сценарий и проверяет код возврата дочернего
//! процесса; с аргументом `--scenario=…` исполняет один сценарий. Без дисплея сценарий печатает
//! причину и завершается успешно (это пропуск).
//!
//! Интервалы проверяются только снизу: «не короче периода» обязана давать сама схема отсчёта,
//! а верхняя граница зависит от машины и системного таймера.

#[path = "events_common/launch.rs"]
mod launch;

use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gterm::app::{AppHandler, AppOptions, Ctx};
use gterm::buffer::ScreenBuffer;
use gterm::config::Config;

const SCENARIO_ARG: &str = "--scenario=";
/// Первое слово stdout дочернего процесса: сценарий выполнен или пропущен с причиной.
const DONE: &str = "ok";
const SKIPPED: &str = "skip";

struct Scenario {
	name: &'static str,
	run: fn() -> Verdict,
}

enum Verdict {
	Done,
	Skipped(&'static str),
}

const SCENARIOS: [Scenario; 4] = [
	Scenario {
		name: "period",
		run: ticks_arrive_with_the_period_and_draw_only_on_request,
	},
	Scenario {
		name: "no-period",
		run: no_ticks_without_a_period,
	},
	Scenario {
		name: "switch",
		run: request_changes_and_disables_the_period,
	},
	Scenario {
		name: "lag",
		run: missed_ticks_are_not_replayed,
	},
];

fn main() {
	let scenario_name =
		std::env::args().find_map(|arg| arg.strip_prefix(SCENARIO_ARG).map(str::to_owned));
	match scenario_name {
		Some(name) => run_scenario(&name),
		None => SCENARIOS.iter().for_each(run_in_child_process),
	}
}

fn run_scenario(name: &str) {
	let scenario = SCENARIOS
		.iter()
		.find(|scenario| scenario.name == name)
		.unwrap_or_else(|| panic!("неизвестный сценарий {name}"));
	match (scenario.run)() {
		Verdict::Done => println!("{DONE}"),
		Verdict::Skipped(reason) => println!("{SKIPPED}: {reason}"),
	}
}

fn run_in_child_process(scenario: &Scenario) {
	let executable = std::env::current_exe().expect("путь к тест-бинарнику");
	let output = Command::new(executable)
		.arg(format!("{SCENARIO_ARG}{}", scenario.name))
		.output()
		.expect("запуск дочернего процесса");
	let stdout = String::from_utf8_lossy(&output.stdout);
	let name = scenario.name;
	eprint!("{}", String::from_utf8_lossy(&output.stderr));
	assert!(
		output.status.success(),
		"сценарий {name}: дочерний процесс завершился с {}",
		output.status
	);
	assert!(
		stdout.starts_with(DONE) || stdout.starts_with(SKIPPED),
		"сценарий {name}: stdout {stdout:?}"
	);
	println!("тик приложения, {name}: {}", stdout.trim());
}

/// Что обработчик узнал о тиках.
#[derive(Default)]
struct Log {
	/// Момент входа в каждый `on_tick`.
	tick_started: Vec<Instant>,
	/// Момент выхода из каждого `on_tick`.
	tick_finished: Vec<Instant>,
	/// `ctx.stats.frames` на входе в каждый `on_tick`.
	frames_at_tick: Vec<u64>,
}

/// Журнал, общий для обработчика (уходит в `App::run`) и проверок после него.
#[derive(Clone, Default)]
struct SharedLog(Arc<Mutex<Log>>);

impl SharedLog {
	/// Записать вход в `on_tick`; возвращает номер тика, считая с единицы.
	fn begin_tick(&self, ctx: &Ctx<'_>) -> usize {
		let mut log = self.0.lock().expect("log");
		log.tick_started.push(Instant::now());
		log.frames_at_tick.push(ctx.stats.frames);
		log.tick_started.len()
	}

	fn end_tick(&self) {
		self.0
			.lock()
			.expect("log")
			.tick_finished
			.push(Instant::now());
	}
}

/// Запустить обработчик на окне; `false` — окна нет, сценарий пропущен.
fn run(handler: impl AppHandler + 'static, options: AppOptions) -> bool {
	let Some(result) = launch::run_handler(
		handler,
		ScreenBuffer::new(24, 6),
		Config::default(),
		options,
	) else {
		return false;
	};
	if let Err(e) = result {
		panic!("application loop failed: {e}");
	}
	true
}

fn options_with_period(period: Duration) -> AppOptions {
	AppOptions {
		tick_period: Some(period),
		..AppOptions::default()
	}
}

/// Каждый вход в `on_tick` не раньше, чем через `period` после предыдущего.
fn assert_intervals_at_least(moments: &[Instant], period: Duration) {
	for (index, pair) in moments.windows(2).enumerate() {
		let interval = pair[1].duration_since(pair[0]);
		assert!(
			interval >= period,
			"тик {} пришёл через {interval:?} после предыдущего, период {period:?}",
			index + 2
		);
	}
}

// --- К1, К4: период из `AppOptions`; кадр только по `ctx.redraw()` ---

const PERIOD: Duration = Duration::from_millis(20);
/// Столько первых тиков обработчик ничего не просит — кадров от них быть не должно.
const QUIET_TICKS: usize = 10;
/// Столько следующих тиков обработчик просит кадр.
const REDRAWING_TICKS: usize = 5;
/// Кадров от запросов обязано быть не меньше стольких: запас на слияние двух запросов в кадр.
const MIN_REQUESTED_FRAMES: u64 = 3;

struct Periodic {
	log: SharedLog,
}

impl AppHandler for Periodic {
	fn on_tick(&mut self, ctx: &mut Ctx<'_>) {
		let tick = self.log.begin_tick(ctx);
		if tick > QUIET_TICKS + REDRAWING_TICKS {
			ctx.exit();
		} else if tick > QUIET_TICKS {
			ctx.redraw();
		}
		self.log.end_tick();
	}
}

fn ticks_arrive_with_the_period_and_draw_only_on_request() -> Verdict {
	let log = SharedLog::default();
	if !run(Periodic { log: log.clone() }, options_with_period(PERIOD)) {
		return Verdict::Skipped("нет цикла событий");
	}
	let log = log.0.lock().expect("log");
	assert_eq!(
		log.tick_started.len(),
		QUIET_TICKS + REDRAWING_TICKS + 1,
		"число тиков до выхода"
	);
	assert_intervals_at_least(&log.tick_started, PERIOD);

	// Вторая половина «тихих» тиков: стартовые кадры окна к этому времени уже отрисованы.
	let frames_mid_quiet = log.frames_at_tick[QUIET_TICKS / 2];
	let frames_after_quiet = log.frames_at_tick[QUIET_TICKS];
	assert_eq!(
		frames_after_quiet, frames_mid_quiet,
		"тики без ctx.redraw() вызвали кадры"
	);
	let frames_at_exit = log.frames_at_tick[QUIET_TICKS + REDRAWING_TICKS];
	assert!(
		frames_at_exit >= frames_after_quiet + MIN_REQUESTED_FRAMES,
		"{REDRAWING_TICKS} тиков с ctx.redraw() дали кадров: {}",
		frames_at_exit - frames_after_quiet
	);
	Verdict::Done
}

// --- К2: без периода тиков нет ---

/// Столько окно живёт без периода, прежде чем считать, что тиков нет.
const UNTIMED_RUN: Duration = Duration::from_millis(150);

/// Без тика цикл ведёт само приложение — кадрами из `on_frame`, как замер `--bench`.
struct Untimed {
	log: SharedLog,
	first_frame_at: Option<Instant>,
}

impl AppHandler for Untimed {
	fn on_backend_changed(&mut self, _name: &'static str, ctx: &mut Ctx<'_>) {
		ctx.redraw();
	}

	fn on_tick(&mut self, ctx: &mut Ctx<'_>) {
		self.log.begin_tick(ctx);
		self.log.end_tick();
	}

	fn on_frame(&mut self, ctx: &mut Ctx<'_>) {
		let first_frame_at = *self.first_frame_at.get_or_insert_with(Instant::now);
		if first_frame_at.elapsed() >= UNTIMED_RUN {
			ctx.exit();
		} else {
			ctx.redraw();
		}
	}
}

fn no_ticks_without_a_period() -> Verdict {
	let log = SharedLog::default();
	let handler = Untimed {
		log: log.clone(),
		first_frame_at: None,
	};
	if !run(handler, AppOptions::default()) {
		return Verdict::Skipped("нет цикла событий");
	}
	let ticks = log.0.lock().expect("log").tick_started.len();
	assert_eq!(ticks, 0, "без периода пришло тиков: {ticks}");
	Verdict::Done
}

// --- К3: смена и выключение периода через `Request` ---

const FAST_PERIOD: Duration = Duration::from_millis(10);
const SLOW_PERIOD: Duration = Duration::from_millis(40);
const FAST_TICKS: usize = 4;
const SLOW_TICKS: usize = 3;
/// Столько после выключения периода окно ещё живёт: новых тиков за это время быть не должно.
const SILENCE_AFTER_DISABLE: Duration = Duration::from_millis(200);

struct Switching {
	log: SharedLog,
	disabled_at: Option<Instant>,
}

impl AppHandler for Switching {
	fn on_backend_changed(&mut self, _name: &'static str, ctx: &mut Ctx<'_>) {
		ctx.request.tick_period = Some(Some(FAST_PERIOD));
	}

	fn on_tick(&mut self, ctx: &mut Ctx<'_>) {
		let tick = self.log.begin_tick(ctx);
		if tick == FAST_TICKS {
			ctx.request.tick_period = Some(Some(SLOW_PERIOD));
		} else if tick == FAST_TICKS + SLOW_TICKS {
			ctx.request.tick_period = Some(None);
			self.disabled_at = Some(Instant::now());
			// Дальше цикл ведут кадры: без тика его больше нечем будить.
			ctx.redraw();
		}
		self.log.end_tick();
	}

	fn on_frame(&mut self, ctx: &mut Ctx<'_>) {
		let Some(disabled_at) = self.disabled_at else {
			return;
		};
		if disabled_at.elapsed() >= SILENCE_AFTER_DISABLE {
			ctx.exit();
		} else {
			ctx.redraw();
		}
	}
}

fn request_changes_and_disables_the_period() -> Verdict {
	let log = SharedLog::default();
	let handler = Switching {
		log: log.clone(),
		disabled_at: None,
	};
	if !run(handler, AppOptions::default()) {
		return Verdict::Skipped("нет цикла событий");
	}
	let log = log.0.lock().expect("log");
	assert_eq!(
		log.tick_started.len(),
		FAST_TICKS + SLOW_TICKS,
		"после выключения периода тики продолжились"
	);
	assert_intervals_at_least(&log.tick_started[..FAST_TICKS], FAST_PERIOD);
	// Интервалы после тика, сменившего период, включая первый из них.
	assert_intervals_at_least(&log.tick_started[FAST_TICKS - 1..], SLOW_PERIOD);
	Verdict::Done
}

// --- К5: отставание не догоняется ---

const LAG_PERIOD: Duration = Duration::from_millis(10);
/// Задержка обработчика — десять периодов: догоняющая схема выдала бы пачку тиков подряд.
const STALL: Duration = Duration::from_millis(100);
const STALLED_TICK: usize = 3;
const LAST_TICK: usize = 6;

struct Stalling {
	log: SharedLog,
}

impl AppHandler for Stalling {
	fn on_tick(&mut self, ctx: &mut Ctx<'_>) {
		let tick = self.log.begin_tick(ctx);
		if tick == STALLED_TICK {
			// Медленный обработчик — предмет проверки, а не синхронизация теста.
			std::thread::sleep(STALL);
		}
		if tick == LAST_TICK {
			ctx.exit();
		}
		self.log.end_tick();
	}
}

fn missed_ticks_are_not_replayed() -> Verdict {
	let log = SharedLog::default();
	if !run(
		Stalling { log: log.clone() },
		options_with_period(LAG_PERIOD),
	) {
		return Verdict::Skipped("нет цикла событий");
	}
	let log = log.0.lock().expect("log");
	assert_eq!(log.tick_started.len(), LAST_TICK, "число тиков до выхода");
	for (index, (finished, next_started)) in log
		.tick_finished
		.iter()
		.zip(log.tick_started.iter().skip(1))
		.enumerate()
	{
		let pause = next_started.duration_since(*finished);
		assert!(
			pause >= LAG_PERIOD,
			"тик {} пришёл через {pause:?} после возврата из предыдущего, период {LAG_PERIOD:?}",
			index + 2
		);
	}
	Verdict::Done
}
