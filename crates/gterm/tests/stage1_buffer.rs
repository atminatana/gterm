use gterm::buffer::{DEFAULT_BACKGROUND, DEFAULT_BG, DEFAULT_FG, ScreenBuffer};
use gterm::cell::{Cell, CellFlags, Color};

#[test]
fn new_buffer_contains_blank_cells_with_default_colors() {
	let screen = ScreenBuffer::new(3, 2);

	assert_eq!(screen.cols(), 3);
	assert_eq!(screen.rows(), 2);
	assert_eq!(
		screen.cell(1, 1),
		Cell::new('\0', DEFAULT_FG, DEFAULT_BG, CellFlags::empty())
	);
}

#[test]
fn custom_default_colors_fill_blank_cells_after_clear_scroll_and_resize() {
	let fg = Color::from_rgb(1, 2, 3);
	let bg = Color::from_rgb(4, 5, 6);
	let mut screen = ScreenBuffer::with_default_colors(2, 2, fg, bg);
	let blank = Cell::new('\0', fg, bg, CellFlags::empty());
	assert_eq!(screen.default_colors(), (fg, bg));
	assert_eq!(screen.cell(0, 0), blank);

	screen.write_row(0, "ab", Color::WHITE, Color::WHITE);
	screen.write_row(1, "cd", Color::WHITE, Color::WHITE);
	screen.scroll_up(1);
	assert_eq!(screen.cell(0, 1), blank);

	screen.clear_region(0, 0, 1, 1);
	assert_eq!(screen.cell(0, 0), blank);

	screen.resize(3, 3);
	assert_eq!(screen.cell(2, 2), blank);

	screen.clear_all();
	assert_eq!(screen.cell(1, 0), blank);
}

#[test]
fn writes_cells_rows_and_regions() {
	let mut screen = ScreenBuffer::new(5, 3);
	let fg = Color::from_rgba(10, 20, 30, 255);
	let bg = Color::from_rgba(40, 50, 60, 255);
	let flags = CellFlags::BOLD | CellFlags::UNDERLINE;

	screen.set_cell(1, 1, 'A', fg, bg, flags);
	screen.write_row(0, "hi", fg, bg);
	screen.fill_rect(2, 2, 2, 1, 'X', bg, fg, CellFlags::empty());

	assert_eq!(screen.cell(0, 0).ch, 'h');
	assert_eq!(screen.cell(1, 0).ch, 'i');
	assert_eq!(screen.cell(1, 1).ch, 'A');
	assert!(screen.cell(1, 1).flags.contains(flags));
	assert_eq!(screen.cell(2, 2).bg, fg);
}

#[test]
fn scroll_and_clear_operations_preserve_grid_shape() {
	let mut screen = ScreenBuffer::new(3, 3);
	screen.write_row(0, "abc", Color::BLACK, Color::BLACK);
	screen.write_row(1, "def", Color::BLACK, Color::BLACK);
	screen.write_row(2, "ghi", Color::BLACK, Color::BLACK);

	// scroll_up(1) pushes row 0 off; rows shift up
	screen.scroll_up(1);
	assert_eq!(screen.cell(0, 0).ch, 'd');
	assert_eq!(screen.cell(0, 2).ch, '\0'); // new bottom row is blank
	assert_eq!(screen.cols(), 3);
	assert_eq!(screen.rows(), 3);

	// scroll_down(1) shifts back down (original row 0 "abc" is lost)
	// scroll_down(1) shifts down from the current state after scroll_up
	screen.scroll_down(1);
	assert_eq!(screen.cell(0, 0).ch, '\0'); // row 0 is blank (was pushed off)
	assert_eq!(screen.cell(0, 1).ch, 'd'); // "def" from old row 0 → new row 1
	assert_eq!(screen.cell(0, 2).ch, 'g'); // "ghi" from old row 1 → new row 2
	assert_eq!(screen.cols(), 3);
	assert_eq!(screen.rows(), 3);

	// clear_region works independently
	let mut screen2 = ScreenBuffer::new(3, 3);
	screen2.write_row(1, "def", Color::BLACK, Color::BLACK);
	screen2.clear_region(1, 1, 2, 1);
	assert_eq!(screen2.cell(1, 1).ch, '\0');
	assert_eq!(screen2.cell(2, 1).ch, '\0');
	assert_eq!(screen2.cols(), 3);
	assert_eq!(screen2.rows(), 3);
}

#[test]
fn resize_preserves_the_intersecting_top_left_region() {
	let mut screen = ScreenBuffer::new(4, 3);
	screen.write_row(0, "abcd", Color::BLACK, Color::BLACK);
	screen.write_row(1, "efgh", Color::BLACK, Color::BLACK);

	screen.resize(2, 2);

	assert_eq!(screen.cols(), 2);
	assert_eq!(screen.rows(), 2);
	assert_eq!(screen.cell(0, 0).ch, 'a');
	assert_eq!(screen.cell(1, 0).ch, 'b');
	assert_eq!(screen.cell(0, 1).ch, 'e');
	assert_eq!(screen.cell(1, 1).ch, 'f');
}

#[test]
fn copy_region_moves_a_rectangle() {
	let mut screen = ScreenBuffer::new(4, 3);
	screen.fill_rect(
		0,
		0,
		2,
		2,
		'7',
		Color::BLACK,
		Color::BLACK,
		CellFlags::empty(),
	);

	screen.copy_region(0, 0, 2, 2, 2, 1);

	// Копирование с (0,0)→(2,1): cells (0,0),(1,0),(0,1),(1,1) копируются на позиции (2,1),(3,1),(2,2),(3,2)
	assert_eq!(screen.cell(2, 1).ch, '7'); // (0,0) → (2,1)
	assert_eq!(screen.cell(3, 1).ch, '7'); // (1,0) → (3,1)
	assert_eq!(screen.cell(2, 2).ch, '7'); // (0,1) → (2,2)
	assert_eq!(screen.cell(3, 2).ch, '7'); // (1,1) → (3,2)
}

#[test]
fn tracks_only_changed_rows_and_resize_marks_every_row_dirty() {
	let mut screen = ScreenBuffer::new(4, 3);
	screen.set_cell(2, 1, '9', Color::BLACK, Color::BLACK, CellFlags::empty());

	assert_eq!(
		screen.dirty_rows().iter().copied().filter(|&d| d).count(),
		1
	);

	screen.set_cell(2, 1, 'A', Color::BLACK, Color::BLACK, CellFlags::empty());
	assert_eq!(
		screen.dirty_rows().iter().copied().filter(|&d| d).count(),
		1
	);

	screen.clear_dirty_rows();
	assert!(screen.dirty_rows().iter().all(|&d| !d));
	screen.resize(3, 2);
	assert_eq!(
		screen.dirty_rows().iter().copied().filter(|&d| d).count(),
		2
	);
}

#[test]
fn cell_pack_and_unpack_are_round_trip_exact() {
	// Кодовая точка вне BMP: R несёт её целиком, флаги — в A (ADR-0009 п.4).
	let cell = Cell::new(
		'\u{F0001}',
		Color::from_rgba(1, 2, 3, 4),
		Color::from_rgba(5, 6, 7, 8),
		CellFlags::BOLD | CellFlags::ITALIC | CellFlags::UNDERLINE,
	);

	let packed = cell.to_packed();
	let unpacked = Cell::from_packed(&packed);

	assert_eq!(unpacked, cell);
	assert_eq!(packed[0], 0xF0001);
	assert_eq!(packed[3] as u16, cell.flags.bits());
	// Альфа цветов переживает упаковку (ADR-0013).
	assert_eq!(packed[1], 0x0401_0203);
	assert_eq!(unpacked.bg.a(), 8);
}

/// Кодировка `0xAARRGGBB` (FR-2, ADR-0013): `from_rgb` непрозрачен, `from_rgba` хранит альфу.
#[test]
fn color_encodes_alpha_in_the_top_byte() {
	let opaque = Color::from_rgb(0x12, 0x34, 0x56);
	assert_eq!(opaque, Color(0xFF12_3456));
	assert_eq!(opaque.a(), 255);

	let half = Color::from_rgba(0x12, 0x34, 0x56, 0x80);
	assert_eq!(half, Color(0x8012_3456));
	assert_eq!(
		(half.a(), half.r(), half.g(), half.b()),
		(0x80, 0x12, 0x34, 0x56)
	);

	for named in [
		Color::BLACK,
		Color::WHITE,
		Color::GREEN,
		Color::MAGENTA,
		Color::CYAN,
	] {
		assert_eq!(named.a(), 255, "{named:?}");
	}
	assert_eq!(Color::TRANSPARENT, Color::default());
	assert_eq!(Color::TRANSPARENT.a(), 0);
	// Пустая ячейка по умолчанию прозрачна, а буфер заполняет пустые места непрозрачными цветами.
	assert_eq!(Cell::default().bg.a(), 0);
	assert_eq!(ScreenBuffer::new(1, 1).cell(0, 0).bg.a(), 255);
}

/// Фон буфера (FR-2.1): по умолчанию чёрный, меняется без затрагивания ячеек и dirty-строк.
#[test]
fn buffer_background_defaults_to_black_and_can_be_changed() {
	let mut screen = ScreenBuffer::new(2, 2);
	assert_eq!(screen.background(), DEFAULT_BACKGROUND);
	assert_eq!(DEFAULT_BACKGROUND, Color::BLACK);

	screen.clear_dirty_rows();
	let before = screen.cell(1, 1);
	let blue = Color::from_rgb(0, 0, 0x80);
	screen.set_background(blue);
	assert_eq!(screen.background(), blue);
	assert_eq!(screen.cell(1, 1), before);
	assert!(screen.dirty_rows().iter().all(|&d| !d));
}

#[test]
fn unpack_of_invalid_codepoint_gives_blank_char() {
	for invalid in [0xD800, 0x11_0000] {
		let cell = Cell::from_packed(&[invalid, 0, 0, 0]);
		assert_eq!(cell.ch, '\0', "U+{invalid:X}");
	}
}

#[test]
fn write_row_keeps_codepoints_outside_bmp() {
	// Раньше `write_row` обрезал `char` до u16: U+F0001 становился U+0001.
	let mut screen = ScreenBuffer::new(4, 1);
	screen.write_row(0, "a\u{F0001}\u{10FFFD}", Color::WHITE, Color::BLACK);

	assert_eq!(screen.cell(0, 0).ch, 'a');
	assert_eq!(screen.cell(1, 0).ch, '\u{F0001}');
	assert_eq!(screen.cell(2, 0).ch, '\u{10FFFD}');
}

/// Порядок битов флагов — контракт с шейдером: константы `F_*` в `cell.frag` те же.
#[test]
fn flag_bits_are_bold_italic_underline_from_zero() {
	assert_eq!(CellFlags::BOLD.bits(), 1 << 0);
	assert_eq!(CellFlags::ITALIC.bits(), 1 << 1);
	assert_eq!(CellFlags::UNDERLINE.bits(), 1 << 2);
	assert_eq!(CellFlags::all().bits(), 0b111);

	let shader = include_str!("../src/renderer/shaders/cell.frag");
	for constant in ["F_BOLD = 1u;", "F_ITALIC = 2u;", "F_UNDERLINE = 4u;"] {
		assert!(shader.contains(constant), "в cell.frag нет {constant}");
	}
}
