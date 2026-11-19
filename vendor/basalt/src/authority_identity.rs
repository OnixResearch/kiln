//! Immutable schema identities shared by the authority core and release receipts.

/// Schema for pure UCAN/Basalt authority inputs.
pub const AUTHORITY_INPUT_SCHEMA: &str = "basalt.ucan-authority-input.v1";
/// Schema for pure UCAN/Basalt authority decisions.
pub const AUTHORITY_DECISION_SCHEMA: &str = "basalt.ucan-authority-decision.v1";
/// Schema for reviewed authority fixture cases.
pub const AUTHORITY_FIXTURE_SCHEMA: &str = "basalt.ucan-authority-fixture.v1";
/// Schema for the release verifier's authority-core receipt.
pub const AUTHORITY_RELEASE_RECEIPT_SCHEMA: &str = "basalt.ucan-authority-release-receipt.v1";
