use std::collections::BTreeSet;

use crate::verified::{policy::ArtifactSubstancePolicy, repo::SpecSource};

use super::markdown::{
    has_requirement_strength, has_scenario_clause, markdown_heading, next_heading_at_or_above,
};

const REQUIREMENT_HEADING_PREFIX: &str = "Requirement:";
const SCENARIO_HEADING_PREFIX: &str = "Scenario:";
const REQUIREMENT_HEADING_LEVEL: usize = 3;
const SCENARIO_HEADING_LEVEL: usize = 4;
const REQUIREMENT_MARKER_PREFIX: &str = "r[";
const REQUIREMENT_MARKER_SUFFIX: char = ']';

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecSubstanceInventory {
    pub path: String,
    pub requirement_blocks: usize,
    pub substantive_requirement_blocks: usize,
    pub scenario_blocks: usize,
}

pub fn validate_spec_sources(sources: &[SpecSource]) -> Vec<String> {
    let mut ordered = sources.to_vec();
    ordered.sort_by(|left, right| left.path.cmp(&right.path).then(left.id.cmp(&right.id)));
    let mut issues = Vec::with_capacity(ordered.len().saturating_mul(2));
    for source in &ordered {
        issues.extend(validate_spec_source(source));
    }
    issues
}

pub fn validate_spec_sources_with_policy(
    sources: &[SpecSource],
    policy: &ArtifactSubstancePolicy,
) -> Vec<String> {
    let mut ordered = sources.to_vec();
    ordered.sort_by(|left, right| left.path.cmp(&right.path).then(left.id.cmp(&right.id)));
    let mut issues = Vec::with_capacity(ordered.len().saturating_mul(2));
    for source in &ordered {
        let (_inventory, source_issues) = validate_spec_source_with_policy(source, policy);
        issues.extend(source_issues);
    }
    issues
}

pub fn validate_spec_source(source: &SpecSource) -> Vec<String> {
    // Compatibility validator retained for callers that do not have reviewed
    // policy. Product validation and mutation surfaces use the policy-aware API.
    // r[impl cairn.supersede.validate_ears] r[impl cairn.supersede.validate_ears.accept]
    // r[impl cairn.supersede.validate_ears.weak_requirement]
    // r[impl cairn.supersede.validate_ears.incomplete_scenario]
    // r[impl cairn.supersede.validate_ears.boundary]
    let lines = source.markdown.lines().collect::<Vec<_>>();
    let mut issues = Vec::with_capacity(lines.len().saturating_div(REQUIREMENT_HEADING_LEVEL));
    for (index, line) in lines.iter().enumerate() {
        let Some((level, heading)) = markdown_heading(line) else {
            continue;
        };
        if level == REQUIREMENT_HEADING_LEVEL && heading.starts_with(REQUIREMENT_HEADING_PREFIX) {
            issues.extend(validate_requirement_block_compatibility(
                source, &lines, index, heading,
            ));
        }
        if level == SCENARIO_HEADING_LEVEL && heading.starts_with(SCENARIO_HEADING_PREFIX) {
            issues.extend(validate_scenario_block(source, &lines, index, heading));
        }
    }
    issues
}

pub fn validate_spec_source_with_policy(
    source: &SpecSource,
    policy: &ArtifactSubstancePolicy,
) -> (SpecSubstanceInventory, Vec<String>) {
    // r[impl cairn.lifecycle_validation.spec_substance]
    // r[impl cairn.lifecycle_validation.spec_substance.borrowed_strength]
    // r[impl cairn.lifecycle_validation.spec_substance.identity]
    let lines = source.markdown.lines().collect::<Vec<_>>();
    let visible_headings = visible_heading_inventory(&lines);
    let mut requirement_blocks = 0;
    let mut substantive_requirement_blocks = 0;
    let mut scenario_blocks = 0;
    let mut issues = Vec::new();

    for heading in &visible_headings {
        if heading.level == REQUIREMENT_HEADING_LEVEL
            && heading.text.starts_with(REQUIREMENT_HEADING_PREFIX)
        {
            requirement_blocks += 1;
            let body_end = next_visible_heading_at_or_above(
                &visible_headings,
                heading.line_index,
                SCENARIO_HEADING_LEVEL,
                lines.len(),
            );
            let requirement_lines = &lines[heading.line_index..body_end];
            let identities = requirement_identities(requirement_lines);
            let has_identity = identities.len() == 1;
            if !has_identity {
                issues.push(format!(
                    "{}:{}: lifecycle_validation.spec_substance.identity: {} must carry exactly one canonical bare r[...] requirement ID before its first scenario; found {}",
                    source.path,
                    heading.line_index + 1,
                    heading.text,
                    identities.len()
                ));
            }
            let has_strength = has_policy_normative_statement(
                &lines[heading.line_index + 1..body_end],
                &policy.normative_tokens,
            );
            if !has_strength {
                issues.push(format!(
                    "{}:{}: lifecycle_validation.spec_substance.normative_strength: {} missing requirement strength ({}) in its own body",
                    source.path,
                    heading.line_index + 1,
                    heading.text,
                    policy.normative_tokens.join("/")
                ));
            }
            if has_identity && has_strength {
                substantive_requirement_blocks += 1;
            }
        }
        if heading.level == SCENARIO_HEADING_LEVEL
            && heading.text.starts_with(SCENARIO_HEADING_PREFIX)
        {
            scenario_blocks += 1;
            issues.extend(validate_visible_scenario_block(
                source,
                &lines,
                &visible_headings,
                heading,
            ));
        }
    }

    if requirement_blocks == 0 {
        issues.push(format!(
            "{}: lifecycle_validation.spec_substance.zero_requirements: expected at least {} well-formed requirement block(s)",
            source.path, policy.minimum_requirement_blocks
        ));
    } else if substantive_requirement_blocks < policy.minimum_requirement_blocks {
        issues.push(format!(
            "{}: lifecycle_validation.spec_substance.minimum_requirements: found {} substantive requirement block(s), expected at least {}",
            source.path, substantive_requirement_blocks, policy.minimum_requirement_blocks
        ));
    }
    issues.sort();
    issues.dedup();
    (
        SpecSubstanceInventory {
            path: source.path.clone(),
            requirement_blocks,
            substantive_requirement_blocks,
            scenario_blocks,
        },
        issues,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VisibleHeading {
    line_index: usize,
    level: usize,
    text: String,
}

fn visible_heading_inventory(lines: &[&str]) -> Vec<VisibleHeading> {
    let mut headings = Vec::new();
    let mut fenced = false;
    let mut in_comment = false;
    for (line_index, line) in lines.iter().enumerate() {
        if fence_boundary(line) && !in_comment {
            fenced = !fenced;
            continue;
        }
        let visible = visible_text_without_comments(line, &mut in_comment);
        if fenced || visible.trim().is_empty() {
            continue;
        }
        if let Some((level, text)) = markdown_heading(visible.trim_end()) {
            headings.push(VisibleHeading {
                line_index,
                level,
                text: text.to_string(),
            });
        }
    }
    headings
}

fn next_visible_heading_at_or_above(
    headings: &[VisibleHeading],
    current_line: usize,
    max_level: usize,
    default_end: usize,
) -> usize {
    headings
        .iter()
        .find(|heading| heading.line_index > current_line && heading.level <= max_level)
        .map_or(default_end, |heading| heading.line_index)
}

fn requirement_identities(lines: &[&str]) -> Vec<String> {
    let mut identities = Vec::new();
    let mut fenced = false;
    let mut in_comment = false;
    for line in lines {
        if fence_boundary(line) && !in_comment {
            fenced = !fenced;
            continue;
        }
        let visible = visible_text_without_comments(line, &mut in_comment);
        if fenced {
            continue;
        }
        let candidates = marker_candidates(&visible);
        if !candidates.is_empty() {
            identities.extend(
                candidates
                    .into_iter()
                    .filter(|candidate| valid_lifecycle_id(candidate)),
            );
            return identities;
        }
    }
    identities
}

fn marker_candidates(line: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find(REQUIREMENT_MARKER_PREFIX) {
        let after_prefix = &rest[start + REQUIREMENT_MARKER_PREFIX.len()..];
        let Some(end) = after_prefix.find(REQUIREMENT_MARKER_SUFFIX) else {
            break;
        };
        candidates.push(after_prefix[..end].trim().to_string());
        rest = &after_prefix[end + REQUIREMENT_MARKER_SUFFIX.len_utf8()..];
    }
    candidates
}

fn valid_lifecycle_id(candidate: &str) -> bool {
    let bytes = candidate.as_bytes();
    bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

pub(super) fn visible_requirement_reference_set(markdown: &str) -> BTreeSet<String> {
    let mut references = BTreeSet::new();
    let mut fenced = false;
    let mut in_comment = false;
    for line in markdown.lines() {
        if fence_boundary(line) && !in_comment {
            fenced = !fenced;
            continue;
        }
        let visible = visible_text_without_comments(line, &mut in_comment);
        if fenced {
            continue;
        }
        references.extend(
            marker_candidates(&visible)
                .into_iter()
                .filter(|candidate| valid_lifecycle_id(candidate)),
        );
    }
    references
}

fn has_policy_normative_statement(lines: &[&str], normative_tokens: &[String]) -> bool {
    let mut fenced = false;
    let mut in_comment = false;
    for line in lines {
        if fence_boundary(line) && !in_comment {
            fenced = !fenced;
            continue;
        }
        let visible = visible_text_without_comments(line, &mut in_comment);
        if fenced {
            continue;
        }
        let upper = visible.to_ascii_uppercase();
        if normative_tokens
            .iter()
            .any(|token| contains_word(&upper, &token.to_ascii_uppercase()))
        {
            return true;
        }
    }
    false
}

pub(super) fn visible_text_without_comments(line: &str, in_comment: &mut bool) -> String {
    let mut output = String::new();
    let mut rest = line;
    loop {
        if *in_comment {
            let Some(end) = rest.find("-->") else {
                return output;
            };
            *in_comment = false;
            rest = &rest[end + "-->".len()..];
            continue;
        }
        let Some(start) = rest.find("<!--") else {
            output.push_str(rest);
            return output;
        };
        output.push_str(&rest[..start]);
        *in_comment = true;
        rest = &rest[start + "<!--".len()..];
    }
}

pub(super) fn fence_boundary(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

fn contains_word(text: &str, needle: &str) -> bool {
    text.split(|value: char| !value.is_ascii_alphanumeric())
        .any(|word| word == needle)
}

fn validate_requirement_block_compatibility(
    source: &SpecSource,
    lines: &[&str],
    index: usize,
    heading: &str,
) -> Option<String> {
    let end = next_heading_at_or_above(lines, index + 1, REQUIREMENT_HEADING_LEVEL);
    if has_requirement_strength(&lines[index + 1..end]) {
        return None;
    }
    Some(format!(
        "{}: {} missing requirement strength (MUST/SHALL/SHOULD/MAY)",
        source.path, heading
    ))
}

fn validate_scenario_block(
    source: &SpecSource,
    lines: &[&str],
    index: usize,
    heading: &str,
) -> Option<String> {
    let end = next_heading_at_or_above(lines, index + 1, SCENARIO_HEADING_LEVEL);
    scenario_clause_issue(source, heading, &lines[index + 1..end])
}

fn validate_visible_scenario_block(
    source: &SpecSource,
    lines: &[&str],
    headings: &[VisibleHeading],
    heading: &VisibleHeading,
) -> Option<String> {
    let end = next_visible_heading_at_or_above(
        headings,
        heading.line_index,
        SCENARIO_HEADING_LEVEL,
        lines.len(),
    );
    scenario_clause_issue(source, &heading.text, &lines[heading.line_index + 1..end])
}

fn scenario_clause_issue(source: &SpecSource, heading: &str, block: &[&str]) -> Option<String> {
    let mut missing = Vec::with_capacity(REQUIREMENT_HEADING_LEVEL);
    if !has_scenario_clause(block, "GIVEN") {
        missing.push("GIVEN");
    }
    if !has_scenario_clause(block, "WHEN") {
        missing.push("WHEN");
    }
    if !has_scenario_clause(block, "THEN") {
        missing.push("THEN");
    }
    if missing.is_empty() {
        return None;
    }
    Some(format!(
        "{}: {} missing {} clause(s)",
        source.path,
        heading,
        missing.join("/")
    ))
}
