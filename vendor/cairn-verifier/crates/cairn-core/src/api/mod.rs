//! Compatibility exports grouped by capability.

mod assurance;
pub use assurance::*;
mod collaboration;
pub use collaboration::*;
// Preserve the original explicit export over the document and workflow globs.
pub use collaboration::BASE_REVISION_KIND_GIT_OID;
mod evidence;
pub use evidence::*;
mod guidance;
pub use guidance::*;
mod lifecycle;
pub use lifecycle::*;
mod provenance;
pub use provenance::*;
