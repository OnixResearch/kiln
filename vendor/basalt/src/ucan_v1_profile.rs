//! Final UCAN `1.0.0` application-policy adapter.
//!
//! UCAN owns wire, signature, proof-chain, policy, revocation-observation, and
//! replay semantics. Basalt owns the closed command mapping and application
//! policy. The replay gate finishes all Basalt work before it calls the final
//! caller-owned replay port.
// r[impl basalt.ucan_token_flow.v1_profile.upstream]
// r[impl basalt.ucan_token_flow.v1_profile.adapter]
// r[impl basalt.ucan_token_flow.v1_profile.mapping]
// r[impl basalt.ucan_token_flow.v1_profile.authority]
// r[impl basalt.ucan_token_flow.v1_profile.receipt]

mod mapping;
mod receipt;
mod shell;

pub use mapping::{
    FINAL_PROFILE_MAPPING_JSON, FinalProfileMapping, FinalProfileMappingError,
    parse_final_profile_mapping,
};
pub use receipt::{FINAL_PROFILE_NON_CLAIMS, FinalProfileReason, FinalProfileReceipt};
pub use shell::{
    FinalProfileEnforcementRequest, FinalProfileFailure, LiveFinalProfileEnforcement,
    ResourceOwnerError, ResourceOwnerVerifier, verify_final_profile_enforcement,
};
