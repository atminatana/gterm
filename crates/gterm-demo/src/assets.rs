//! Каталог ресурсов демо-бинарников `assets/` (ТЗ FR-8.1, ADR-0017): шрифт семейства демо в
//! `fonts/`, SVG-глифы в `svg/`. Библиотека `gterm` каталогов относительно exe не ищет —
//! это обязанность приложения.

use std::path::{Path, PathBuf};

/// Имя семейства шрифта демо в `assets/fonts/` (сравнивается с `name` ID 1 файла): именно вариант
/// **Mono** — в нём иконки ужаты в один advance, иначе они шире ячейки и обрезались бы (ТЗ FR-8).
pub const FONT_FAMILY: &str = "RobotoMono Nerd Font Mono";
/// Имя каталога ресурсов рядом с exe (FR-8.1).
pub const ASSETS_DIR_NAME: &str = "assets";
/// Подкаталог со шрифтами внутри `assets/` (FR-8.1).
pub const FONTS_DIR_NAME: &str = "fonts";
/// Подкаталог SVG-глифов внутри `assets/` (FR-8.6).
pub const SVG_DIR_NAME: &str = "svg";
/// Глубина поиска `assets/` вверх по родителям (из `target/debug/` до корня workspace — 2–3 уровня).
const SEARCH_PARENTS_DEPTH: u8 = 4;
/// Глубина поиска `assets/` вниз по потомкам.
const SEARCH_KIDS_DEPTH: u8 = 1;

/// Найти каталог `assets/` (FR-8.1, крейт `find_folder`): от каталога exe — потомки, затем
/// родители; не нашлось — так же от рабочего каталога; не нашлось — каталог пакета на момент
/// сборки (`cargo run` из корня workspace: `assets/` лежит в `crates/gterm-demo/`, а не на пути
/// от `target/debug/` вверх). У установленного exe пути сборки нет — он просто пропускается.
pub fn find_assets_dir() -> Option<PathBuf> {
	let search = find_folder::Search::KidsThenParents(SEARCH_KIDS_DEPTH, SEARCH_PARENTS_DEPTH);
	let from_exe = std::env::current_exe()
		.ok()
		.and_then(|exe| exe.parent().map(Path::to_path_buf))
		.and_then(|dir| search.of(dir).for_folder(ASSETS_DIR_NAME).ok());
	from_exe
		.or_else(|| search.for_folder(ASSETS_DIR_NAME).ok())
		.or_else(|| {
			let package_assets = Path::new(env!("CARGO_MANIFEST_DIR")).join(ASSETS_DIR_NAME);
			package_assets.is_dir().then_some(package_assets)
		})
}

/// Каталог шрифтов внутри `assets/`.
pub fn fonts_dir(assets_dir: &Path) -> PathBuf {
	assets_dir.join(FONTS_DIR_NAME)
}

/// Каталог SVG-глифов внутри `assets/`.
pub fn svg_dir(assets_dir: &Path) -> PathBuf {
	assets_dir.join(SVG_DIR_NAME)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn find_assets_dir_locates_package_assets_from_test_cwd() {
		// Тесты пакета запускаются из `crates/gterm-demo/`: `assets/` — потомок рабочего каталога.
		let found = find_assets_dir().expect("assets dir");
		assert_eq!(
			found.canonicalize().unwrap(),
			Path::new(env!("CARGO_MANIFEST_DIR"))
				.join(ASSETS_DIR_NAME)
				.canonicalize()
				.unwrap()
		);
		assert!(fonts_dir(&found).is_dir());
	}
}
