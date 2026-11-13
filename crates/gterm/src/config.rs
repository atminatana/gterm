//! Конфигурация запуска (ТЗ §5.1, FR-19, FR-20, §9 п.6, §11.4 п.4, ADR-0007): аргументы командной строки
//! и переменная окружения `TERM_BACKEND`. Приоритет: аргументы > окружение > умолчания.

use std::fmt;

use crate::renderer::RendererKind;

/// Переменная окружения с бэкендом (FR-19).
pub const BACKEND_ENV: &str = "TERM_BACKEND";

/// Запрошенный бэкенд (FR-19). Значение — **стартовый уровень** цепочки FR-17: ниже него
/// откат продолжается (§11.2: `--backend=gl` без GL должен закончиться на `soft`, не падением).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BackendKind {
	#[default]
	Auto,
	Gl,
	Gles,
	Soft,
}

impl BackendKind {
	/// Цепочка уровней для запрошенного бэкенда.
	pub const fn chain(self) -> &'static [RendererKind] {
		match self {
			Self::Auto => &[
				RendererKind::OpenGl33,
				RendererKind::Gles30,
				RendererKind::Software,
			],
			Self::Gl => &[RendererKind::OpenGl33, RendererKind::Software],
			Self::Gles => &[RendererKind::Gles30, RendererKind::Software],
			Self::Soft => &[RendererKind::Software],
		}
	}

	fn parse(value: &str) -> Option<Self> {
		match value.trim().to_ascii_lowercase().as_str() {
			"auto" => Some(Self::Auto),
			"gl" | "opengl" => Some(Self::Gl),
			"gles" | "es" => Some(Self::Gles),
			"soft" | "software" | "cpu" => Some(Self::Soft),
			_ => None,
		}
	}
}

/// Параметры запуска.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
	/// Запрошенный бэкенд.
	pub backend: BackendKind,
	/// Имитировать отказ создания любого GL-контекста (§11.4 п.4).
	pub fail_gl: bool,
	/// Разрешать программные GL-драйверы (`llvmpipe`, `softpipe`); `GDI Generic` — отказ всегда (§9 п.6).
	pub allow_sw_gl: bool,
	/// Имя бэкенда в заголовке окна (FR-20).
	pub debug: bool,
	/// Сколько первых попыток кадра завершить ошибкой рендера (ADR-0007); счётчик общий на запуск.
	pub fail_frames: u32,
	/// Завершить первую попытку кадра потерей контекста (ADR-0007).
	pub lose_context: bool,
	/// Запрошена справка.
	pub help: bool,
}

impl Default for Config {
	fn default() -> Self {
		Self {
			backend: BackendKind::Auto,
			fail_gl: false,
			allow_sw_gl: true,
			debug: false,
			fail_frames: 0,
			lose_context: false,
			help: false,
		}
	}
}

/// Ошибка разбора аргументов; сообщение пригодно для stderr.
#[derive(Debug, PartialEq, Eq)]
pub enum ConfigError {
	UnknownArgument(String),
	BadBackend { source: &'static str, value: String },
	BadBool { option: &'static str, value: String },
	BadNumber { option: &'static str, value: String },
	MissingValue(&'static str),
}

impl fmt::Display for ConfigError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::UnknownArgument(a) => write!(f, "unknown argument '{a}'"),
			Self::BadBackend { source, value } => {
				write!(
					f,
					"{source}: unknown backend '{value}' (expected auto|gl|gles|soft)"
				)
			}
			Self::BadBool { option, value } => {
				write!(f, "{option}: expected true|false, got '{value}'")
			}
			Self::BadNumber { option, value } => {
				write!(
					f,
					"{option}: expected a non-negative integer, got '{value}'"
				)
			}
			Self::MissingValue(option) => write!(f, "{option}: value required"),
		}
	}
}

impl std::error::Error for ConfigError {}

impl Config {
	/// Разобрать аргументы (без имени программы) и окружение.
	pub fn parse<I, S>(args: I, env_backend: Option<&str>) -> Result<Self, ConfigError>
	where
		I: IntoIterator<Item = S>,
		S: AsRef<str>,
	{
		let mut config = Self::default();
		if let Some(value) = env_backend {
			config.backend = BackendKind::parse(value).ok_or_else(|| ConfigError::BadBackend {
				source: BACKEND_ENV,
				value: value.to_owned(),
			})?;
		}
		let mut args = args.into_iter();
		while let Some(arg) = args.next() {
			let arg = arg.as_ref();
			let (name, inline_value) = match arg.split_once('=') {
				Some((n, v)) => (n, Some(v.to_owned())),
				None => (arg, None),
			};
			match name {
				"--backend" => {
					let value = take_value("--backend", inline_value, &mut args)?;
					config.backend = BackendKind::parse(&value).ok_or(ConfigError::BadBackend {
						source: "--backend",
						value,
					})?;
				}
				"--fail-gl" => config.fail_gl = parse_bool("--fail-gl", inline_value, true)?,
				"--allow-sw-gl" => {
					config.allow_sw_gl = parse_bool("--allow-sw-gl", inline_value, true)?;
				}
				"--debug" => config.debug = parse_bool("--debug", inline_value, true)?,
				"--fail-frames" => {
					let value = take_value("--fail-frames", inline_value, &mut args)?;
					config.fail_frames = value.parse().map_err(|_| ConfigError::BadNumber {
						option: "--fail-frames",
						value,
					})?;
				}
				"--lose-context" => {
					config.lose_context = parse_bool("--lose-context", inline_value, true)?;
				}
				"--help" | "-h" => config.help = true,
				_ => return Err(ConfigError::UnknownArgument(arg.to_owned())),
			}
		}
		Ok(config)
	}

	/// Разобрать `std::env::args()` и `TERM_BACKEND` процесса.
	pub fn from_process() -> Result<Self, ConfigError> {
		let env = std::env::var(BACKEND_ENV).ok();
		Self::parse(std::env::args().skip(1), env.as_deref())
	}

	/// Текст справки (FR-19, FR-20).
	pub fn usage(program: &str) -> String {
		format!(
			"usage: {program} [--backend=auto|gl|gles|soft] [--fail-gl] [--allow-sw-gl=false] [--debug]\n\
             \x20      [--fail-frames=N] [--lose-context]\n\
             \x20 --backend=X      start level of the fallback chain (default auto; env {BACKEND_ENV})\n\
             \x20 --fail-gl        simulate GL context creation failure\n\
             \x20 --allow-sw-gl=B  accept software GL drivers like llvmpipe (default true)\n\
             \x20 --debug          show backend name in the window title\n\
             \x20 --fail-frames=N  simulate a render error on the first N frame attempts\n\
             \x20 --lose-context   simulate GL context loss on the first frame\n\
             \x20 -h, --help       this text"
		)
	}
}

/// Значение параметра: из `--name=value` либо из следующего аргумента.
fn take_value<S: AsRef<str>>(
	option: &'static str,
	inline_value: Option<String>,
	args: &mut impl Iterator<Item = S>,
) -> Result<String, ConfigError> {
	match inline_value {
		Some(value) => Ok(value),
		None => args
			.next()
			.map(|s| s.as_ref().to_owned())
			.ok_or(ConfigError::MissingValue(option)),
	}
}

fn parse_bool(
	option: &'static str,
	value: Option<String>,
	default: bool,
) -> Result<bool, ConfigError> {
	match value.as_deref() {
		None => Ok(default),
		Some("true" | "1" | "yes" | "on") => Ok(true),
		Some("false" | "0" | "no" | "off") => Ok(false),
		Some(other) => Err(ConfigError::BadBool {
			option,
			value: other.to_owned(),
		}),
	}
}
