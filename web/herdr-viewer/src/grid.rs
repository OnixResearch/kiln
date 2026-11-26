//! Pure snapshot core: Herdr's visible-screen ANSI bytes become a bounded,
//! read-only cell grid through Rio's terminal state engine.
//!
//! The bytes are untrusted pane output. They only ever drive terminal state
//! in a fresh, throwaway surface; nothing here produces markup. Every event
//! the terminal raises toward a host (replies to the child, clipboard writes,
//! titles, bells, notifications, close requests) goes to an inert delegate
//! and is dropped. Links, selection and images are never read back.

use std::sync::Arc;

use librio::{
    AnsiColor, Engine, NamedColor, RenderState, StyleFlags, SurfaceDelegate, SurfaceDesc, SurfaceId,
};

/// The largest snapshot interpreted; the Fleet capture route bounds to the same size.
pub const MAX_SNAPSHOT_BYTES: usize = 64 * 1_024;
/// Grid sides beyond these are clipped.
pub const MAX_COLUMNS: u16 = 500;
pub const MAX_ROWS: u16 = 500;
/// librio refuses a side smaller than two cells.
pub const MIN_SIDE: u16 = 2;
/// Width used when the pane size is unknown.
pub const DEFAULT_COLUMNS: u16 = 80;
/// Nominal cell pixels reported to the terminal (only for pixel-size queries).
const CELL_PIXEL_WIDTH: u16 = 8;
const CELL_PIXEL_HEIGHT: u16 = 16;
/// A snapshot keeps no history.
const SCROLLBACK_LINES: usize = 0;
/// CAN aborts any escape sequence a clipped snapshot left open; then end any
/// synchronized update so the grid is always readable.
const SNAPSHOT_END: &[u8] = b"\x18\x1b[?2026l";
/// Dim and faint colors keep this fraction of their intensity.
const DIM_NUMERATOR: u16 = 2;
const DIM_DENOMINATOR: u16 = 3;
/// Base of the xterm 6x6x6 color cube and its channel levels.
const CUBE_BASE: u8 = 16;
const CUBE_SIDE: u8 = 6;
const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
/// Base, start and step of the xterm grayscale ramp.
const GRAY_BASE: u8 = 232;
const GRAY_START: u8 = 8;
const GRAY_STEP: u8 = 10;
const WIDE: u8 = 1;
const SPACER: u8 = 2;
const LEADING_SPACER: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

const fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    Rgb { r, g, b }
}

pub const FOREGROUND: Rgb = rgb(0xe5, 0xe5, 0xe5);
pub const BACKGROUND: Rgb = rgb(0x14, 0x16, 0x1a);
const BRIGHT_FOREGROUND: Rgb = rgb(0xff, 0xff, 0xff);
/// The xterm default sixteen-color palette.
const ANSI: [Rgb; 16] = [
    rgb(0x00, 0x00, 0x00),
    rgb(0xcd, 0x00, 0x00),
    rgb(0x00, 0xcd, 0x00),
    rgb(0xcd, 0xcd, 0x00),
    rgb(0x00, 0x00, 0xee),
    rgb(0xcd, 0x00, 0xcd),
    rgb(0x00, 0xcd, 0xcd),
    rgb(0xe5, 0xe5, 0xe5),
    rgb(0x7f, 0x7f, 0x7f),
    rgb(0xff, 0x00, 0x00),
    rgb(0x00, 0xff, 0x00),
    rgb(0xff, 0xff, 0x00),
    rgb(0x5c, 0x5c, 0xff),
    rgb(0xff, 0x00, 0xff),
    rgb(0x00, 0xff, 0xff),
    rgb(0xff, 0xff, 0xff),
];

/// One rendered cell with its colors resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    /// The base character; a space for blank, hidden and spacer cells.
    pub ch: char,
    /// The whole grapheme cluster when the cell carries combining marks.
    pub cluster: Option<Box<str>>,
    pub fg: Rgb,
    pub bg: Rgb,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
    /// The first cell of a double-width character.
    pub wide: bool,
    /// The second half of a wide character; nothing is drawn here.
    pub spacer: bool,
}

impl Cell {
    fn blank() -> Self {
        Self {
            ch: ' ',
            cluster: None,
            fg: FOREGROUND,
            bg: BACKGROUND,
            bold: false,
            italic: false,
            underline: false,
            strikeout: false,
            wide: false,
            spacer: false,
        }
    }
}

/// A fixed, read-only grid of one visible-screen snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub columns: u16,
    pub rows: u16,
    /// Row-major, `columns * rows` cells.
    pub cells: Vec<Cell>,
    /// Bytes or sides beyond the viewer's bounds were dropped.
    pub clipped: bool,
}

/// Fit the grid: the reported size when known, never fewer rows than the
/// snapshot has lines, always within `MIN_SIDE..=MAX_*`.
#[must_use]
pub fn fit(columns: u16, rows: u16, bytes: &[u8]) -> (u16, u16, bool) {
    let lines = bytes.split(|byte| *byte == b'\n').count();
    let rows = rows.max(u16::try_from(lines).unwrap_or(u16::MAX));
    let columns = if columns == 0 {
        DEFAULT_COLUMNS
    } else {
        columns
    };
    let clipped = columns > MAX_COLUMNS || rows > MAX_ROWS;
    (
        columns.clamp(MIN_SIDE, MAX_COLUMNS),
        rows.clamp(MIN_SIDE, MAX_ROWS),
        clipped,
    )
}

/// Drops every host-bound terminal event: replies meant for the child,
/// clipboard writes, titles, bells, notifications and close requests.
struct Inert;

impl SurfaceDelegate for Inert {
    fn wakeup(&self, _surface: SurfaceId) {}
}

impl Snapshot {
    /// Interpret at most `MAX_SNAPSHOT_BYTES` of `bytes` in a fresh terminal
    /// of the fitted size and read back its visible cells.
    ///
    /// # Errors
    /// Returns librio's diagnostic when it cannot create the surface.
    pub fn parse(bytes: &[u8], columns: u16, rows: u16) -> Result<Self, String> {
        let bounded = &bytes[..bytes.len().min(MAX_SNAPSHOT_BYTES)];
        let (columns, rows, clipped_sides) = fit(columns, rows, bounded);
        let engine = Engine::new(Arc::new(Inert));
        let surface = engine
            .create_surface(&SurfaceDesc {
                cols: columns,
                rows,
                pixel_width: columns.saturating_mul(CELL_PIXEL_WIDTH),
                pixel_height: rows.saturating_mul(CELL_PIXEL_HEIGHT),
                scrollback: SCROLLBACK_LINES,
                ..SurfaceDesc::default()
            })
            .map_err(|error| error.to_string())?;
        surface.inject_output(bounded);
        surface.inject_output(SNAPSHOT_END);
        let mut state = RenderState::new(&surface);
        state.update();
        let width = usize::from(columns);
        let height = usize::from(rows);
        let mut cells = Vec::with_capacity(width * height);
        for line in 0..height {
            for column in 0..width {
                cells.push(read_cell(&state, line, column));
            }
        }
        Ok(Self {
            columns,
            rows,
            cells,
            clipped: clipped_sides || bounded.len() < bytes.len(),
        })
    }

    #[must_use]
    pub fn cell(&self, row: u16, column: u16) -> Option<&Cell> {
        if row >= self.rows || column >= self.columns {
            return None;
        }
        self.cells
            .get(usize::from(row) * usize::from(self.columns) + usize::from(column))
    }

    /// One row's characters with trailing blanks trimmed.
    #[must_use]
    pub fn row_text(&self, row: u16) -> String {
        let mut text = String::new();
        for column in 0..self.columns {
            let Some(cell) = self.cell(row, column) else {
                break;
            };
            if cell.spacer {
                continue;
            }
            match &cell.cluster {
                Some(cluster) => text.push_str(cluster),
                None => text.push(cell.ch),
            }
        }
        text.truncate(text.trim_end().len());
        text
    }

    /// Every row's text, for the canvas fallback content and assistive technology.
    #[must_use]
    pub fn plain_text(&self) -> String {
        let mut rows: Vec<String> = (0..self.rows).map(|row| self.row_text(row)).collect();
        while rows.last().is_some_and(String::is_empty) {
            rows.pop();
        }
        rows.join("\n")
    }
}

fn read_cell(state: &RenderState, line: usize, column: usize) -> Cell {
    let Some(square) = state.square(line, column) else {
        return Cell::blank();
    };
    let style = state.style_at(line, column, square);
    let flags = style.flags;
    let wide = square.wide() as u8;
    let mut fg = color(style.fg, true);
    let mut bg = color(style.bg, false);
    if flags.contains(StyleFlags::DIM) {
        fg = dim(fg);
    }
    if flags.contains(StyleFlags::INVERSE) {
        std::mem::swap(&mut fg, &mut bg);
    }
    let hidden = flags.contains(StyleFlags::HIDDEN);
    let spacer = matches!(wide, SPACER | LEADING_SPACER);
    let ch = match square.c() {
        _ if hidden || spacer => ' ',
        '\0' => ' ',
        ch if ch.is_control() => ' ',
        ch => ch,
    };
    let cluster = if hidden || spacer {
        None
    } else {
        state.cluster_of(square).map(|tail| {
            let mut text = String::with_capacity(4 * (1 + tail.len()));
            text.push(ch);
            text.extend(tail.iter().filter(|ch| !ch.is_control()));
            text.into_boxed_str()
        })
    };
    Cell {
        ch,
        cluster,
        fg,
        bg,
        bold: flags.contains(StyleFlags::BOLD),
        italic: flags.contains(StyleFlags::ITALIC),
        underline: flags.intersects(StyleFlags::ALL_UNDERLINES),
        strikeout: flags.contains(StyleFlags::STRIKEOUT),
        wide: wide == WIDE,
        spacer,
    }
}

/// Resolve a terminal color against Kiln's fixed palette. Palette changes a
/// program requests (OSC 4/10/11) are not applied: the snapshot only uses
/// the colors it names.
#[must_use]
pub fn color(color: AnsiColor, foreground: bool) -> Rgb {
    match color {
        AnsiColor::Spec(spec) => rgb(spec.r, spec.g, spec.b),
        AnsiColor::Indexed(index) => indexed(index),
        AnsiColor::Named(named) => named_color(named, foreground),
    }
}

fn named_color(named: NamedColor, foreground: bool) -> Rgb {
    use NamedColor as N;
    let ansi = |index: usize| ANSI[index];
    match named {
        N::Black => ansi(0),
        N::Red => ansi(1),
        N::Green => ansi(2),
        N::Yellow => ansi(3),
        N::Blue => ansi(4),
        N::Magenta => ansi(5),
        N::Cyan => ansi(6),
        N::White => ansi(7),
        N::LightBlack => ansi(8),
        N::LightRed => ansi(9),
        N::LightGreen => ansi(10),
        N::LightYellow => ansi(11),
        N::LightBlue => ansi(12),
        N::LightMagenta => ansi(13),
        N::LightCyan => ansi(14),
        N::LightWhite => ansi(15),
        N::Foreground | N::Cursor => FOREGROUND,
        N::Background => {
            if foreground {
                FOREGROUND
            } else {
                BACKGROUND
            }
        }
        N::DimBlack => dim(ansi(0)),
        N::DimRed => dim(ansi(1)),
        N::DimGreen => dim(ansi(2)),
        N::DimYellow => dim(ansi(3)),
        N::DimBlue => dim(ansi(4)),
        N::DimMagenta => dim(ansi(5)),
        N::DimCyan => dim(ansi(6)),
        N::DimWhite => dim(ansi(7)),
        N::LightForeground => BRIGHT_FOREGROUND,
        N::DimForeground => dim(FOREGROUND),
    }
}

/// xterm's 256-color table.
#[must_use]
pub fn indexed(index: u8) -> Rgb {
    if index < CUBE_BASE {
        return ANSI[usize::from(index)];
    }
    if index >= GRAY_BASE {
        let level = GRAY_START + (index - GRAY_BASE) * GRAY_STEP;
        return rgb(level, level, level);
    }
    let cube = index - CUBE_BASE;
    let level = |value: u8| CUBE_LEVELS[usize::from(value % CUBE_SIDE)];
    rgb(
        level(cube / (CUBE_SIDE * CUBE_SIDE)),
        level(cube / CUBE_SIDE),
        level(cube),
    )
}

fn dim(color: Rgb) -> Rgb {
    let scale = |channel: u8| {
        u8::try_from(u16::from(channel) * DIM_NUMERATOR / DIM_DENOMINATOR).unwrap_or(channel)
    };
    rgb(scale(color.r), scale(color.g), scale(color.b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(bytes: &[u8], columns: u16, rows: u16) -> Snapshot {
        match Snapshot::parse(bytes, columns, rows) {
            Ok(snapshot) => snapshot,
            Err(error) => panic!("snapshot must parse: {error}"),
        }
    }

    fn cell(snapshot: &Snapshot, row: u16, column: u16) -> &Cell {
        match snapshot.cell(row, column) {
            Some(cell) => cell,
            None => panic!("cell {row},{column} is outside the grid"),
        }
    }

    // r[verify kiln.sdlc.herdr_viewer.scenario.view]
    #[test]
    fn colored_cells_keep_sgr_colors_and_attributes() {
        let snapshot = parse(
            b"\x1b[31mred\x1b[0m \x1b[1;38;2;1;2;3;48;5;21mrgb\x1b[0m\r\n\x1b[7minv\x1b[0m",
            20,
            2,
        );
        assert_eq!(snapshot.row_text(0), "red rgb");
        assert_eq!(snapshot.row_text(1), "inv");
        assert_eq!(cell(&snapshot, 0, 0).fg, ANSI[1]);
        assert_eq!(cell(&snapshot, 0, 0).bg, BACKGROUND);
        let rgb_cell = cell(&snapshot, 0, 4);
        assert_eq!(rgb_cell.fg, rgb(1, 2, 3));
        assert_eq!(rgb_cell.bg, indexed(21));
        assert!(rgb_cell.bold);
        let inverse = cell(&snapshot, 1, 0);
        assert_eq!((inverse.fg, inverse.bg), (BACKGROUND, FOREGROUND));
        assert_eq!(cell(&snapshot, 1, 5), &Cell::blank());
    }

    // r[verify kiln.sdlc.herdr_viewer.scenario.inert]
    #[test]
    fn markup_and_script_text_stay_literal_cells() {
        let hostile = b"<script>alert(1)</script>\r\n<img src=x onerror=alert(2)>&amp;";
        let snapshot = parse(hostile, 40, 2);
        assert_eq!(snapshot.row_text(0), "<script>alert(1)</script>");
        assert_eq!(snapshot.row_text(1), "<img src=x onerror=alert(2)>&amp;");
        assert_eq!(cell(&snapshot, 0, 0).ch, '<');
        assert_eq!(
            snapshot.plain_text(),
            "<script>alert(1)</script>\n<img src=x onerror=alert(2)>&amp;"
        );
    }

    // r[verify kiln.sdlc.herdr_viewer.scenario.inert]
    #[test]
    fn unsupported_sequences_have_no_effect_outside_the_grid() {
        let hostile = concat!(
            "\x1b]0;stolen title\x07",
            "\x1b]52;c;aGVsbG8=\x07",
            "\x1b]8;;https://example.invalid/\x07link\x1b]8;;\x07",
            "\x1b[c\x1b[6n\x1b[>c",
            "\x1b]4;1;rgb:00/00/ff\x07",
            "\x1bPq#0;2;0;0;0#0~~@@vv@@~~$-\x1b\\",
            "\x1b_Ga=T,f=100;AAAA\x1b\\",
            "\x07",
            "\x1b[31mafter\x1b[0m",
        );
        let snapshot = parse(hostile.as_bytes(), 30, 2);
        assert_eq!(snapshot.row_text(0), "linkafter");
        assert_eq!(snapshot.row_text(1), "");
        assert_eq!(
            cell(&snapshot, 0, 4).fg,
            ANSI[1],
            "palette changes are not applied"
        );
        assert!(!snapshot.clipped);
    }

    #[test]
    fn open_sequences_and_synchronized_updates_cannot_blank_the_grid() {
        let snapshot = parse(b"\x1b[?2026hshown", 10, 2);
        assert_eq!(snapshot.row_text(0), "shown");
        let unterminated = parse(b"kept\x1b]0;never ends", 10, 2);
        assert_eq!(unterminated.row_text(0), "kept");
    }

    #[test]
    fn oversized_snapshots_are_clipped_to_bounds() {
        let mut bytes = b"top".to_vec();
        bytes.resize(MAX_SNAPSHOT_BYTES + 1_024, b'x');
        let snapshot = parse(&bytes, MAX_COLUMNS + 100, MAX_ROWS + 100);
        assert!(snapshot.clipped);
        assert_eq!((snapshot.columns, snapshot.rows), (MAX_COLUMNS, MAX_ROWS));
        assert_eq!(
            snapshot.cells.len(),
            usize::from(MAX_COLUMNS) * usize::from(MAX_ROWS)
        );
        let interpreted: usize = (0..snapshot.rows)
            .map(|row| snapshot.row_text(row).len())
            .sum();
        assert_eq!(interpreted, MAX_SNAPSHOT_BYTES);

        let lines = "row\r\n".repeat(usize::from(MAX_ROWS) + 10);
        let tall = parse(lines.as_bytes(), 10, 4);
        assert!(tall.clipped);
        assert_eq!(tall.rows, MAX_ROWS);
    }

    #[test]
    fn grid_fits_reported_size_and_snapshot_lines() {
        assert_eq!(fit(120, 40, b"a\r\nb"), (120, 40, false));
        assert_eq!(fit(0, 0, b"a\r\nb\r\nc"), (DEFAULT_COLUMNS, 3, false));
        assert_eq!(fit(1, 1, b""), (MIN_SIDE, MIN_SIDE, false));
        assert_eq!(fit(120, 2, b"1\n2\n3\n4"), (120, 4, false));
        let wide = parse("界x".as_bytes(), 10, 2);
        assert!(cell(&wide, 0, 0).wide);
        assert!(cell(&wide, 0, 1).spacer);
        assert_eq!(wide.row_text(0), "界x");
    }

    #[test]
    fn indexed_colors_follow_the_xterm_table() {
        assert_eq!(indexed(1), ANSI[1]);
        assert_eq!(indexed(16), rgb(0, 0, 0));
        assert_eq!(indexed(21), rgb(0, 0, 255));
        assert_eq!(indexed(196), rgb(255, 0, 0));
        assert_eq!(indexed(231), rgb(255, 255, 255));
        assert_eq!(indexed(232), rgb(8, 8, 8));
        assert_eq!(indexed(255), rgb(238, 238, 238));
    }
}
