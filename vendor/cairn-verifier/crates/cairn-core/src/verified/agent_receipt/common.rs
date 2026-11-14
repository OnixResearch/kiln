use crate::verified::nominal_identity::{Blake3Digest, DigestRoleMarker, RAW_BLAKE3_PROFILE};

use super::types::AgentRunGateIssue;

pub(super) fn is_digest<Role: DigestRoleMarker>(candidate: &str) -> bool {
    Blake3Digest::<Role>::parse_compatible(candidate, RAW_BLAKE3_PROFILE, "agent_receipt.digest")
        .is_ok()
}

pub(super) fn issue(code: impl Into<String>, message: impl Into<String>) -> AgentRunGateIssue {
    AgentRunGateIssue {
        code: code.into(),
        message: message.into(),
    }
}
