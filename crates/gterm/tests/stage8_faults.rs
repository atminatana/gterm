//! Этап 8 (ТЗ §5.3 п.5, FR-20, FR-21; ADR-0007): ошибки кадра, инжектированные флагами `Config`,
//! проходят настоящий путь `App::redraw` — повтор кадра, откат на Software, остановка.
//!
//! `winit` разрешает один цикл событий на процесс, а сценариев несколько, поэтому бинарник без
//! аргумента сценария запускает сам себя по разу на сценарий и проверяет код возврата и stderr
//! дочернего процесса; с аргументом `--scenario=…` исполняет один сценарий на настоящем окне.
//! Без дисплея сценарий печатает причину и завершается успешно (это пропуск).

mod events_common;

use std::process::Command;

use events_common::{
	Finished, Log, WANTED, assert_frame_stats, assert_kept_drawing, assert_known_backends,
	run_once, run_to_end,
};
use gterm::app::{AppError, GridMode};
use gterm::config::{BackendKind, Config};

const SCENARIO_ARG: &str = "--scenario=";
/// Первое слово stdout дочернего процесса: сценарий выполнен или пропущен с причиной.
const DONE: &str = "ok";
const SKIPPED: &str = "skip";

/// Сообщения библиотеки в stderr (FR-20), по которым виден пройденный путь.
const RETRY_MESSAGE: &str = "retrying once";
const FALLBACK_MESSAGE: &str = "switching to Software";
const INJECTED_MESSAGE: &str = "injected by --fail-frames";
const CONTEXT_LOST_MESSAGE: &str = "context lost";

/// Сценарий: что инжектируется, что обязано и чего не должно быть в stderr.
struct Scenario {
	name: &'static str,
	config: fn() -> Config,
	check: fn(Config) -> Verdict,
	expected: &'static [&'static str],
	forbidden: &'static [&'static str],
	/// Сколько раз кадр повторяется: по разу на каждый бэкенд, получивший ошибку.
	retries: usize,
}

enum Verdict {
	Done,
	Skipped(&'static str),
}

const SCENARIOS: [Scenario; 5] = [
	Scenario {
		name: "retry",
		config: || with_failed_frames(BackendKind::Auto, 1),
		check: frame_is_retried_and_backend_stays,
		expected: &[RETRY_MESSAGE, INJECTED_MESSAGE],
		forbidden: &[FALLBACK_MESSAGE],
		retries: 1,
	},
	Scenario {
		name: "fallback",
		config: || with_failed_frames(BackendKind::Auto, 2),
		check: falls_back_to_software_and_keeps_running,
		expected: &[RETRY_MESSAGE, INJECTED_MESSAGE, FALLBACK_MESSAGE],
		forbidden: &[],
		retries: 1,
	},
	Scenario {
		name: "context-lost",
		config: || Config {
			lose_context: true,
			..Config::default()
		},
		check: falls_back_to_software_and_keeps_running,
		expected: &[CONTEXT_LOST_MESSAGE, FALLBACK_MESSAGE],
		forbidden: &[RETRY_MESSAGE],
		retries: 0,
	},
	Scenario {
		name: "software-stops",
		config: || with_failed_frames(BackendKind::Soft, 2),
		check: software_failure_stops_the_loop,
		expected: &[RETRY_MESSAGE, INJECTED_MESSAGE],
		forbidden: &[FALLBACK_MESSAGE],
		retries: 1,
	},
	Scenario {
		name: "chain-end",
		config: || with_failed_frames(BackendKind::Auto, 4),
		check: fallback_then_software_failure_stops_the_loop,
		expected: &[RETRY_MESSAGE, INJECTED_MESSAGE, FALLBACK_MESSAGE],
		forbidden: &[],
		retries: 2,
	},
];

fn with_failed_frames(backend: BackendKind, fail_frames: u32) -> Config {
	Config {
		backend,
		fail_frames,
		..Config::default()
	}
}

fn main() {
	let scenario_name =
		std::env::args().find_map(|arg| arg.strip_prefix(SCENARIO_ARG).map(str::to_owned));
	match scenario_name {
		Some(name) => run_scenario(&name),
		None => SCENARIOS.iter().for_each(run_in_child_process),
	}
}

/// Дочерний процесс: один сценарий на настоящем окне.
fn run_scenario(name: &str) {
	let scenario = SCENARIOS
		.iter()
		.find(|scenario| scenario.name == name)
		.unwrap_or_else(|| panic!("неизвестный сценарий {name}"));
	match (scenario.check)((scenario.config)()) {
		Verdict::Done => println!("{DONE}"),
		Verdict::Skipped(reason) => println!("{SKIPPED}: {reason}"),
	}
}

/// Родительский процесс: запустить сценарий отдельным процессом и сверить stderr.
fn run_in_child_process(scenario: &Scenario) {
	let executable = std::env::current_exe().expect("путь к тест-бинарнику");
	let output = Command::new(executable)
		.arg(format!("{SCENARIO_ARG}{}", scenario.name))
		.output()
		.expect("запуск дочернего процесса");
	let stdout = String::from_utf8_lossy(&output.stdout);
	let stderr = String::from_utf8_lossy(&output.stderr);
	let name = scenario.name;
	eprint!("{stderr}");
	assert!(
		output.status.success(),
		"сценарий {name}: дочерний процесс завершился с {}",
		output.status
	);
	if !stdout.starts_with(DONE) {
		assert!(
			stdout.starts_with(SKIPPED),
			"сценарий {name}: stdout {stdout:?}"
		);
		println!("инжекция ошибок кадра, {name}: {}", stdout.trim());
		return;
	}
	for message in scenario.expected {
		assert!(
			stderr.contains(message),
			"сценарий {name}: в stderr нет {message:?}"
		);
	}
	for message in scenario.forbidden {
		assert!(
			!stderr.contains(message),
			"сценарий {name}: в stderr есть {message:?}"
		);
	}
	assert_eq!(
		stderr.matches(RETRY_MESSAGE).count(),
		scenario.retries,
		"сценарий {name}: число повторов кадра"
	);
	println!("инжекция ошибок кадра, {name}: ok");
}

/// Одна ошибка кадра: кадр повторяется, бэкенд остаётся прежним, приложение работает дальше.
fn frame_is_retried_and_backend_stays(config: Config) -> Verdict {
	let Some(log) = run_once(config, GridMode::FixedGrid, None) else {
		return Verdict::Skipped("нет цикла событий");
	};
	let log = log.lock().expect("log");
	let backends = assert_known_backends(&log);
	assert_eq!(
		backends.len(),
		1,
		"бэкенд сменился после одного повтора: {backends:?}"
	);
	assert_survived(&log);
	Verdict::Done
}

/// Ошибка и после повтора либо потеря контекста: GL → Software на том же окне, без перезапуска.
fn falls_back_to_software_and_keeps_running(config: Config) -> Verdict {
	let Some(Finished { log, result }) = run_to_end(config, GridMode::FixedGrid, None) else {
		return Verdict::Skipped("нет цикла событий");
	};
	let log = log.lock().expect("log");
	if started_on_software(&log) {
		return Verdict::Skipped("GL недоступен, откатываться не с чего");
	}
	if let Err(e) = result {
		panic!("цикл остановлен вместо отката: {e}");
	}
	let backends = assert_known_backends(&log);
	assert_eq!(backends.len(), 2, "ожидался один откат: {backends:?}");
	assert_eq!(backends[1], "Software");
	assert_survived(&log);
	Verdict::Done
}

/// Software получает ошибку и после повтора: откатываться некуда, `App::run` возвращает ошибку.
fn software_failure_stops_the_loop(config: Config) -> Verdict {
	let Some(Finished { log, result }) = run_to_end(config, GridMode::FixedGrid, None) else {
		return Verdict::Skipped("нет цикла событий");
	};
	let log = log.lock().expect("log");
	assert_eq!(assert_known_backends(&log), &["Software"]);
	assert_stopped_without_frames(&log, result);
	Verdict::Done
}

/// Ошибки переживают откат: GL → Software, затем Software тоже отказывает — остановка.
fn fallback_then_software_failure_stops_the_loop(config: Config) -> Verdict {
	let Some(Finished { log, result }) = run_to_end(config, GridMode::FixedGrid, None) else {
		return Verdict::Skipped("нет цикла событий");
	};
	let log = log.lock().expect("log");
	if started_on_software(&log) {
		return Verdict::Skipped("GL недоступен, откатываться не с чего");
	}
	let backends = assert_known_backends(&log);
	assert_eq!(backends.len(), 2, "ожидался один откат: {backends:?}");
	assert_eq!(backends[1], "Software");
	assert_stopped_without_frames(&log, result);
	Verdict::Done
}

fn started_on_software(log: &Log) -> bool {
	log.backends.first() == Some(&"Software")
}

/// Приложение пережило ошибки кадра: сетка та же, кадры идут и считаются.
fn assert_survived(log: &Log) {
	assert_eq!(log.last_grid, Some(WANTED), "сетка потеряна");
	assert_kept_drawing(log);
	assert_frame_stats(log);
}

/// Цикл остановлен ошибкой рендера, и ни один ошибочный кадр не попал в статистику.
fn assert_stopped_without_frames(log: &Log, result: Result<(), AppError>) {
	match result {
		Err(AppError::Render(_)) => {}
		Err(e) => panic!("ожидалась ошибка рендера, получено: {e}"),
		Ok(()) => panic!("ожидалась ошибка рендера, цикл завершился штатно"),
	}
	assert_eq!(log.frames, 0, "ошибочный кадр попал в статистику");
	assert_eq!(log.frame_events, 0, "on_frame пришёл за ошибочный кадр");
}
