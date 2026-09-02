// Octet: the stable wire error vocabulary uses one exhaustive public code set and a fixed safe
// path.
#![allow(
    unknown_lints,
    missing_const_fn,
    path_segment_repetition,
    reason = "version-one public error names are retained for independent consumer compatibility; Display cannot be const"
)]

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    SchemaMismatch,
    EmptyCapability,
    CapabilityOrder,
    LimitInvalid,
    ProfileMismatch,
    UnsupportedPlatform,
    UnsupportedNodeKind,
    UnsupportedEffectClass,
    UnsupportedAdapterKind,
    UnsupportedControl,
    GraphEmpty,
    GraphLimit,
    GraphOrder,
    GraphDuplicate,
    GraphReference,
    GraphCycle,
    GraphIdentityMismatch,
    RequestIdentityMismatch,
    AcceptedLinkageMismatch,
    ObservationIdentityMismatch,
    ObservationLinkageMismatch,
    ObservationOrder,
    ObservationNodeMismatch,
    ObservationStateMismatch,
    ObservationLimit,
    ControlLinkageMismatch,
    LogReadLinkageMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractError {
    pub code: ErrorCode,
    pub path: &'static str,
}

impl core::fmt::Display for ContractError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "contract error {:?} at {}", self.code, self.path)
    }
}
