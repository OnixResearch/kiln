#![forbid(unsafe_code)]

mod coordinator;
mod journal;
pub mod project;
mod status;
mod storage;

pub use coordinator::{CiProfile, CiService};
pub use journal::CiJournal;
pub type Result<T, E = String> = std::result::Result<T, E>;
