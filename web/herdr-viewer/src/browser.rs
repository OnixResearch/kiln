//! Browser shell: paints one snapshot's cells on a 2D canvas.
//!
//! Glyphs reach the page only through `fillText` and the canvas fallback
//! `textContent`; no captured byte is ever parsed as markup. The viewer
//! registers no input, pointer, clipboard or link handler.

use std::fmt::Write as _;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, Element, HtmlCanvasElement};

use crate::grid::{BACKGROUND, Cell, Rgb, Snapshot};

const FONT_PX: f64 = 13.0;
const LINE_HEIGHT: f64 = 1.25;
const FONT_FAMILY: &str = "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace";
/// Underline and strikeout thickness and positions, in CSS pixels / cell fractions.
const RULE_PX: f64 = 1.0;
const UNDERLINE_AT: f64 = 0.9;
const STRIKEOUT_AT: f64 = 0.55;
const MIN_PIXEL_RATIO: f64 = 1.0;
const MAX_PIXEL_RATIO: f64 = 3.0;
/// Cap canvas backing store dimensions so 500x500 grids cannot exceed browser 2D canvas limits.
const MAX_CANVAS_SIDE_PX: f64 = 16_384.0;
const MAX_CANVAS_AREA_PX: f64 = 67_108_864.0;

fn bounded_pixel_ratio(device_ratio: f64, width: f64, height: f64) -> f64 {
    let side_cap = MAX_CANVAS_SIDE_PX / width.max(height).max(1.0);
    let area_cap = (MAX_CANVAS_AREA_PX / (width * height).max(1.0)).sqrt();
    let cap = MAX_PIXEL_RATIO.min(side_cap).min(area_cap).max(MIN_PIXEL_RATIO);
    device_ratio.clamp(MIN_PIXEL_RATIO, cap)
}

fn failure(message: &str) -> JsError {
    JsError::new(message)
}

fn css(color: Rgb, out: &mut String) {
    out.clear();
    let _ = write!(out, "#{:02x}{:02x}{:02x}", color.r, color.g, color.b);
}

fn font(bold: bool, italic: bool) -> String {
    format!(
        "{}{}{FONT_PX}px {FONT_FAMILY}",
        if italic { "italic " } else { "" },
        if bold { "bold " } else { "" },
    )
}

/// One mounted canvas. Dropping it from JavaScript (`dispose`) removes it.
#[wasm_bindgen]
pub struct HerdrViewer {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
}

/// Append a read-only terminal canvas to `element`.
///
/// # Errors
/// Fails when the document or a 2D canvas context is unavailable, so the
/// caller can fall back to plain-text capture.
#[wasm_bindgen]
pub fn mount(element: &Element) -> Result<HerdrViewer, JsError> {
    let document = element
        .owner_document()
        .ok_or_else(|| failure("viewer element has no document"))?;
    let canvas: HtmlCanvasElement = document
        .create_element("canvas")
        .map_err(|_| failure("canvas unavailable"))?
        .dyn_into()
        .map_err(|_| failure("canvas unavailable"))?;
    let context: CanvasRenderingContext2d = canvas
        .get_context("2d")
        .map_err(|_| failure("2D canvas context unavailable"))?
        .ok_or_else(|| failure("2D canvas context unavailable"))?
        .dyn_into()
        .map_err(|_| failure("2D canvas context unavailable"))?;
    canvas
        .set_attribute("role", "img")
        .and_then(|()| canvas.set_attribute("aria-label", "Terminal snapshot"))
        .and_then(|()| element.append_child(&canvas).map(|_| ()))
        .map_err(|_| failure("viewer element refused the canvas"))?;
    Ok(HerdrViewer { canvas, context })
}

#[wasm_bindgen]
impl HerdrViewer {
    /// Replace the canvas with one snapshot. `columns`/`rows` are the pane's
    /// reported size, or 0 when unknown. Returns `true` when the viewer had
    /// to clip bytes or grid dimensions to stay within its bounds.
    ///
    /// # Errors
    /// Fails when the snapshot cannot be interpreted or the canvas refuses
    /// drawing.
    pub fn render(&self, bytes: &[u8], columns: u16, rows: u16) -> Result<bool, JsError> {
        let snapshot = Snapshot::parse(bytes, columns, rows).map_err(|error| failure(&error))?;
        let device_ratio = web_sys::window()
            .map_or(MIN_PIXEL_RATIO, |window| window.device_pixel_ratio());
        let context = &self.context;
        context.set_font(&font(false, false));
        let cell_width = context
            .measure_text("M")
            .map_err(|_| failure("canvas text metrics unavailable"))?
            .width();
        let cell_height = (FONT_PX * LINE_HEIGHT).ceil();
        let width = cell_width * f64::from(snapshot.columns);
        let height = cell_height * f64::from(snapshot.rows);
        let ratio = bounded_pixel_ratio(device_ratio, width, height);
        // Integral canvas sizes; the float-to-int cast saturates.
        self.canvas.set_width((width * ratio).ceil() as u32);
        self.canvas.set_height((height * ratio).ceil() as u32);
        let style = self.canvas.style();
        style
            .set_property("width", &format!("{width}px"))
            .and_then(|()| style.set_property("height", &format!("{height}px")))
            .map_err(|_| failure("canvas style unavailable"))?;
        context
            .set_transform(ratio, 0.0, 0.0, ratio, 0.0, 0.0)
            .map_err(|_| failure("canvas transform unavailable"))?;
        context.set_text_baseline("middle");
        let mut color = String::new();
        css(BACKGROUND, &mut color);
        context.set_fill_style_str(&color);
        context.fill_rect(0.0, 0.0, width, height);
        let geometry = Geometry {
            cell_width,
            cell_height,
        };
        for row in 0..snapshot.rows {
            paint_backgrounds(context, &snapshot, row, geometry, &mut color);
        }
        let mut current_font = (false, false);
        context.set_font(&font(false, false));
        let mut glyph = [0_u8; 4];
        for row in 0..snapshot.rows {
            for column in 0..snapshot.columns {
                let Some(cell) = snapshot.cell(row, column) else {
                    continue;
                };
                if cell.spacer {
                    continue;
                }
                let x = f64::from(column) * cell_width;
                let y = f64::from(row) * cell_height;
                let text = match &cell.cluster {
                    Some(cluster) => cluster.as_ref(),
                    None => cell.ch.encode_utf8(&mut glyph),
                };
                if text != " " {
                    if current_font != (cell.bold, cell.italic) {
                        current_font = (cell.bold, cell.italic);
                        context.set_font(&font(cell.bold, cell.italic));
                    }
                    css(cell.fg, &mut color);
                    context.set_fill_style_str(&color);
                    context
                        .fill_text(text, x, y + cell_height / 2.0)
                        .map_err(|_| failure("canvas refused text"))?;
                }
                paint_rules(context, cell, x, y, geometry, &mut color);
            }
        }
        // Fallback content for assistive technology, as text, never markup.
        self.canvas.set_text_content(Some(&snapshot.plain_text()));
        Ok(snapshot.clipped)
    }

    /// Remove the canvas; the JavaScript handle is released with it.
    pub fn dispose(self) {
        self.canvas.remove();
    }
}

#[derive(Clone, Copy)]
struct Geometry {
    cell_width: f64,
    cell_height: f64,
}

/// Fill runs of equal non-default background in one row.
fn paint_backgrounds(
    context: &CanvasRenderingContext2d,
    snapshot: &Snapshot,
    row: u16,
    geometry: Geometry,
    color: &mut String,
) {
    let mut column = 0;
    while column < snapshot.columns {
        let Some(bg) = snapshot.cell(row, column).map(|cell| cell.bg) else {
            break;
        };
        let start = column;
        while column < snapshot.columns
            && snapshot.cell(row, column).is_some_and(|cell| cell.bg == bg)
        {
            column += 1;
        }
        if bg != BACKGROUND {
            css(bg, color);
            context.set_fill_style_str(color);
            context.fill_rect(
                f64::from(start) * geometry.cell_width,
                f64::from(row) * geometry.cell_height,
                f64::from(column - start) * geometry.cell_width,
                geometry.cell_height,
            );
        }
    }
}

fn paint_rules(
    context: &CanvasRenderingContext2d,
    cell: &Cell,
    x: f64,
    y: f64,
    geometry: Geometry,
    color: &mut String,
) {
    if !cell.underline && !cell.strikeout {
        return;
    }
    let span = if cell.wide { 2.0 } else { 1.0 } * geometry.cell_width;
    css(cell.fg, color);
    context.set_fill_style_str(color);
    if cell.underline {
        context.fill_rect(x, y + geometry.cell_height * UNDERLINE_AT, span, RULE_PX);
    }
    if cell.strikeout {
        context.fill_rect(x, y + geometry.cell_height * STRIKEOUT_AT, span, RULE_PX);
    }
}
