//! Local operator dashboard over admitted Kiln CI state.
//!
//! The dashboard is a read-only rendering shell over the accepted
//! `kiln-observe-core` projection. It adds no view vocabulary, no projection
//! logic, no control intake, and no canonical state. Every reported fact comes
//! from the accepted observation projection over an admitted journal state.

pub mod journal;
pub mod profile;
pub mod render;
pub mod report;
pub mod shell;
pub mod store;

pub use journal::{
    AdmittedJournal, EventIdentityConvention, JournalDocument, JournalFacts, MAX_JOURNAL_BYTES,
    MAX_JOURNAL_RECORDS,
};
pub use profile::{
    DASHBOARD_NON_CLAIMS, DASHBOARD_PROFILE_SCHEMA_VERSION, DashboardProfile, ProfileDocument,
};
pub use render::{RenderStyle, render_report};
pub use report::{DashboardReport, DashboardSelection, DashboardViews, RunPages, build_report};
pub use shell::{Input, Options, Parsed, parse_options, run_from_env};
pub use store::{STORE_JOURNAL_SCHEMA, admit_store, parse_reference};
