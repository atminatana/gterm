//! Запуск окна с произвольным обработчиком для оконных тест-бинарников: шрифт репозитория и
//! сторож от зависания.
//!
//! Отдельный файл, чтобы бинарник со своим обработчиком подключал только его
//! (`#[path = "events_common/launch.rs"] mod launch;`): остальное из `events_common` осталось бы
//! в нём неиспользуемым.

use std::path::{Path, PathBuf};
use std::time::Duration;

use gterm::app::{App, AppError, AppHandler, AppOptions};
use gterm::buffer::ScreenBuffer;
use gterm::config::Config;
use gterm::font::FontFamily;
use gterm::rasterizer::Rasterizer;

/// Имя семейства шрифта репозитория — как `common::REPO_FONT_FAMILY`; оконные бинарники
/// модуль `common` не подключают.
const REPO_FONT_FAMILY: &str = "RobotoMono Nerd Font Mono";
/// Сторож: столько без выхода — процесс завершается с ошибкой, чтобы тест упал, а не завис.
/// Кадры с ошибками и откат бэкенда бывают медленными, поэтому запас большой.
const WATCHDOG: Duration = Duration::from_secs(60);

/// Один запуск `handler` на настоящем окне: чем закончился `App::run`.
/// `None` — окно недоступно, это пропуск.
///
/// Шрифт берётся из репозитория: сцена не про его поиск, а `assets/` в тестах есть всегда.
pub fn run_handler(
	handler: impl AppHandler + 'static,
	screen: ScreenBuffer,
	config: Config,
	options: AppOptions,
) -> Option<Result<(), AppError>> {
	start_watchdog();
	let rasterizer = Rasterizer::new(
		FontFamily::load(&repo_fonts_dir(), REPO_FONT_FAMILY).expect("шрифт из репозитория"),
	)
	.expect("растеризатор");
	let app = App::new("gterm test", screen, rasterizer, config, options);
	match app.run(handler) {
		Err(AppError::EventLoop(e)) => {
			eprintln!("application test skipped: no event loop ({e})");
			None
		}
		result => Some(result),
	}
}

/// Каталог шрифтов репозитория (пакет `gterm-demo`).
fn repo_fonts_dir() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("../gterm-demo/assets/fonts")
}

/// Завершить процесс с ошибкой, если цикл не закончился за [`WATCHDOG`]: зависание иначе
/// выглядело бы как бесконечный тест.
fn start_watchdog() {
	std::thread::spawn(|| {
		std::thread::sleep(WATCHDOG);
		eprintln!("application test: no exit within {WATCHDOG:?}, the loop is stuck");
		std::process::exit(1);
	});
}
