#[cfg(any(target_arch = "wasm32", test))]
mod model;

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::{mount_index, select_history};
