//! Системные шрифты (ADR-0017): семейство по имени среди установленных в системе и моноширинный
//! шрифт по умолчанию. База `fontdb` строится на время вызова: каталоги шрифтов Windows и Linux
//! (на Linux — по fontconfig, если он есть), файлы отображаются в память и читаются только
//! таблицы заголовка; целиком читаются лишь файлы выбранного семейства.

use std::path::PathBuf;

use fontdb::{Database, Family, Source};

use super::{FontError, FontFamily};

/// Кандидаты моноширинного семейства по умолчанию в порядке предпочтения; обобщённое семейство
/// `monospace` платформы пробуется до или после них (см. [`FontFamily::system_monospace`]).
#[cfg(windows)]
const MONOSPACE_CANDIDATES: &[&str] =
	&["Cascadia Mono", "Consolas", "Lucida Console", "Courier New"];
#[cfg(not(windows))]
const MONOSPACE_CANDIDATES: &[&str] = &[
	"DejaVu Sans Mono",
	"Liberation Mono",
	"Noto Sans Mono",
	"FreeMono",
];

impl FontFamily {
	/// Семейство `family` из установленных в системе шрифтов: те же правила отбора начертаний и
	/// проверки, что у [`Self::load`]. Семейства нет — `NoRegularFace` с пустым `found`.
	pub fn system(family: &str) -> Result<Self, FontError> {
		Self::from_system_database(&system_database(), family)
	}

	/// Моноширинный шрифт системы по умолчанию: первое из семейств-кандидатов, у которого есть
	/// Regular и которое прошло проверку моноширинности. На Linux первым пробуется обобщённое
	/// семейство `monospace` из fontconfig, затем список кандидатов; на Windows — список, затем
	/// умолчание `fontdb` (`Courier New`). Ни одно не подошло — `NoSystemMonospace` с причиной
	/// по каждому кандидату.
	pub fn system_monospace() -> Result<Self, FontError> {
		let db = system_database();
		let generic = db.family_name(&Family::Monospace).to_owned();
		let listed = MONOSPACE_CANDIDATES.iter().copied();
		let ordered: Vec<&str> = if cfg!(windows) {
			listed.chain([generic.as_str()]).collect()
		} else {
			[generic.as_str()].into_iter().chain(listed).collect()
		};
		let mut candidates: Vec<&str> = Vec::with_capacity(ordered.len());
		for candidate in ordered {
			if !candidates.contains(&candidate) {
				candidates.push(candidate);
			}
		}
		let mut tried = Vec::with_capacity(candidates.len());
		for candidate in candidates {
			match Self::from_system_database(&db, candidate) {
				Ok(family) => return Ok(family),
				Err(e) => tried.push(format!("{candidate}: {e}")),
			}
		}
		Err(FontError::NoSystemMonospace { tried })
	}

	/// Файлы семейства из базы (имя — семейство файла по `fontdb`, в том числе типографское);
	/// окончательный отбор по `name` ID 1 — в [`Self::from_files`].
	fn from_system_database(db: &Database, family: &str) -> Result<Self, FontError> {
		let mut paths: Vec<PathBuf> = db
			.faces()
			.filter(|face| face.families.iter().any(|(name, _)| name == family))
			.filter_map(|face| match &face.source {
				Source::File(path) | Source::SharedFile(path, _) => Some(path.clone()),
				Source::Binary(_) => None,
			})
			.collect();
		paths.sort();
		paths.dedup();
		Self::from_files(&paths, family)
	}
}

/// Все установленные шрифты системы.
fn system_database() -> Database {
	let mut db = Database::new();
	db.load_system_fonts();
	db
}
