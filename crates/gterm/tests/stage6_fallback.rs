//! Этап 6 (ТЗ FR-21, §5.3 п.5; ADR-0004): откат на Software на лету по запросу приложения.
//!
//! Отдельный бинарник: `winit` разрешает цикл событий только один на процесс, а здесь нужен
//! запуск с собственным сценарием. См. `stage6_events`, `stage6_events_auto`.

mod events_common;

use events_common::{
	WANTED, assert_frame_stats, assert_kept_drawing, assert_known_backends, run_once,
};
use gterm::app::GridMode;
use gterm::config::Config;

/// На этом кадре приложение просит Software вместо GL (FR-21).
const SWITCH_AT_FRAME: u64 = 2;

fn main() {
	if fallback_to_software_at_runtime() {
		println!("откат на Software на лету без перезапуска: ok");
	}
}

/// Смена бэкенда по запросу приложения: GL → Software на том же окне, без перезапуска (FR-21,
/// §5.3 п.5). Уведомление приходит повторно — значит, цепочка уровней отработала на лету.
fn fallback_to_software_at_runtime() -> bool {
	let Some(log) = run_once(
		Config::default(),
		GridMode::FixedGrid,
		Some(SWITCH_AT_FRAME),
	) else {
		return false;
	};
	let log = log.lock().expect("log");
	let backends = assert_known_backends(&log);
	assert_eq!(
		backends,
		&["OpenGL 3.3", "Software"],
		"ожидалась смена GL → Software, получено {backends:?}"
	);
	// Сетка приложения пережила смену бэкенда: буфер тот же, содержимое то же.
	assert_eq!(
		log.last_grid,
		Some(WANTED),
		"сетка потеряна при смене бэкенда"
	);
	// Кадры шли и после смены бэкенда: цикл не завис.
	assert_kept_drawing(&log);
	// Статистика кадров пережила откат: `on_frame` считает кадры обоих бэкендов.
	assert_frame_stats(&log);
	eprintln!("откат на лету: {backends:?}, {} кадров", log.frames);
	true
}
