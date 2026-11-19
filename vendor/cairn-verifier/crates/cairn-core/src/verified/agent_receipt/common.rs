pub(super) fn is_digest<Role: crate::verified::nominal_identity::DigestRoleMarker>(
    candidate: &str,
) -> bool {
    crate::verified::nominal_identity::Blake3Digest::<Role>::parse_compatible(
        candidate,
        crate::verified::nominal_identity::RAW_BLAKE3_PROFILE,
        "agent_receipt.digest",
    )
    .is_ok()
}

pub(super) fn issue(
    code: impl Into<String>,
    message: impl Into<String>,
) -> super::types::AgentRunGateIssue {
    super::types::AgentRunGateIssue {
        code: code.into(),
        message: message.into(),
    }
}
