//! Этап 2 (ТЗ §11.1, ADR-0003): семейство из `assets/`, подстановка начертаний, растеризация,
//! динамический атлас с каналами, процедурная псевдографика.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use gterm::atlas::{Atlas, EMPTY_SLOT, PAGE_PX_MAX, PAGE_PX_MIN, page_px_for_limit};
use gterm::box_drawing;
use gterm::cell::Style;
use gterm::font::{
	FONT_SCALE, FONT_SIZE_PX, Face, FontError, FontFamily, GLYPH_PX_MAX, GLYPH_PX_MIN,
	cell_metrics, glyph_px, glyph_px_for, has_glyph_instructions,
};
use gterm::rasterizer::Rasterizer;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{
	DrawSettings, Engine, HintingInstance, HintingOptions, OutlinePen, SmoothMode, Target,
};
use skrifa::{FontRef, MetadataProvider};

/// Каталог шрифтов репозитория — источник TTF для тестов.
fn repo_fonts_dir() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("../gterm-demo/assets/fonts")
}

/// Кодовая точка, которой нет ни в одном шрифте: U+FFFF — noncharacter.
const CODEPOINT_ABSENT: char = '\u{FFFF}';
/// Иконка Material Design шрифта вне BMP (ADR-0009); младшие 16 бит совпадают с `A` — при
/// обрезке кодовой точки до u16 она слилась бы с латинской буквой.
const CODEPOINT_OUTSIDE_BMP: char = '\u{F0041}';
/// Полное покрытие.
const FULL: u8 = 255;
/// Имя семейства шрифта репозитория (`name` ID 1).
const FAMILY: &str = "RobotoMono Nerd Font Mono";
/// Семейства с таким именем нет ни в одном файле.
const FAMILY_ABSENT: &str = "No Such Family";

/// Временный каталог шрифтов с подмножеством файлов семейства.
struct TempFonts(PathBuf);

impl TempFonts {
	fn with_files(tag: &str, names: &[&str]) -> Self {
		static COUNTER: AtomicU32 = AtomicU32::new(0);
		let n = COUNTER.fetch_add(1, Ordering::Relaxed);
		let dir =
			std::env::temp_dir().join(format!("gterm-stage2-{tag}-{}-{n}", std::process::id()));
		fs::create_dir_all(&dir).unwrap();
		for name in names {
			fs::copy(repo_fonts_dir().join(name), dir.join(name)).unwrap();
		}
		Self(dir)
	}

	fn path(&self) -> &Path {
		&self.0
	}
}

impl Drop for TempFonts {
	fn drop(&mut self) {
		let _ = fs::remove_dir_all(&self.0);
	}
}

const REGULAR: &str = "RobotoMonoNerdFontMono-Regular.ttf";
const BOLD: &str = "RobotoMonoNerdFontMono-Bold.ttf";
const ITALIC: &str = "RobotoMonoNerdFontMono-Italic.ttf";
const BOLD_ITALIC: &str = "RobotoMonoNerdFontMono-BoldItalic.ttf";

fn repo_family() -> FontFamily {
	FontFamily::load(&repo_fonts_dir(), FAMILY).unwrap()
}

fn repo_rasterizer() -> Rasterizer {
	Rasterizer::new(repo_family()).unwrap()
}

/// Маленькая страница, чтобы тесты не выделяли 16 МБ и чтобы переполнение было достижимо.
fn small_atlas(page_px: u32) -> Atlas {
	Atlas::new(repo_rasterizer(), page_px)
}

fn has_ink(buf: &[u8]) -> bool {
	buf.iter().any(|&v| v > 0)
}

mod family {
	use super::*;

	#[test]
	fn loads_all_four_faces_by_fs_selection_not_by_name() {
		let family = repo_family();
		assert_eq!(family.faces().len(), 4);
		for style in Style::ALL {
			assert_eq!(
				family.style_source(style),
				style,
				"{style:?} has its own file"
			);
			assert_eq!(family.face(style).style, style);
		}
		let m = family.metrics();
		assert!(m.glyph_w >= 4 && m.glyph_h > m.glyph_w);
		assert!(m.baseline > 0 && (m.baseline as usize) < m.glyph_h);
	}

	#[test]
	fn missing_faces_fall_back_along_the_chain() {
		let only_regular = TempFonts::with_files("reg", &[REGULAR]);
		let family = FontFamily::load(only_regular.path(), FAMILY).unwrap();
		for style in Style::ALL {
			assert_eq!(family.style_source(style), Style::Regular);
		}

		let no_bold_italic = TempFonts::with_files("nobi", &[REGULAR, BOLD, ITALIC]);
		let family = FontFamily::load(no_bold_italic.path(), FAMILY).unwrap();
		assert_eq!(family.style_source(Style::BoldItalic), Style::Italic);
		assert_eq!(family.style_source(Style::Bold), Style::Bold);

		let regular_and_bold = TempFonts::with_files("rb", &[REGULAR, BOLD]);
		let family = FontFamily::load(regular_and_bold.path(), FAMILY).unwrap();
		assert_eq!(family.style_source(Style::Italic), Style::Regular);
		assert_eq!(family.style_source(Style::BoldItalic), Style::Bold);
	}

	#[test]
	fn missing_regular_is_an_error_even_if_other_faces_exist() {
		let no_regular = TempFonts::with_files("noreg", &[BOLD, ITALIC, BOLD_ITALIC]);
		assert!(matches!(
			FontFamily::load(no_regular.path(), FAMILY),
			Err(FontError::NoRegularFace { .. })
		));
		let empty = TempFonts::with_files("empty", &[]);
		assert!(matches!(
			FontFamily::load(empty.path(), FAMILY),
			Err(FontError::NoRegularFace { .. })
		));
		let missing_dir =
			std::env::temp_dir().join(format!("gterm-missing-{}", std::process::id()));
		assert!(matches!(
			FontFamily::load(&missing_dir, FAMILY),
			Err(FontError::Io(_, _))
		));
	}

	#[test]
	fn metrics_use_font_size_constant_and_reject_garbage() {
		let bytes = fs::read(repo_fonts_dir().join(REGULAR)).unwrap();
		let m = cell_metrics(&bytes, FONT_SIZE_PX).unwrap();
		assert_eq!(m, repo_family().metrics());
		assert!(matches!(
			cell_metrics(&[0u8; 64], FONT_SIZE_PX),
			Err(FontError::TtfParse(_))
		));
	}
}

mod sources {
	use super::*;

	fn coverage(r: &mut Rasterizer, ch: char, style: Style) -> Vec<u8> {
		let m = r.metrics();
		let mut buf = vec![0u8; m.glyph_w * m.glyph_h];
		assert!(r.rasterize(ch, style, &mut buf), "{ch} {style:?} drawn");
		buf
	}

	#[test]
	fn wrong_family_name_is_no_regular_face_listing_every_file() {
		match FontFamily::load(&repo_fonts_dir(), FAMILY_ABSENT) {
			Err(FontError::NoRegularFace { family, found }) => {
				assert_eq!(family, FAMILY_ABSENT);
				assert_eq!(found.len(), 4, "{found:?}");
			}
			other => panic!("expected NoRegularFace, got {other:?}"),
		}
	}

	/// К3 (ADR-0017): семейство из явного списка файлов неотличимо от загруженного из каталога.
	#[test]
	fn from_files_matches_load() {
		let from_dir = repo_family();
		let paths: Vec<PathBuf> = [REGULAR, BOLD, ITALIC, BOLD_ITALIC]
			.iter()
			.map(|name| repo_fonts_dir().join(name))
			.collect();
		let from_files = FontFamily::from_files(&paths, FAMILY).unwrap();
		assert_eq!(from_files.metrics(), from_dir.metrics());
		for style in Style::ALL {
			assert_eq!(from_files.style_source(style), from_dir.style_source(style));
			assert_eq!(from_files.face(style).path, from_dir.face(style).path);
		}
		let mut a = Rasterizer::new(from_dir).unwrap();
		let mut b = Rasterizer::new(from_files).unwrap();
		for (ch, style) in [('A', Style::Regular), ('Я', Style::Bold)] {
			assert_eq!(
				coverage(&mut a, ch, style),
				coverage(&mut b, ch, style),
				"{ch} {style:?}"
			);
		}
	}

	/// К4 (ADR-0017): пропорциональный шрифт отклоняется — на копии Regular, где `i` уже `M`.
	#[test]
	fn proportional_font_is_rejected() {
		let regular = fs::read(repo_fonts_dir().join(REGULAR)).unwrap();
		let face = Face {
			path: PathBuf::from(REGULAR),
			bytes: with_narrow_i(&regular),
			style: Style::Regular,
		};
		match FontFamily::from_faces(vec![face], FAMILY, Vec::new()) {
			Err(FontError::NotMonospace {
				glyph,
				advance,
				expected,
				..
			}) => {
				assert_eq!(glyph, 'i');
				assert!(advance < expected, "{advance} < {expected}");
			}
			other => panic!("expected NotMonospace, got {other:?}"),
		}
	}

	/// К1 (ADR-0025): среди двух Regular побеждает файл, чьи глифы несут родные инструкции,
	/// при любом порядке файлов — на копии Regular с ненулевым `maxp.maxSizeOfInstructions`.
	#[test]
	fn face_with_glyph_instructions_is_preferred_regardless_of_order() {
		let plain = fs::read(repo_fonts_dir().join(REGULAR)).unwrap();
		assert!(!has_glyph_instructions(&FontRef::new(&plain).unwrap()));
		let instructed = with_instruction_budget(&plain, 1);
		assert!(has_glyph_instructions(&FontRef::new(&instructed).unwrap()));
		let face = |name: &str, bytes: &[u8]| Face {
			path: PathBuf::from(name),
			bytes: bytes.to_vec(),
			style: Style::Regular,
		};
		for faces in [
			vec![face("plain", &plain), face("instructed", &instructed)],
			vec![face("instructed", &instructed), face("plain", &plain)],
		] {
			let family = FontFamily::from_faces(faces, FAMILY, Vec::new()).unwrap();
			for style in Style::ALL {
				assert_eq!(
					family.face(style).path,
					PathBuf::from("instructed"),
					"{style:?}"
				);
			}
		}
	}

	/// Копия TTF, где `maxp.maxSizeOfInstructions` (u16 по смещению 26 в `maxp` версии 1.0)
	/// равен `budget`: признак инструкций без самих инструкций — глифы остаются прежними.
	fn with_instruction_budget(ttf: &[u8], budget: u16) -> Vec<u8> {
		let (maxp, _) = table_bounds(ttf, b"maxp");
		let mut out = ttf.to_vec();
		out[maxp + 26..maxp + 28].copy_from_slice(&budget.to_be_bytes());
		out
	}

	/// Смещение и длина таблицы `tag` из каталога таблиц TTF.
	fn table_bounds(ttf: &[u8], tag: &[u8; 4]) -> (usize, usize) {
		let count = u16::from_be_bytes([ttf[4], ttf[5]]) as usize;
		let record = (0..count)
			.map(|n| 12 + 16 * n)
			.find(|&r| &ttf[r..r + 4] == tag)
			.unwrap();
		let offset = u32::from_be_bytes(ttf[record + 8..record + 12].try_into().unwrap());
		let length = u32::from_be_bytes(ttf[record + 12..record + 16].try_into().unwrap());
		(offset as usize, length as usize)
	}

	/// Копия TTF, где advance `i` — половина advance `M`. У моноширинного шрифта
	/// `hhea.numberOfHMetrics` мал (все глифы делят последнюю запись `hmtx`), поэтому таблица
	/// расширяется до глифов `M` и `i`, а их записи задаются явно; записи между ними читаются из
	/// бывшего массива lsb, но проверка упирается в `i` первым. Контрольные суммы `skrifa` не
	/// проверяет; смещения таблиц — из каталога таблиц TTF.
	fn with_narrow_i(ttf: &[u8]) -> Vec<u8> {
		let font = FontRef::new(ttf).unwrap();
		let gid = |ch: char| font.charmap().map(ch).unwrap().to_u32() as usize;
		let (m, i) = (gid('M'), gid('i'));
		let table = |tag: &[u8; 4]| table_bounds(ttf, tag);
		let ((hhea, _), (hmtx, hmtx_len)) = (table(b"hhea"), table(b"hmtx"));
		let metrics_count = u16::from_be_bytes([ttf[hhea + 34], ttf[hhea + 35]]) as usize;
		let last = hmtx + 4 * (metrics_count - 1);
		let advance_m = u16::from_be_bytes([ttf[last], ttf[last + 1]]);
		let new_count = m.max(i) + 1;
		assert!(
			4 * new_count <= hmtx_len,
			"hmtx too short for {new_count} metrics"
		);
		let mut out = ttf.to_vec();
		out[hhea + 34..hhea + 36].copy_from_slice(&(new_count as u16).to_be_bytes());
		out[hmtx + 4 * m..hmtx + 4 * m + 2].copy_from_slice(&advance_m.to_be_bytes());
		out[hmtx + 4 * i..hmtx + 4 * i + 2].copy_from_slice(&(advance_m / 2).to_be_bytes());
		out
	}
}

mod system {
	use std::time::Instant;

	use super::*;

	/// Печатает причину и даёт `None`, если в системе нет моноширинного шрифта (как `stage4_gl`
	/// без GL): тест зависит от окружения.
	fn system_monospace_or_skip(test: &str) -> Option<FontFamily> {
		match FontFamily::system_monospace() {
			Ok(family) => Some(family),
			Err(e) => {
				eprintln!("{test}: skipped — {e}");
				None
			}
		}
	}

	/// К5/К7 (ADR-0017): выбор детерминирован — два вызова дают те же файлы; время первого — в
	/// stderr.
	#[test]
	fn system_monospace_is_deterministic() {
		let started = Instant::now();
		let Some(first) = system_monospace_or_skip("system_monospace_is_deterministic") else {
			return;
		};
		eprintln!(
			"system_monospace: '{}' {} in {:?}",
			first.family(),
			first.face(Style::Regular).path.display(),
			started.elapsed()
		);
		let second = FontFamily::system_monospace().unwrap();
		assert_eq!(first.family(), second.family());
		assert_eq!(first.metrics(), second.metrics());
		for style in Style::ALL {
			assert_eq!(first.face(style).path, second.face(style).path);
			assert_eq!(first.style_source(style), second.style_source(style));
		}
	}

	/// К6 (ADR-0017): семейство по имени; отсутствующее — `NoRegularFace` без найденных файлов.
	#[test]
	fn system_family_by_name() {
		match FontFamily::system(FAMILY_ABSENT) {
			Err(FontError::NoRegularFace { family, found }) => {
				assert_eq!(family, FAMILY_ABSENT);
				assert!(found.is_empty(), "{found:?}");
			}
			other => panic!("expected NoRegularFace, got {other:?}"),
		}
		let Some(default) = system_monospace_or_skip("system_family_by_name") else {
			return;
		};
		let by_name = FontFamily::system(default.family()).unwrap();
		assert_eq!(
			by_name.face(Style::Regular).path,
			default.face(Style::Regular).path
		);
	}
}

mod rasterization {
	use super::*;

	fn raster(r: &mut Rasterizer, cp: char, style: Style) -> (bool, Vec<u8>) {
		let m = r.metrics();
		let mut buf = vec![0u8; m.glyph_w * m.glyph_h];
		let drawn = r.rasterize(cp, style, &mut buf);
		(drawn, buf)
	}

	#[test]
	fn latin_and_cyrillic_have_ink_with_full_and_partial_coverage() {
		let mut r = repo_rasterizer();
		let (drawn, a) = raster(&mut r, 'A', Style::Regular);
		assert!(drawn && a.contains(&FULL), "hinted stems give full pixels");
		assert!(
			a.iter().any(|&v| v > 0 && v < FULL),
			"AA gives partial pixels"
		);
		assert!(raster(&mut r, 'Я', Style::Regular).0);
		assert!(!raster(&mut r, ' ', Style::Regular).0);
		assert!(!raster(&mut r, CODEPOINT_ABSENT, Style::Regular).0);
		assert!(!r.has_glyph(CODEPOINT_ABSENT, Style::Regular));
		assert!(r.has_glyph('A', Style::BoldItalic));
	}

	#[test]
	fn glyph_outside_bmp_is_rasterized() {
		let mut r = repo_rasterizer();
		assert!(r.has_glyph(CODEPOINT_OUTSIDE_BMP, Style::Regular));
		let (drawn, icon) = raster(&mut r, CODEPOINT_OUTSIDE_BMP, Style::Regular);
		assert!(drawn && has_ink(&icon));
		assert_ne!(icon, raster(&mut r, 'A', Style::Regular).1);
	}

	#[test]
	fn styles_render_differently_but_share_the_cell() {
		let mut r = repo_rasterizer();
		let regular = raster(&mut r, 'A', Style::Regular).1;
		let bold = raster(&mut r, 'A', Style::Bold).1;
		let italic = raster(&mut r, 'A', Style::Italic).1;
		assert_ne!(regular, bold);
		assert_ne!(regular, italic);
		assert_eq!(regular.len(), bold.len());
		// Жирный несёт больше чернил.
		let ink = |b: &[u8]| b.iter().map(|&v| v as u32).sum::<u32>();
		assert!(ink(&bold) > ink(&regular));
	}

	#[test]
	fn fallback_style_renders_exactly_like_its_source() {
		let only_regular = TempFonts::with_files("fb", &[REGULAR]);
		let mut r =
			Rasterizer::new(FontFamily::load(only_regular.path(), FAMILY).unwrap()).unwrap();
		assert_eq!(
			raster(&mut r, 'g', Style::Bold).1,
			raster(&mut r, 'g', Style::Regular).1
		);
	}

	#[test]
	fn box_drawing_comes_from_procedural_generator_in_every_style() {
		let mut r = repo_rasterizer();
		let m = r.metrics();
		let mut expected = vec![0u8; m.glyph_w * m.glyph_h];
		assert!(box_drawing::draw(
			'\u{2500}',
			m.glyph_w,
			m.glyph_h,
			&mut expected
		));
		for style in Style::ALL {
			assert_eq!(raster(&mut r, '\u{2500}', style).1, expected, "{style:?}");
		}
		assert!(raster(&mut r, '\u{259F}', Style::Regular).0);
	}

	/// К2 (ADR-0025): шрифт без родных инструкций хинтуется автохинтером в `Smooth/Normal`:
	/// растр совпадает с прямым рендером `skrifa` + `zeno` тем же путём и отличается от `Mono` —
	/// откат к прежней цели тест ловит.
	#[test]
	fn uninstructed_font_is_autohinted_with_smooth_target() {
		let mut r = repo_rasterizer();
		let m = r.metrics();
		let bytes = fs::read(repo_fonts_dir().join(REGULAR)).unwrap();
		let font = FontRef::new(&bytes).unwrap();
		assert!(!has_glyph_instructions(&font));
		let render = |ch: char, target: Target| -> Vec<u8> {
			let outlines = font.outline_glyphs();
			let options = HintingOptions {
				engine: Engine::Auto(None),
				target,
			};
			let size = Size::new(f32::from(FONT_SIZE_PX));
			let hinter =
				HintingInstance::new(&outlines, size, LocationRef::default(), options).unwrap();
			let glyph = outlines.get(font.charmap().map(ch).unwrap()).unwrap();
			let mut pen = PathPen(Vec::new());
			glyph
				.draw(DrawSettings::hinted(&hinter, false), &mut pen)
				.unwrap();
			let mut out = vec![0u8; m.glyph_w * m.glyph_h];
			zeno::Mask::new(&pen.0[..])
				.size(m.glyph_w as u32, m.glyph_h as u32)
				.origin(zeno::Origin::BottomLeft)
				.offset(zeno::Vector::new(
					0.0,
					(m.glyph_h as i32 - m.baseline) as f32,
				))
				.render_into(&mut out, None);
			out
		};
		for ch in ['к', 'С'] {
			let actual = raster(&mut r, ch, Style::Regular).1;
			assert_eq!(actual, render(ch, SmoothMode::Normal.into()), "{ch} smooth");
			assert_ne!(
				actual,
				render(ch, Target::Mono),
				"{ch}: targets must differ"
			);
		}
	}

	/// Контур глифа в командах `zeno`, как `PathPen` растеризатора.
	struct PathPen(Vec<zeno::Command>);

	impl OutlinePen for PathPen {
		fn move_to(&mut self, x: f32, y: f32) {
			self.0.push(zeno::Command::MoveTo((x, y).into()));
		}
		fn line_to(&mut self, x: f32, y: f32) {
			self.0.push(zeno::Command::LineTo((x, y).into()));
		}
		fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
			self.0
				.push(zeno::Command::QuadTo((cx, cy).into(), (x, y).into()));
		}
		fn curve_to(&mut self, c0x: f32, c0y: f32, c1x: f32, c1y: f32, x: f32, y: f32) {
			self.0.push(zeno::Command::CurveTo(
				(c0x, c0y).into(),
				(c1x, c1y).into(),
				(x, y).into(),
			));
		}
		fn close(&mut self) {
			self.0.push(zeno::Command::Close);
		}
	}

	#[test]
	fn rasterization_is_deterministic() {
		let mut a = repo_rasterizer();
		let mut b = repo_rasterizer();
		for cp in ['A', 'g', 'Ж', '\u{2588}'] {
			assert_eq!(
				raster(&mut a, cp, Style::Italic),
				raster(&mut b, cp, Style::Italic)
			);
		}
	}
}

mod procedural_box_drawing {
	use super::*;

	/// Кодовая точка — числом: в таблицах псевдографики HEX-код нагляднее литерала `char`.
	fn drawn(cp: u32, w: usize, h: usize) -> Vec<u8> {
		let ch = char::from_u32(cp).expect("кодовая точка псевдографики");
		let mut out = vec![0u8; w * h];
		assert!(box_drawing::draw(ch, w, h, &mut out));
		out
	}

	fn row(buf: &[u8], w: usize, y: usize) -> &[u8] {
		&buf[y * w..(y + 1) * w]
	}

	fn column(buf: &[u8], w: usize, h: usize, x: usize) -> Vec<u8> {
		(0..h).map(|y| buf[y * w + x]).collect()
	}

	#[test]
	fn light_horizontal_is_one_full_row_at_center() {
		let (w, h) = (10, 18);
		let buf = drawn(0x2500, w, h);
		assert!(row(&buf, w, 9).iter().all(|&v| v == 255));
		assert_eq!(buf.iter().filter(|&&v| v == 255).count(), w);
	}

	#[test]
	fn light_vertical_is_one_full_column_and_cross_joins_both() {
		let (w, h) = (10, 18);
		let vertical = drawn(0x2502, w, h);
		assert!(column(&vertical, w, h, 5).iter().all(|&v| v == 255));
		assert_eq!(vertical.iter().filter(|&&v| v == 255).count(), h);

		let cross = drawn(0x253C, w, h);
		assert!(row(&cross, w, 9).iter().all(|&v| v == 255));
		assert!(column(&cross, w, h, 5).iter().all(|&v| v == 255));
	}

	#[test]
	fn corners_meet_at_center_and_stop_at_edges() {
		let (w, h) = (10, 18);
		let corner = drawn(0x250C, w, h); // ┌: вниз + вправо
		assert_eq!(corner[9 * w + 5], 255);
		assert_eq!(corner[9 * w + 4], 0);
		assert_eq!(corner[8 * w + 5], 0);
		assert_eq!(corner[9 * w + 9], 255);
		assert_eq!(corner[17 * w + 5], 255);
	}

	/// Тройник на двойной линии: одиночная рука упирается в ближний штрих, зазор пуст (как в
	/// шрифтах); крест и угол проводят одиночную линию сквозь зазор.
	#[test]
	fn single_arm_onto_double_line_stops_at_near_stroke() {
		let (w, h) = (10, 18);
		// Ячейка 10×18: light = 1, центр (5, 9), штрихи двойной горизонтали — строки 8 и 10,
		// двойной вертикали — колонки 4 и 6.
		let tee_down = drawn(0x2564, w, h); // ╤
		assert_eq!(tee_down[9 * w + 5], 0, "╤: зазор пуст");
		assert_eq!(
			tee_down[10 * w + 5],
			255,
			"╤: вертикаль касается нижнего штриха"
		);
		assert!(column(&tee_down, w, h, 5)[11..].iter().all(|&v| v == 255));
		let tee_up = drawn(0x2567, w, h); // ╧
		assert_eq!(tee_up[9 * w + 5], 0, "╧: зазор пуст");
		assert_eq!(tee_up[8 * w + 5], 255);
		let tee_right = drawn(0x255F, w, h); // ╟
		assert_eq!(tee_right[9 * w + 5], 0, "╟: зазор пуст");
		assert!(row(&tee_right, w, 9)[6..].iter().all(|&v| v == 255));
		let tee_left = drawn(0x2562, w, h); // ╢
		assert_eq!(tee_left[9 * w + 5], 0, "╢: зазор пуст");
		assert!(row(&tee_left, w, 9)[..5].iter().all(|&v| v == 255));
		// Крест ╪ и угол ╒: одиночная линия идёт сквозь зазор, как в шрифтах.
		assert_eq!(drawn(0x256A, w, h)[9 * w + 5], 255, "╪: сквозь зазор");
		assert_eq!(drawn(0x2552, w, h)[9 * w + 5], 255, "╒: до внешнего штриха");
	}

	#[test]
	fn heavy_is_thicker_and_double_is_two_strokes() {
		let (w, h) = (10, 18);
		let heavy = drawn(0x2501, w, h);
		assert_eq!(heavy.iter().filter(|&&v| v == 255).count(), 3 * w);
		let double = drawn(0x2550, w, h);
		assert!(row(&double, w, 8).iter().all(|&v| v == 255));
		assert!(row(&double, w, 10).iter().all(|&v| v == 255));
		assert!(row(&double, w, 9).iter().all(|&v| v == 0));
	}

	#[test]
	fn blocks_shades_and_quadrants() {
		let (w, h) = (10, 18);
		assert!(drawn(0x2588, w, h).iter().all(|&v| v == 255));
		assert!(drawn(0x2591, w, h).iter().all(|&v| v == 64));
		let left_half = drawn(0x258C, w, h);
		assert!(row(&left_half, w, 0)[..5].iter().all(|&v| v == 255));
		assert!(row(&left_half, w, 0)[5..].iter().all(|&v| v == 0));
		let lower_half = drawn(0x2584, w, h);
		assert!(row(&lower_half, w, 8).iter().all(|&v| v == 0));
		assert!(row(&lower_half, w, 9).iter().all(|&v| v == 255));
		let quadrant = drawn(0x2598, w, h); // ▘ верх-лево
		assert_eq!(quadrant[0], 255);
		assert_eq!(quadrant[w - 1], 0);
		assert_eq!(quadrant[(h - 1) * w], 0);
	}

	/// Число непрерывных отрезков полного покрытия.
	fn runs(line: &[u8]) -> usize {
		let starts_inside = line.windows(2).filter(|p| p[0] != 255 && p[1] == 255);
		starts_inside.count() + usize::from(line.first() == Some(&255))
	}

	#[test]
	fn dashed_horizontal_has_gap_at_end_of_each_period() {
		let (w, h) = (10, 18);
		let buf = drawn(0x2504, w, h); // ┄: три штриха, периоды 3 + 3 + 4, зазор 1
		assert_eq!(
			row(&buf, w, 9),
			[255, 255, 0, 255, 255, 0, 255, 255, 255, 0]
		);
		assert_eq!(buf.iter().filter(|&&v| v != 0).count(), 7);
	}

	#[test]
	fn dash_count_follows_the_code_point() {
		let (w, h) = (10, 18);
		for (horizontal, vertical, count) in [
			(0x254C, 0x254E, 2),
			(0x2504, 0x2506, 3),
			(0x2508, 0x250A, 4),
		] {
			assert_eq!(runs(row(&drawn(horizontal, w, h), w, 9)), count);
			assert_eq!(runs(&column(&drawn(vertical, w, h), w, h, 5)), count);
		}
	}

	#[test]
	fn heavy_dashed_keeps_heavy_thickness() {
		let (w, h) = (10, 18);
		let buf = drawn(0x2505, w, h); // ┅
		for y in 8..=10 {
			assert_eq!(
				row(&buf, w, y),
				[255, 255, 0, 255, 255, 0, 255, 255, 255, 0]
			);
		}
	}

	#[test]
	fn dashes_degrade_to_fewer_when_cell_is_too_small() {
		let (w, h) = (5, 9);
		let buf = drawn(0x2508, w, h); // ┈: четыре штриха в 5 px не помещаются
		assert_eq!(row(&buf, w, 4), [255, 0, 255, 255, 0]);
	}

	#[test]
	fn arcs_end_where_straight_lines_run() {
		let (w, h) = (10, 18);
		// Концы: (x, y) на горизонтальном и на вертикальном краю ячейки.
		for (cp, horizontal_end, vertical_end) in [
			(0x256D, (9, 9), (5, 17)), // ╭: вправо + вниз
			(0x256E, (0, 9), (5, 17)), // ╮: влево + вниз
			(0x256F, (0, 9), (5, 0)),  // ╯: влево + вверх
			(0x2570, (9, 9), (5, 0)),  // ╰: вправо + вверх
		] {
			let buf = drawn(cp, w, h);
			assert_eq!(
				buf[horizontal_end.1 * w + horizontal_end.0],
				255,
				"U+{cp:04X}"
			);
			assert_eq!(buf[vertical_end.1 * w + vertical_end.0], 255, "U+{cp:04X}");
		}
	}

	#[test]
	fn arc_rounds_the_corner_and_is_antialiased() {
		let (w, h) = (10, 18);
		let buf = drawn(0x256D, w, h); // ╭
		assert_eq!(buf[9 * w + 5], 0, "угол, занятый у ┌, пуст");
		for y in 0..h {
			for x in 0..w {
				if x < 5 || y < 9 {
					assert_eq!(buf[y * w + x], 0, "вне четверти вправо-вниз: ({x}, {y})");
				}
			}
		}
		assert!(buf.iter().any(|&v| v != 0 && v != 255));
	}

	#[test]
	fn diagonal_runs_from_corner_to_corner_and_is_antialiased() {
		let (w, h) = (10, 18);
		let down = drawn(0x2572, w, h); // ╲
		assert_ne!(down[0], 0);
		assert_ne!(down[h * w - 1], 0);
		assert_eq!(down[w - 1], 0, "верхний правый угол пуст");
		assert_eq!(down[(h - 1) * w], 0, "нижний левый угол пуст");
		// Ось проходит через центр ячейки — рисунок совпадает со своим поворотом на 180°.
		let rotated: Vec<u8> = down.iter().rev().copied().collect();
		assert_eq!(down, rotated);
		assert!(down.iter().any(|&v| v != 0 && v != 255));
	}

	#[test]
	fn diagonals_mirror_each_other_and_cross_contains_both() {
		let (w, h) = (10, 18);
		let down = drawn(0x2572, w, h); // ╲
		let up = drawn(0x2571, w, h); // ╱
		let cross = drawn(0x2573, w, h); // ╳
		for y in 0..h {
			for x in 0..w {
				assert_eq!(up[y * w + x], down[y * w + w - 1 - x], "({x}, {y})");
				assert!(
					cross[y * w + x] >= up[y * w + x].max(down[y * w + x]),
					"({x}, {y})"
				);
			}
		}
	}

	#[test]
	fn diagonal_thickness_is_light() {
		// Площадь полосы ≈ толщина × длина диагонали за вычетом срезов в углах:
		// 10×18, light 1: 1 × 20,6 ≈ 20; 20×36, light 2: 2 × 41,2 ≈ 80.
		let area = |w: usize, h: usize| {
			let total: u32 = drawn(0x2572, w, h).iter().map(|&v| u32::from(v)).sum();
			total / 255
		};
		assert!((18..=22).contains(&area(10, 18)), "{}", area(10, 18));
		assert!((74..=86).contains(&area(20, 36)), "{}", area(20, 36));
	}

	#[test]
	fn outside_range_is_not_drawn() {
		let mut out = vec![7u8; 4];
		assert!(!box_drawing::draw('A', 2, 2, &mut out));
		assert_eq!(out, vec![7u8; 4]);
	}
}

mod atlas {
	use super::*;

	#[test]
	fn page_size_follows_driver_limit_within_bounds() {
		assert_eq!(page_px_for_limit(None), PAGE_PX_MIN);
		assert_eq!(page_px_for_limit(Some(512)), PAGE_PX_MIN);
		assert_eq!(page_px_for_limit(Some(1536)), 1536);
		assert_eq!(page_px_for_limit(Some(16384)), PAGE_PX_MAX);
	}

	#[test]
	fn miss_allocates_slot_and_hit_reuses_it() {
		let mut atlas = small_atlas(256);
		assert_eq!(atlas.used_slots(), 1); // пустой слот 0
		assert_eq!(atlas.slot_of('A'), None);

		let slot = atlas.ensure('A', Style::Regular);
		assert!(slot >= 1);
		assert_eq!(atlas.slot_of('A'), Some(slot));
		assert_eq!(atlas.ensure('A', Style::Regular), slot);
		assert_eq!(atlas.used_slots(), 2);
		assert!(atlas.is_ready(slot, Style::Regular));
		assert!(has_ink(&atlas.glyph('A', Style::Regular)));
	}

	#[test]
	fn styles_fill_channels_lazily_in_the_same_slot() {
		let mut atlas = small_atlas(256);
		let slot = atlas.ensure('A', Style::Regular);
		assert!(!atlas.is_ready(slot, Style::Bold));
		let channel_ink = |atlas: &Atlas, style: Style| {
			(0..atlas.glyph_h())
				.any(|y| (0..atlas.glyph_w()).any(|x| atlas.pixel(slot, x, y, style) > 0))
		};
		assert!(
			!channel_ink(&atlas, Style::Bold),
			"Bold channel is empty before ensure"
		);

		assert_eq!(atlas.ensure('A', Style::Bold), slot);
		assert!(atlas.is_ready(slot, Style::Bold));
		assert!(channel_ink(&atlas, Style::Bold));
		assert_ne!(
			atlas.glyph('A', Style::Bold),
			atlas.glyph('A', Style::Regular)
		);
		assert!(!atlas.is_ready(slot, Style::Italic));
	}

	#[test]
	fn codepoint_outside_bmp_gets_its_own_slot() {
		let mut atlas = small_atlas(256);
		let letter = atlas.ensure('A', Style::Regular);
		let icon = atlas.ensure(CODEPOINT_OUTSIDE_BMP, Style::Regular);
		assert_ne!(icon, EMPTY_SLOT);
		assert_ne!(icon, letter);
		assert_eq!(atlas.slot_of(CODEPOINT_OUTSIDE_BMP), Some(icon));
		assert_ne!(
			atlas.glyph_bytes(icon, Style::Regular),
			atlas.glyph_bytes(letter, Style::Regular)
		);
		// Последняя кодовая точка Unicode — граница таблицы атласа; глифа нет.
		assert_eq!(atlas.ensure(char::MAX, Style::Regular), EMPTY_SLOT);
	}

	#[test]
	fn absent_codepoint_maps_to_empty_slot_without_allocating() {
		let mut atlas = small_atlas(256);
		assert_eq!(atlas.ensure(CODEPOINT_ABSENT, Style::Regular), EMPTY_SLOT);
		assert_eq!(atlas.ensure('\0', Style::Bold), EMPTY_SLOT);
		assert_eq!(atlas.used_slots(), 1);
		assert!(
			atlas
				.glyph(CODEPOINT_ABSENT, Style::Regular)
				.iter()
				.all(|&v| v == 0)
		);
	}

	#[test]
	fn procedural_glyph_is_ready_in_all_channels_at_once() {
		let mut atlas = small_atlas(256);
		let slot = atlas.ensure('\u{2500}', Style::Regular);
		for style in Style::ALL {
			assert!(atlas.is_ready(slot, style), "{style:?}");
			assert!(has_ink(&atlas.glyph('\u{2500}', style)));
		}
	}

	#[test]
	fn overflow_resets_the_page_and_keeps_serving() {
		let mut atlas = small_atlas(64); // 6×3 = 18 слотов при 10×21
		let capacity = atlas.capacity();
		assert!(capacity < 40);
		let first = atlas.ensure('A', Style::Regular);
		for ch in ('\u{0400}'..).take(capacity) {
			atlas.ensure(ch, Style::Regular);
		}
		assert_eq!(atlas.resets(), 1);
		assert!(atlas.used_slots() <= capacity);
		// Старый слот недействителен: 'A' снова промах, но результат — тот же глиф.
		let again = atlas.ensure('A', Style::Regular);
		assert!(again >= 1);
		let _ = first;
		assert!(has_ink(&atlas.glyph('A', Style::Regular)));
	}

	#[test]
	fn dirty_slots_are_reported_once() {
		let mut atlas = small_atlas(256);
		let a = atlas.ensure('A', Style::Regular);
		let b = atlas.ensure('B', Style::Italic);
		assert_eq!(atlas.take_dirty_slots(), vec![a, b]);
		assert!(atlas.take_dirty_slots().is_empty());
		atlas.ensure('A', Style::Bold);
		assert_eq!(atlas.take_dirty_slots(), vec![a]);
	}

	#[test]
	fn slot_geometry_fits_the_page() {
		let atlas = small_atlas(PAGE_PX_MIN);
		let cols = atlas.page_px() / atlas.glyph_w();
		let rows = atlas.page_px() / atlas.glyph_h();
		assert_eq!(atlas.capacity(), cols * rows);
		let (ox, oy) = atlas.slot_origin((cols + 1) as u16);
		assert_eq!((ox, oy), (atlas.glyph_w(), atlas.glyph_h()));
		assert_eq!(atlas.cells().len(), atlas.page_px() * atlas.page_px() * 4);
	}
}

/// Размер шрифта как единственный источник геометрии (ADR-0005): формула, границы, метрики.
mod font_size {
	use super::*;

	#[test]
	fn size_follows_dpi_exactly_and_never_exceeds_bounds() {
		// Базовый случай: scale_factor 1 → базовый размер.
		assert_eq!(glyph_px(1.0), FONT_SIZE_PX);
		// Типичные мониторы: 16 → 24 → 32 px, округление до целого.
		assert_eq!(glyph_px(1.5), 24);
		assert_eq!(glyph_px(2.0), 32);
		assert_eq!(glyph_px(1.25), 20);
		assert_eq!(glyph_px(1.75), 28);
		// Границы зажимаются: 3.0 → 48, а 4.0 и 10.0 — тоже 48.
		assert_eq!(GLYPH_PX_MAX, 48);
		assert_eq!(glyph_px(3.0), GLYPH_PX_MAX);
		assert_eq!(glyph_px(4.0), GLYPH_PX_MAX);
		assert_eq!(glyph_px(1_000.0), GLYPH_PX_MAX);
		// Снизу: 0.5 → 8.
		assert_eq!(glyph_px(0.5), GLYPH_PX_MIN);
		assert_eq!(glyph_px(0.01), GLYPH_PX_MIN);
	}

	#[test]
	fn non_finite_and_non_positive_factors_fall_back_to_base_size() {
		// `f64::clamp` с NaN даёт NaN, округление упало бы в 0 и привело к панике в растеризаторе.
		for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, -1.0, -0.5] {
			assert_eq!(glyph_px(bad), FONT_SIZE_PX, "scale_factor {bad}");
			assert_eq!(glyph_px_for(FONT_SIZE_PX, FONT_SCALE, bad), FONT_SIZE_PX);
		}
		// Множитель пользователя проверяется так же.
		assert_eq!(glyph_px_for(FONT_SIZE_PX, 0.0, 1.0), FONT_SIZE_PX);
		assert_eq!(glyph_px_for(FONT_SIZE_PX, f32::NAN, 1.0), FONT_SIZE_PX);
		assert_eq!(glyph_px_for(FONT_SIZE_PX, 2.0, 1.0), 32);
	}

	#[test]
	fn user_scale_is_a_constant_one_in_this_version() {
		// Зума нет (FR-28), но множитель уже участвует в формуле — точка подключения настроек.
		assert_eq!(FONT_SCALE, 1.0);
		assert_eq!(
			glyph_px(FONT_SCALE as f64 * 1.0) as f64,
			FONT_SIZE_PX as f64
		);
	}

	#[test]
	fn metrics_at_size_match_the_direct_metric_helper() {
		let family = repo_family();
		assert_eq!(family.size_px(), FONT_SIZE_PX);
		assert_eq!(family.metrics(), family.metrics_at(FONT_SIZE_PX).unwrap());
		for size in [8u16, 12, 24, 32, 48] {
			let m = family.metrics_at(size).unwrap();
			let direct = cell_metrics(&family.face(Style::Regular).bytes, size).unwrap();
			assert_eq!(m, direct, "size {size}");
			assert!(m.glyph_w > 0 && m.glyph_h > 0, "size {size}");
			assert!(
				m.baseline > 0 && (m.baseline as usize) <= m.glyph_h,
				"size {size}"
			);
		}
	}

	#[test]
	fn cell_metrics_reject_a_zero_size_instead_of_panicking() {
		// Отсюда берётся защита рендереров `cell size must be >= 1 px`: нулевой размер недопустим.
		let bytes = repo_family().face(Style::Regular).bytes.clone();
		assert!(matches!(
			cell_metrics(&bytes, 0),
			Err(FontError::BadGlyphSize { .. })
		));
	}

	#[test]
	fn rasterizer_metrics_and_hinting_follow_the_requested_size() {
		for size in [8u16, 16, 32, 48] {
			let mut r = Rasterizer::new_at(repo_family(), size).unwrap();
			assert_eq!(r.size_px(), size);
			let m = r.metrics();
			assert_eq!(m, repo_family().metrics_at(size).unwrap());
			let mut buf = vec![0u8; m.glyph_w * m.glyph_h];
			assert!(r.rasterize('A', Style::Regular, &mut buf), "size {size}");
		}
	}

	/// Ёмкость страницы при максимальном размере: сцена «таблица всех глифов» (FR-27) помещается
	/// даже на минимальной странице 1024 px, иначе был бы регулярный откат GL → Software.
	#[test]
	fn page_at_max_font_size_still_holds_enough_glyphs() {
		let rasterizer = Rasterizer::new_at(repo_family(), GLYPH_PX_MAX).unwrap();
		let atlas = Atlas::new(rasterizer, PAGE_PX_MIN);
		assert!(
			atlas.capacity() >= 400,
			"page 1024 at {}px holds only {} glyphs",
			GLYPH_PX_MAX,
			atlas.capacity()
		);
	}
}

/// Пересборка атласа при смене размера (ADR-0005 п.7): страница переиспользуется, слоты заново.
mod atlas_resize {
	use super::*;

	#[test]
	fn set_rasterizer_keeps_the_page_but_invalidates_every_slot() {
		let mut atlas = small_atlas(PAGE_PX_MIN);
		let page_before = atlas.page_px();
		let slot = atlas.ensure('A', Style::Regular);
		assert!(atlas.is_ready(slot, Style::Regular));
		let resets_before = atlas.resets();

		let rasterizer = Rasterizer::new_at(repo_family(), 32).unwrap();
		let m = rasterizer.metrics();
		atlas.set_rasterizer(rasterizer);

		assert_eq!(atlas.page_px(), page_before, "страница не перевыделяется");
		assert_eq!(atlas.size_px(), 32);
		assert_eq!(atlas.glyph_w(), m.glyph_w);
		assert_eq!(atlas.glyph_h(), m.glyph_h);
		assert_eq!(atlas.resets(), resets_before + 1, "слоты недействительны");
		assert_eq!(atlas.slot_of('A'), None, "слоты начинаются заново");
		assert_eq!(atlas.used_slots(), 1, "только пустой слот 0");
		// Пиксели страницы обнулены: старый растр не должен просвечивать.
		assert!(atlas.cells().iter().all(|&v| v == 0));

		// Глифы растеризуются заново лениво и в новом размере.
		let new_slot = atlas.ensure('A', Style::Regular);
		assert!(new_slot >= 1);
		assert_eq!(
			atlas.glyph_bytes(new_slot, Style::Regular).len(),
			atlas.glyph_w() * atlas.glyph_h()
		);
		assert!(has_ink(&atlas.glyph_bytes(new_slot, Style::Regular)));
	}

	#[test]
	fn capacity_follows_the_new_cell_size() {
		let mut atlas = small_atlas(PAGE_PX_MIN);
		let at_16 = atlas.capacity();
		atlas.set_rasterizer(Rasterizer::new_at(repo_family(), 32).unwrap());
		let at_32 = atlas.capacity();
		assert!(at_32 < at_16, "крупная ячейка — меньше слотов");
		assert!(at_32 >= 400);
	}
}
