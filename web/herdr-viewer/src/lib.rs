//! Read-only Herdr pane viewer: a pure snapshot core over Rio's terminal
//! state engine, and a thin browser shell that paints the cells on a canvas.

#[cfg(target_arch = "wasm32")]
mod browser;
pub mod grid;
