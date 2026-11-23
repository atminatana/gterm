//! Каталоги тем, перечень их тем и тема на экране. Первой в перечне идёт встроенная тема
//! (`Theme::default()`), за ней — имена файлов каталогов по алфавиту (`Theme::list`).

use std::path::PathBuf;

use gterm_widgets::{Theme, ThemeError};

/// Разделитель каталогов в поле ввода и между значениями ключа `--theme-dir=`.
pub const DIR_SEPARATOR: char = ';';

/// Имя файла для записи встроенной темы и добавка к имени файла полной записи.
const BUILT_IN_FILE: &str = "default";
const SAVE_SUFFIX: &str = "_full";

/// Подпись встроенной темы в перечне.
const BUILT_IN_TITLE: &str = "‹встроенная›";

pub struct Catalog {
	dirs: Vec<PathBuf>,
	/// Имена тем каталогов, по алфавиту.
	names: Vec<String>,
	/// Номер темы на экране в перечне: 0 — встроенная, дальше `names` со сдвигом на единицу.
	current: usize,
	theme: Theme,
}

impl Catalog {
	/// Без каталогов: в перечне одна встроенная тема, она же на экране.
	pub fn new() -> Self {
		Self {
			dirs: Vec::new(),
			names: Vec::new(),
			current: 0,
			theme: Theme::default(),
		}
	}

	/// Сменить каталоги и перечитать перечень. Каталог не прочитан — перечень остаётся из одной
	/// встроенной темы. Тема на экране не меняется; её номер ищется в новом перечне по имени, а
	/// если её там нет — указывает на встроенную.
	pub fn scan(&mut self, dirs: Vec<PathBuf>) -> Result<(), ThemeError> {
		let shown = self.title(self.current).to_owned();
		let listed = Theme::list(&dirs);
		self.dirs = dirs;
		let result = match listed {
			Ok(names) => {
				self.names = names;
				Ok(())
			}
			Err(error) => {
				self.names.clear();
				Err(error)
			}
		};
		self.current = self.position(&shown).unwrap_or(0);
		result
	}

	/// Загрузить тему перечня и показать её; при ошибке на экране остаётся прежняя.
	pub fn select(&mut self, index: usize) -> Result<(), ThemeError> {
		let index = index.min(self.names.len());
		self.theme = match index.checked_sub(1) {
			Some(file) => Theme::load(&self.dirs, &self.names[file])?,
			None => Theme::default(),
		};
		self.current = index;
		Ok(())
	}

	/// Число тем перечня вместе со встроенной.
	pub fn len(&self) -> usize {
		self.names.len() + 1
	}

	pub fn dir_count(&self) -> usize {
		self.dirs.len()
	}

	pub fn dirs(&self) -> &[PathBuf] {
		&self.dirs
	}

	/// Куда предложить записать тему на экране: `<имя>_full.toml` в первом каталоге перечня,
	/// без каталогов — в текущем.
	pub fn save_path(&self) -> PathBuf {
		let stem = self
			.current
			.checked_sub(1)
			.and_then(|file| self.names.get(file))
			.map_or(BUILT_IN_FILE, String::as_str);
		let file = format!("{stem}{SAVE_SUFFIX}.toml");
		match self.dirs.first() {
			Some(dir) => dir.join(file),
			None => PathBuf::from(file),
		}
	}

	/// Подписи перечня: встроенная тема, затем имена файлов.
	pub fn titles(&self) -> impl Iterator<Item = &str> {
		std::iter::once(BUILT_IN_TITLE).chain(self.names.iter().map(String::as_str))
	}

	pub fn title(&self, index: usize) -> &str {
		match index.checked_sub(1).and_then(|file| self.names.get(file)) {
			Some(name) => name,
			None => BUILT_IN_TITLE,
		}
	}

	/// Номер темы из файла `<name>.toml` в перечне.
	pub fn position(&self, name: &str) -> Option<usize> {
		let file = self.names.iter().position(|known| known == name)?;
		Some(file + 1)
	}

	pub fn current(&self) -> usize {
		self.current
	}

	pub fn theme(&self) -> &Theme {
		&self.theme
	}
}

/// Каталоги из строки: пути через [`DIR_SEPARATOR`], пробелы по краям и пустые части не в счёт.
pub fn parse_dirs(text: &str) -> Vec<PathBuf> {
	text.split(DIR_SEPARATOR)
		.map(str::trim)
		.filter(|part| !part.is_empty())
		.map(PathBuf::from)
		.collect()
}

/// Обратное к [`parse_dirs`]: строка для поля ввода.
pub fn join_dirs(dirs: &[PathBuf]) -> String {
	let parts: Vec<String> = dirs.iter().map(|dir| dir.display().to_string()).collect();
	parts.join(&DIR_SEPARATOR.to_string())
}
