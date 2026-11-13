//! Этап 5 (ТЗ FR-19, FR-20, §9 п.6, §11.4 п.4, ADR-0007): разбор аргументов и окружения, цепочки
//! уровней.

use gterm::config::{BACKEND_ENV, BackendKind, Config, ConfigError};
use gterm::renderer::RendererKind;

fn parse(args: &[&str], env: Option<&str>) -> Result<Config, ConfigError> {
	Config::parse(args.iter().copied(), env)
}

#[test]
fn defaults_are_auto_with_gl_allowed() {
	let c = parse(&[], None).unwrap();
	assert_eq!(c, Config::default());
	assert_eq!(c.backend, BackendKind::Auto);
	assert!(!c.fail_gl && c.allow_sw_gl && !c.debug && !c.help);
	assert!(c.fail_frames == 0 && !c.lose_context);
}

#[test]
fn backend_from_cli_in_both_spellings_and_aliases() {
	assert_eq!(
		parse(&["--backend=gl"], None).unwrap().backend,
		BackendKind::Gl
	);
	assert_eq!(
		parse(&["--backend", "gles"], None).unwrap().backend,
		BackendKind::Gles
	);
	assert_eq!(
		parse(&["--backend=SOFT"], None).unwrap().backend,
		BackendKind::Soft
	);
	assert_eq!(
		parse(&["--backend=software"], None).unwrap().backend,
		BackendKind::Soft
	);
	assert_eq!(
		parse(&["--backend=opengl"], None).unwrap().backend,
		BackendKind::Gl
	);
	assert_eq!(
		parse(&["--backend=auto"], None).unwrap().backend,
		BackendKind::Auto
	);
}

#[test]
fn backend_from_env_and_cli_overrides_env() {
	assert_eq!(parse(&[], Some("soft")).unwrap().backend, BackendKind::Soft);
	assert_eq!(
		parse(&["--backend=gl"], Some("soft")).unwrap().backend,
		BackendKind::Gl
	);
}

#[test]
fn flags_and_booleans() {
	let c = parse(&["--fail-gl", "--debug", "--allow-sw-gl=false"], None).unwrap();
	assert!(c.fail_gl && c.debug && !c.allow_sw_gl);
	assert!(!parse(&["--fail-gl=0"], None).unwrap().fail_gl);
	assert!(parse(&["--allow-sw-gl=yes"], None).unwrap().allow_sw_gl);
	assert!(parse(&["-h"], None).unwrap().help);
	assert!(parse(&["--help"], None).unwrap().help);
}

#[test]
fn fault_injection_flags() {
	let c = parse(&["--fail-frames=2", "--lose-context"], None).unwrap();
	assert_eq!(c.fail_frames, 2);
	assert!(c.lose_context);
	assert_eq!(parse(&["--fail-frames", "4"], None).unwrap().fail_frames, 4);
	assert!(!parse(&["--lose-context=false"], None).unwrap().lose_context);
	assert_eq!(
		parse(&["--fail-frames"], None),
		Err(ConfigError::MissingValue("--fail-frames"))
	);
	for bad in ["many", "-1", ""] {
		assert_eq!(
			parse(&[&format!("--fail-frames={bad}")], None),
			Err(ConfigError::BadNumber {
				option: "--fail-frames",
				value: bad.to_owned()
			})
		);
	}
}

#[test]
fn errors_are_typed_and_name_the_source() {
	assert_eq!(
		parse(&["--backend=vulkan"], None),
		Err(ConfigError::BadBackend {
			source: "--backend",
			value: "vulkan".to_owned()
		})
	);
	assert_eq!(
		parse(&[], Some("dx12")),
		Err(ConfigError::BadBackend {
			source: BACKEND_ENV,
			value: "dx12".to_owned()
		})
	);
	assert_eq!(
		parse(&["--backend"], None),
		Err(ConfigError::MissingValue("--backend"))
	);
	assert_eq!(
		parse(&["--debug=maybe"], None),
		Err(ConfigError::BadBool {
			option: "--debug",
			value: "maybe".to_owned()
		})
	);
	assert_eq!(
		parse(&["--frobnicate"], None),
		Err(ConfigError::UnknownArgument("--frobnicate".to_owned()))
	);
	assert!(
		ConfigError::UnknownArgument("x".to_owned())
			.to_string()
			.contains("'x'")
	);
}

#[test]
fn chains_start_at_requested_level_and_end_in_software() {
	use RendererKind::*;
	assert_eq!(BackendKind::Auto.chain(), &[OpenGl33, Gles30, Software]);
	assert_eq!(BackendKind::Gl.chain(), &[OpenGl33, Software]);
	assert_eq!(BackendKind::Gles.chain(), &[Gles30, Software]);
	assert_eq!(BackendKind::Soft.chain(), &[Software]);
	for kind in [
		BackendKind::Auto,
		BackendKind::Gl,
		BackendKind::Gles,
		BackendKind::Soft,
	] {
		assert_eq!(
			kind.chain().last(),
			Some(&Software),
			"{kind:?} ends in Software"
		);
	}
	assert_eq!(OpenGl33.name(), "OpenGL 3.3");
	assert!(Gles30.is_gl() && !Software.is_gl());
}

#[test]
fn usage_mentions_every_option() {
	let u = Config::usage("demo");
	for opt in [
		"--backend",
		"--fail-gl",
		"--allow-sw-gl",
		"--debug",
		"--fail-frames",
		"--lose-context",
		"--help",
		BACKEND_ENV,
	] {
		assert!(u.contains(opt), "usage lacks {opt}");
	}
}
