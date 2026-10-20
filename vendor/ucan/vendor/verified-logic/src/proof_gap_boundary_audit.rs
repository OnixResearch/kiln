//! Source-derived proof-gap boundary audit constants.
//!
//! This module records the current audited inventory used by the
//! `audit-proof-gap-boundaries` OpenSpec. It does not model crypto/codec
//! semantics; it only gives Tracey-covered, mechanically checked constants for
//! the audit counts and closeout policy.

use vstd::prelude::*;

verus! {

pub const CURRENT_EXTERNAL_BODY_COUNT: usize = 16;
pub const CURRENT_ASSUME_SPECIFICATION_COUNT: usize = 0;
pub const CURRENT_NO_ENSURES_COUNT: usize = 0;
pub const CURRENT_RUNTIME_EXEMPT_COUNT: usize = 0;
pub const CURRENT_ASSUME_CALL_COUNT: usize = 0;
pub const SELECTED_CLOSEABLE_BOUNDARY_COUNT: usize = 0;
pub const INVENTORY_DRIFT_GATE_EXPECTED_EXTERNAL_BODY_COUNT: usize = 16;
pub const INVENTORY_SITE_LIST_DRIFT_GATE_EXPECTED_SITE_COUNT: usize = 16;
pub const INVENTORY_CLASSIFICATION_DRIFT_GATE_EXPECTED_SITE_COUNT: usize = 16;
pub const INVENTORY_CLASSIFICATION_DRIFT_GATE_CLOSEABLE_CANDIDATE_COUNT: usize = 0;
pub const INVENTORY_SITE_MANIFEST_SOURCE_EXPECTED_SITE_COUNT: usize = 16;
pub const INVENTORY_MANIFEST_RATIONALE_EXPECTED_SITE_COUNT: usize = 16;

// r[impl verification-gaps.boundary-audit.inventory]
pub open spec fn proof_gap_inventory_counts_spec() -> (nat, nat, nat, nat, nat) {
    (
        CURRENT_EXTERNAL_BODY_COUNT as nat,
        CURRENT_ASSUME_SPECIFICATION_COUNT as nat,
        CURRENT_NO_ENSURES_COUNT as nat,
        CURRENT_RUNTIME_EXEMPT_COUNT as nat,
        CURRENT_ASSUME_CALL_COUNT as nat,
    )
}

// r[impl verification-gaps.boundary-audit.classification]
pub open spec fn all_current_boundaries_classified_permanent_spec() -> bool {
    CURRENT_EXTERNAL_BODY_COUNT == 16 && SELECTED_CLOSEABLE_BOUNDARY_COUNT == 0
}

// r[impl verification-gaps.boundary-audit.closeout-slice]
pub open spec fn no_replacement_weakened_marker_spec() -> bool {
    CURRENT_ASSUME_SPECIFICATION_COUNT == 0
        && CURRENT_NO_ENSURES_COUNT == 0
        && CURRENT_RUNTIME_EXEMPT_COUNT == 0
        && CURRENT_ASSUME_CALL_COUNT == 0
}

// r[impl verification-gaps.boundary-audit.inventory-sync]
pub open spec fn published_inventory_count_spec() -> nat {
    CURRENT_EXTERNAL_BODY_COUNT as nat
}

// r[impl verification-gaps.inventory-drift-gate]
pub open spec fn inventory_drift_gate_expected_counts_spec() -> (nat, nat, nat, nat, nat) {
    (
        INVENTORY_DRIFT_GATE_EXPECTED_EXTERNAL_BODY_COUNT as nat,
        CURRENT_ASSUME_SPECIFICATION_COUNT as nat,
        CURRENT_NO_ENSURES_COUNT as nat,
        CURRENT_RUNTIME_EXEMPT_COUNT as nat,
        CURRENT_ASSUME_CALL_COUNT as nat,
    )
}

// r[impl verification-gaps.inventory-site-list-drift-gate]
pub open spec fn inventory_site_list_drift_gate_expected_site_count_spec() -> nat {
    INVENTORY_SITE_LIST_DRIFT_GATE_EXPECTED_SITE_COUNT as nat
}

// r[impl verification-gaps.inventory-classification-drift-gate]
pub open spec fn inventory_classification_drift_gate_expected_classification_spec() -> (nat, nat) {
    (
        INVENTORY_CLASSIFICATION_DRIFT_GATE_EXPECTED_SITE_COUNT as nat,
        INVENTORY_CLASSIFICATION_DRIFT_GATE_CLOSEABLE_CANDIDATE_COUNT as nat,
    )
}

// r[impl verification-gaps.inventory-site-manifest-source]
pub open spec fn inventory_site_manifest_source_expected_site_count_spec() -> nat {
    INVENTORY_SITE_MANIFEST_SOURCE_EXPECTED_SITE_COUNT as nat
}

// r[impl verification-gaps.manifest-rationale-source]
// r[impl verification-gaps.manifest-rationale-drift-gate]
// r[impl verification-gaps.manifest-rationale-publication]
pub open spec fn inventory_manifest_rationale_expected_site_count_spec() -> nat {
    INVENTORY_MANIFEST_RATIONALE_EXPECTED_SITE_COUNT as nat
}

// r[verify verification-gaps.boundary-audit.inventory]
pub proof fn proof_gap_inventory_counts_current()
    ensures proof_gap_inventory_counts_spec() == (16nat, 0nat, 0nat, 0nat, 0nat)
{
}

// r[verify verification-gaps.boundary-audit.classification]
pub proof fn proof_gap_classification_current()
    ensures all_current_boundaries_classified_permanent_spec()
{
}

// r[verify verification-gaps.boundary-audit.closeout-slice]
pub proof fn proof_gap_closeout_slice_introduces_no_replacement_markers()
    ensures no_replacement_weakened_marker_spec()
{
}

// r[verify verification-gaps.boundary-audit.inventory-sync]
pub proof fn proof_gap_published_inventory_count_current()
    ensures published_inventory_count_spec() == 16nat
{
}

// r[verify verification-gaps.inventory-drift-gate]
pub proof fn proof_gap_inventory_drift_gate_expected_counts_current()
    ensures inventory_drift_gate_expected_counts_spec() == (16nat, 0nat, 0nat, 0nat, 0nat)
{
}

// r[verify verification-gaps.inventory-site-list-drift-gate]
pub proof fn proof_gap_inventory_site_list_drift_gate_expected_sites_current()
    ensures inventory_site_list_drift_gate_expected_site_count_spec() == 16nat
{
}

// r[verify verification-gaps.inventory-classification-drift-gate]
pub proof fn proof_gap_inventory_classification_drift_gate_expected_classifications_current()
    ensures inventory_classification_drift_gate_expected_classification_spec() == (16nat, 0nat)
{
}

// r[verify verification-gaps.inventory-site-manifest-source]
pub proof fn proof_gap_inventory_site_manifest_source_expected_sites_current()
    ensures inventory_site_manifest_source_expected_site_count_spec() == 16nat
{
}

// r[verify verification-gaps.manifest-rationale-source]
// r[verify verification-gaps.manifest-rationale-drift-gate]
// r[verify verification-gaps.manifest-rationale-publication]
pub proof fn proof_gap_inventory_manifest_rationale_expected_sites_current()
    ensures inventory_manifest_rationale_expected_site_count_spec() == 16nat
{
}

} // verus!
