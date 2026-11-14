use super::REQUIREMENT_MARKER_SUFFIX;

const DEPENDENCY_TARGET_CLASS_LIFECYCLE_ID: &str = "lifecycle-id";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MarkerObservation {
    pub is_present: bool,
    pub is_valid: bool,
    pub diagnostics: Vec<String>,
}

pub(super) fn configured_marker(
    policy: &crate::verified::policy::Policy,
) -> Option<&crate::verified::policy::TaskMarker> {
    policy
        .task_marker_tokens
        .iter()
        .find(|marker| marker.id == policy.artifact_substance_policy.dependency_marker_id)
}

pub(super) fn observe_marker(
    line: &str,
    marker: &crate::verified::policy::TaskMarker,
    policy: &crate::verified::policy::Policy,
) -> MarkerObservation {
    let prefix = marker_prefix(&marker.token);
    let diagnostic_capacity = line.matches(prefix).count();
    let mut diagnostics = Vec::with_capacity(diagnostic_capacity);
    let mut rest = line;
    let mut is_present = false;
    let mut is_valid = false;
    while let Some(start) = rest.find(prefix) {
        is_present = true;
        let after_prefix = &rest[start + prefix.len()..];
        let Some(end) = after_prefix.find(REQUIREMENT_MARKER_SUFFIX) else {
            diagnostics.push(format!(
                "unclosed dependency marker beginning with `{prefix}`"
            ));
            break;
        };
        let target = after_prefix[..end].trim();
        observe_target(target, policy, &mut is_valid, &mut diagnostics);
        rest = &after_prefix[end + REQUIREMENT_MARKER_SUFFIX.len_utf8()..];
    }
    MarkerObservation {
        is_present,
        is_valid: is_valid && diagnostics.is_empty(),
        diagnostics,
    }
}

fn observe_target(
    target: &str,
    policy: &crate::verified::policy::Policy,
    is_valid: &mut bool,
    diagnostics: &mut Vec<String>,
) {
    if target.is_empty() {
        diagnostics.push("dependency target must not be empty".to_string());
    } else if policy.artifact_substance_policy.dependency_target_class
        != DEPENDENCY_TARGET_CLASS_LIFECYCLE_ID
    {
        diagnostics.push(format!(
            "unsupported dependency target class `{}`",
            policy.artifact_substance_policy.dependency_target_class
        ));
    } else if !valid_lifecycle_id(target) {
        diagnostics.push(format!(
            "dependency target `{target}` is not a valid lifecycle ID"
        ));
    } else {
        *is_valid = true;
    }
}

pub(super) fn marker_prefix(token: &str) -> &str {
    token.split('<').next().unwrap_or(token)
}

pub(crate) fn valid_lifecycle_id(candidate: &str) -> bool {
    let bytes = candidate.as_bytes();
    bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}
