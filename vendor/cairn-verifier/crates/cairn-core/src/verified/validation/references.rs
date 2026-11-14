use crate::verified::policy::Policy;

use super::gates::{gate_issue, GateArtifact, GateIssue, GateSnapshot, GateStage};

pub(super) fn reference_hygiene_issues(
    snapshot: &GateSnapshot,
    stage: GateStage,
    policy: &Policy,
) -> Vec<GateIssue> {
    if snapshot.delta_specs.is_empty() {
        return Vec::new();
    }
    let known = known_delta_references(&snapshot.delta_specs);
    let mut artifacts = Vec::with_capacity(3);
    if let Some(artifact) = &snapshot.proposal {
        artifacts.push(artifact);
    }
    if matches!(stage, GateStage::Design | GateStage::Tasks) {
        if let Some(artifact) = &snapshot.design {
            artifacts.push(artifact);
        }
    }
    if matches!(stage, GateStage::Tasks) {
        if let Some(artifact) = &snapshot.tasks {
            artifacts.push(artifact);
        }
    }
    let mut issues = Vec::with_capacity(artifacts.len().saturating_mul(2).saturating_add(1));
    for artifact in artifacts {
        issues.extend(unknown_reference_issues(artifact, &known, policy));
        issues.extend(malformed_exemption_issues(artifact, policy));
    }
    if matches!(stage, GateStage::Tasks) {
        if let Some(tasks) = &snapshot.tasks {
            issues.extend(unlinked_task_issues(tasks, &known, policy));
        }
    }
    issues
}

fn known_delta_references(delta_specs: &[GateArtifact]) -> Vec<String> {
    let marker_bound = delta_specs
        .iter()
        .map(|artifact| reference_marker_bound(&artifact.markdown))
        .sum();
    let mut refs = Vec::with_capacity(marker_bound);
    for artifact in delta_specs {
        refs.extend(reference_markers(&artifact.markdown));
    }
    refs.sort();
    refs.dedup();
    refs
}

fn unknown_reference_issues(
    artifact: &GateArtifact,
    known: &[String],
    policy: &Policy,
) -> Vec<GateIssue> {
    reference_markers(&artifact.markdown)
        .into_iter()
        .filter(|reference| !is_external_trace_reference(reference) && !known.contains(reference))
        .map(|reference| {
            gate_issue(
                format!(
                    "{}: unknown requirement reference r[{reference}]",
                    artifact.path
                ),
                policy,
            )
        })
        .collect()
}

fn malformed_exemption_issues(artifact: &GateArtifact, policy: &Policy) -> Vec<GateIssue> {
    let mut issues = Vec::with_capacity(artifact.markdown.lines().count());
    for (line_number, line) in artifact.markdown.lines().enumerate() {
        for marker in exemption_markers(line) {
            if exemption_marker_is_valid(marker, policy) {
                continue;
            }
            issues.push(gate_issue(
                format!(
                    "{}:{}: malformed gate exemption; expected gate-exempt[{}]",
                    artifact.path,
                    line_number + 1,
                    exemption_marker_hint(policy)
                ),
                policy,
            ));
        }
    }
    issues
}

fn exemption_markers(line: &str) -> Vec<&str> {
    let marker_bound = line.matches("gate-exempt[").count();
    let mut markers = Vec::with_capacity(marker_bound);
    let mut rest = line;
    while let Some(start) = rest.find("gate-exempt[") {
        let after = &rest[start + "gate-exempt[".len()..];
        let Some(end) = after.find(']') else {
            markers.push(after);
            break;
        };
        markers.push(&after[..end]);
        rest = &after[end + 1..];
    }
    markers
}

fn exemption_marker_is_valid(marker: &str, policy: &Policy) -> bool {
    let fields = marker_fields(marker);
    if !policy
        .exemption_marker_policy
        .required_fields
        .iter()
        .all(|field| {
            fields
                .iter()
                .any(|(name, value)| name == field && !value.is_empty())
        })
    {
        return false;
    }
    policy
        .exemption_marker_policy
        .date_fields
        .iter()
        .all(|field| {
            fields
                .iter()
                .any(|(name, value)| name == field && is_iso_date(value))
        })
}

fn marker_fields(marker: &str) -> Vec<(&str, &str)> {
    marker
        .split_whitespace()
        .filter_map(|part| part.split_once('='))
        .collect()
}

fn is_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[8..].iter().all(u8::is_ascii_digit)
}

fn exemption_marker_hint(policy: &Policy) -> String {
    policy
        .exemption_marker_policy
        .required_fields
        .iter()
        .map(|field| format!("{field}=..."))
        .collect::<Vec<_>>()
        .join(" ")
}

fn unlinked_task_issues(
    artifact: &GateArtifact,
    known: &[String],
    policy: &Policy,
) -> Vec<GateIssue> {
    let mut issues = Vec::with_capacity(artifact.markdown.lines().count());
    for (line_number, line) in artifact.markdown.lines().enumerate() {
        let trimmed = line.trim_start();
        if !is_checkbox_task(trimmed) || has_valid_dependency_marker(trimmed, policy) {
            continue;
        }
        let refs = reference_markers(trimmed);
        if refs.iter().any(|reference| known.contains(reference)) {
            continue;
        }
        issues.push(gate_issue(
            format!(
                "{}:{}: task missing known requirement reference",
                artifact.path,
                line_number + 1
            ),
            policy,
        ));
    }
    issues
}

fn is_checkbox_task(trimmed: &str) -> bool {
    trimmed.starts_with("- [ ]")
        || trimmed.starts_with("- [x]")
        || trimmed.starts_with("- [X]")
        || trimmed.starts_with("- [~]")
}

fn has_valid_dependency_marker(trimmed: &str, policy: &Policy) -> bool {
    policy.task_marker_tokens.iter().any(|marker| {
        if marker.id != policy.artifact_substance_policy.dependency_marker_id {
            return false;
        }
        match marker.kind.as_str() {
            "prefix" | "depends-prefix" => {
                valid_dependency_target(trimmed, dependency_marker_prefix(&marker.token))
            }
            "literal" => trimmed.contains(&marker.token),
            _ => false,
        }
    })
}

fn valid_dependency_target(line: &str, prefix: &str) -> bool {
    let Some(start) = line.find(prefix) else {
        return false;
    };
    let after_prefix = &line[start + prefix.len()..];
    let Some(end) = after_prefix.find(']') else {
        return false;
    };
    let target = after_prefix[..end].trim();
    let bytes = target.as_bytes();
    bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn dependency_marker_prefix(token: &str) -> &str {
    token.split('<').next().unwrap_or(token)
}

fn is_external_trace_reference(reference: &str) -> bool {
    reference.starts_with("impl ")
        || reference.starts_with("verify ")
        || reference.starts_with("depends ")
}

fn reference_markers(markdown: &str) -> Vec<String> {
    let mut refs = Vec::with_capacity(reference_marker_bound(markdown));
    let mut rest = markdown;
    while let Some(start) = rest.find("r[") {
        let after = &rest[start + 2..];
        let Some(end) = after.find(']') else {
            break;
        };
        let reference = after[..end].trim();
        if !reference.is_empty() && reference != "..." {
            refs.push(reference.to_string());
        }
        rest = &after[end + 1..];
    }
    refs
}

fn reference_marker_bound(markdown: &str) -> usize {
    markdown.matches("r[").count()
}
