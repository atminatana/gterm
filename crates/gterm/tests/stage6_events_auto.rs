//! Этап 6 (ТЗ FR-23, FR-25; ADR-0004): события приложения в режиме [`GridMode::AutoGrid`].
//!
//! Отдельный бинарник, потому что `winit` разрешает цикл событий только один на процесс, а
//! режимы сетки требуют двух запусков. См. `stage6_events` (FixedGrid) и `events_common`.

mod events_common;

use events_common::{
	WANTED, assert_frame_stats, assert_kept_drawing, assert_known_backends, run_once,
};
use gterm::app::GridMode;
use gterm::config::Config;

fn main() {
	if auto_grid_derives_the_grid_from_the_window() {
		println!("AutoGrid: сетка выведена из окна, запрос проигнорирован: ok");
	}
}

/// `AutoGrid`: сетка выводится из окна, а запрос приложения не действует (FR-23,
/// [`Request::grid`](gterm::app::Request::grid)). Проверяем, что библиотека не применяет то,
/// что тут же перезаписала бы перерисовка по размеру окна.
fn auto_grid_derives_the_grid_from_the_window() -> bool {
	let Some(log) = run_once(Config::default(), GridMode::AutoGrid, None) else {
		return false;
	};
	let log = log.lock().expect("log");
	assert_eq!(
		assert_known_backends(&log).len(),
		1,
		"уведомлений о бэкенде: {:?}",
		log.backends
	);
	let backend = log.backends[0];

	let grid = log.last_grid.expect("сетка не сообщена");
	assert_ne!(
		grid, WANTED,
		"запрос сетки не должен действовать в AutoGrid"
	);
	// Сетка осталась той, что выведена из окна под исходный экран приложения.
	assert!(
		grid.cols > 0 && grid.rows > 0,
		"сетка из окна вырождена: {grid:?}"
	);
	assert_kept_drawing(&log);
	assert_frame_stats(&log);
	eprintln!("AutoGrid: сетка из окна {grid:?}, бэкенд {backend}");
	true
}
