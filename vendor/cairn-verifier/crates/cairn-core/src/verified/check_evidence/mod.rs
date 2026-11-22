//! Cairn-owned evidence production decisions over supplied facts.
//! Admission is not execution, authority, freshness observation, or publication.
mod admission;
mod cargo_output;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "the published cargo_coverage name remains available while the implementation avoids repeating its cargo_output module segment"
    )
)]
pub use cargo_output::coverage as cargo_coverage;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "the published CargoCoverage name remains available while the implementation avoids repeating its cargo_output module segment"
    )
)]
pub use cargo_output::Coverage as CargoCoverage;
pub use cargo_output::{coverage, Coverage};
mod kiln;
pub use kiln::{admit_kiln_json, KilnAdmission, KILN_REVISION};
mod lattice;
mod manifest;
mod normalization;
pub use lattice::{admit_lattice_json, LatticeAdmission, LATTICE_REVISION};
pub use manifest::{
    admit_manifest, recovery_decision, RecoveryDecision, MAX_BUNDLE_ARTIFACTS,
    MAX_BUNDLE_ARTIFACT_BYTES, MAX_BUNDLE_BYTES,
};
mod octet;
mod types;
mod valence;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "the published admit_octet_json name remains available while the implementation avoids repeating its octet module segment"
    )
)]
pub use octet::admit_json as admit_octet_json;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "the published OctetAdmission name remains available while the implementation avoids repeating its octet module segment"
    )
)]
pub use octet::Admission as OctetAdmission;
#[cfg_attr(
    dylint_lib = "octet",
    allow(
        renamed_imports,
        reason = "the published OctetStatus name remains available while the implementation avoids repeating its octet module segment"
    )
)]
pub use octet::Status as OctetStatus;
pub use octet::{admit_json, Admission, Status, OCTET_REVISION};
pub use valence::{admit_valence_json, ValenceAdmission, VALENCE_REVISION};

pub use admission::{admit_plan, authorize_attempt, profile_command, valid_applicability};
pub use normalization::{normalize_attempt, publication_decision, whole_subject_evidence_current};
pub use types::*;

#[cfg(test)]
mod kiln_tests;
#[cfg(test)]
mod manifest_tests;
#[cfg(test)]
mod tests;
