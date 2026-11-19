//! Immutable identities shared by UCAN semantics replay and release receipts.

/// Schema accepted by the UCAN semantics fixture adapter.
pub const UCAN_SEMANTICS_FIXTURE_SCHEMA: &str = "basalt.ucan-semantics-fixture.v2";
/// Protocol profile selected by the current compact adapter.
pub const UCAN_PROTOCOL_PROFILE: &str = "onix-compact-ed25519-v1";
/// Reviewed UCAN revision in Basalt's validated vendored source boundary.
pub const UCAN_RUNTIME_REVISION: &str = "417f74662b9de2de3bdabe93ad5db996e1bd2861";
/// Reviewed UCAN revision that produced the bound support catalog.
pub const UCAN_SUPPORT_CATALOG_REVISION: &str = "417f74662b9de2de3bdabe93ad5db996e1bd2861";
/// BLAKE3 identity of the reviewed UCAN integration support catalog.
pub const UCAN_SUPPORT_CATALOG_BLAKE3: &str =
    "fe77fd63076eaef555b74e702e4c14e483bdba70151ccc955c75b17d97412427";
/// BLAKE3 identity of Basalt's checked-in generated policy artifact.
pub const BASALT_POLICY_BLAKE3: &str =
    "fd17db568eb24222d4ca65fe45f02edb21ef910fad214a392441d62c07ef4b14";
