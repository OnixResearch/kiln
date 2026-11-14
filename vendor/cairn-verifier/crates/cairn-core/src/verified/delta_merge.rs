use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::evidence::blake3_hex;

const ACCEPTED_REQUIREMENTS_HEADING: &str = "## Requirements";
const ADDED_REQUIREMENTS_HEADING: &str = "## ADDED Requirements";
const MODIFIED_REQUIREMENTS_HEADING: &str = "## MODIFIED Requirements";
const REMOVED_REQUIREMENTS_HEADING: &str = "## REMOVED Requirements";
const REQUIREMENT_HEADING_PREFIX: &str = "### Requirement:";
const SCENARIO_HEADING_PREFIX: &str = "#### Scenario:";
const REQUIREMENT_ID_PREFIX: &str = "r[";
const REQUIREMENT_ID_SUFFIX: char = ']';

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeltaOperationKind {
    Added,
    Modified,
    Removed,
}

impl DeltaOperationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Modified => "modified",
            Self::Removed => "removed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeltaOperationState {
    Applied,
    AlreadyApplied,
}

impl DeltaOperationState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::AlreadyApplied => "already_applied",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeltaOperation {
    pub kind: DeltaOperationKind,
    pub requirement_id: String,
    pub heading: String,
    pub block_hash: String,
    #[serde(skip)]
    pub block: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeltaOperationOutcome {
    pub kind: DeltaOperationKind,
    pub requirement_id: String,
    pub state: DeltaOperationState,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct DeltaMergeDiagnostic {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeltaMerge {
    pub markdown: String,
    pub added_blocks: usize,
    pub removed_blocks: usize,
    pub operations: Vec<DeltaOperation>,
    pub outcomes: Vec<DeltaOperationOutcome>,
    pub diagnostics: Vec<DeltaMergeDiagnostic>,
    pub delta_hash: String,
    pub accepted_hash: String,
    pub result_hash: String,
    pub changed: bool,
    pub blocked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AcceptedBlock {
    requirement_id: String,
    heading: String,
    block: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AcceptedDocument {
    prefix: String,
    preamble: String,
    blocks: Vec<AcceptedBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeltaSection {
    kind: DeltaOperationKind,
    heading: &'static str,
    lines: Vec<String>,
}

pub fn merge_delta_spec(accepted: Option<&str>, delta: &str, spec_id: &str) -> DeltaMerge {
    // r[impl cairn.sync_archive.delta_merge.preflight]
    // r[impl cairn.sync_archive.delta_merge.identity]
    // r[impl cairn.sync_archive.delta_merge.operations]
    // r[impl cairn.sync_archive.delta_merge.receipt_binding]
    // r[impl cairn.sync_archive.delta_merge.mutation_truth]
    let base = accepted
        .filter(|text| !text.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| empty_accepted_spec(spec_id));
    let accepted_hash = blake3_hex(base.as_bytes());
    let delta_hash = blake3_hex(delta.as_bytes());

    let (operations, mut diagnostics) = parse_delta_operations(delta);
    let accepted_document = parse_accepted_document(&base, &mut diagnostics);
    diagnose_duplicate_operation_ids(&operations, &mut diagnostics);
    sort_diagnostics(&mut diagnostics);

    let (mut computed_markdown, outcomes) = if diagnostics.is_empty() {
        apply_operations(accepted_document, &operations, &mut diagnostics)
    } else {
        (base.clone(), Vec::new())
    };
    sort_diagnostics(&mut diagnostics);

    let blocked = !diagnostics.is_empty();
    let outcomes = if blocked { Vec::new() } else { outcomes };
    let all_operations_already_applied = !outcomes.is_empty()
        && outcomes
            .iter()
            .all(|outcome| outcome.state == DeltaOperationState::AlreadyApplied);
    if all_operations_already_applied {
        computed_markdown = base.clone();
    }
    let changed = !blocked && computed_markdown != base;
    let markdown = if blocked { base } else { computed_markdown };
    let result_hash = blake3_hex(markdown.as_bytes());
    let added_blocks = operations
        .iter()
        .filter(|operation| operation.kind != DeltaOperationKind::Removed)
        .count();
    let removed_blocks = operations
        .iter()
        .filter(|operation| operation.kind != DeltaOperationKind::Added)
        .count();

    DeltaMerge {
        markdown,
        added_blocks,
        removed_blocks,
        operations,
        outcomes,
        diagnostics,
        delta_hash,
        accepted_hash,
        result_hash,
        changed,
        blocked,
    }
}

fn parse_delta_operations(delta: &str) -> (Vec<DeltaOperation>, Vec<DeltaMergeDiagnostic>) {
    let mut diagnostics = Vec::new();
    let sections = extract_delta_sections(delta, &mut diagnostics);
    let mut operations = Vec::new();

    for section in sections {
        let blocks = parse_requirement_blocks(&section.lines, section.heading, &mut diagnostics);
        if blocks.is_empty() {
            diagnostics.push(diagnostic(
                "delta_merge.empty_section",
                format!(
                    "{} contains no requirement operations; add at least one `{REQUIREMENT_HEADING_PREFIX} ...` block with one bare r[...] ID",
                    section.heading
                ),
            ));
        }
        for block in blocks {
            if let Some(operation) = operation_from_block(section.kind, &block, &mut diagnostics) {
                operations.push(operation);
            }
        }
    }

    if operations.is_empty() {
        diagnostics.push(diagnostic(
            "delta_merge.no_operations",
            "no supported delta operations were recognized; use explicit `## ADDED Requirements`, `## MODIFIED Requirements`, or `## REMOVED Requirements` sections instead of a full accepted-spec projection",
        ));
    }

    (operations, diagnostics)
}

fn extract_delta_sections(
    delta: &str,
    diagnostics: &mut Vec<DeltaMergeDiagnostic>,
) -> Vec<DeltaSection> {
    let mut sections = Vec::new();
    let mut current: Option<DeltaSection> = None;
    let mut seen = BTreeSet::new();
    let mut fenced = false;

    for line in delta.lines() {
        if fence_boundary(line) {
            fenced = !fenced;
            if let Some(section) = current.as_mut() {
                section.lines.push(line.to_string());
            }
            continue;
        }
        if !fenced {
            if let Some((kind, heading)) = operation_section(line) {
                if let Some(section) = current.take() {
                    sections.push(section);
                }
                if !seen.insert(kind) {
                    diagnostics.push(diagnostic(
                        "delta_merge.duplicate_section",
                        format!("delta contains duplicate `{heading}` sections"),
                    ));
                }
                current = Some(DeltaSection {
                    kind,
                    heading,
                    lines: Vec::new(),
                });
                continue;
            }
            if line.starts_with("## ") {
                if let Some(section) = current.take() {
                    sections.push(section);
                }
                continue;
            }
        }
        if let Some(section) = current.as_mut() {
            section.lines.push(line.to_string());
        }
    }
    if let Some(section) = current {
        sections.push(section);
    }
    sections
}

fn operation_section(line: &str) -> Option<(DeltaOperationKind, &'static str)> {
    match line.trim_end() {
        ADDED_REQUIREMENTS_HEADING => Some((DeltaOperationKind::Added, ADDED_REQUIREMENTS_HEADING)),
        MODIFIED_REQUIREMENTS_HEADING => {
            Some((DeltaOperationKind::Modified, MODIFIED_REQUIREMENTS_HEADING))
        }
        REMOVED_REQUIREMENTS_HEADING => {
            Some((DeltaOperationKind::Removed, REMOVED_REQUIREMENTS_HEADING))
        }
        _ => None,
    }
}

fn parse_requirement_blocks(
    lines: &[String],
    section_heading: &str,
    diagnostics: &mut Vec<DeltaMergeDiagnostic>,
) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current = Vec::new();
    let mut preamble = Vec::new();
    let mut fenced = false;

    for line in lines {
        if fence_boundary(line) {
            fenced = !fenced;
        }
        let starts_requirement = !fenced && line.starts_with(REQUIREMENT_HEADING_PREFIX);
        if starts_requirement {
            if !current.is_empty() {
                blocks.push(normalize_block(&current));
                current.clear();
            }
            current.push(line.to_string());
        } else if current.is_empty() {
            preamble.push(line.to_string());
        } else {
            current.push(line.to_string());
        }
    }
    if !current.is_empty() {
        blocks.push(normalize_block(&current));
    }
    if preamble.iter().any(|line| !line.trim().is_empty()) {
        diagnostics.push(diagnostic(
            "delta_merge.unscoped_section_content",
            format!(
                "{section_heading} contains content outside a requirement block; move reviewed operation text under a `{REQUIREMENT_HEADING_PREFIX} ...` heading"
            ),
        ));
    }
    blocks.retain(|block| !block.is_empty());
    blocks
}

fn operation_from_block(
    kind: DeltaOperationKind,
    block: &str,
    diagnostics: &mut Vec<DeltaMergeDiagnostic>,
) -> Option<DeltaOperation> {
    let heading = block.lines().next().unwrap_or_default().trim().to_string();
    if heading
        .trim_start_matches(REQUIREMENT_HEADING_PREFIX)
        .trim()
        .is_empty()
    {
        diagnostics.push(diagnostic(
            "delta_merge.empty_requirement_heading",
            format!(
                "{} operation has an empty requirement heading",
                kind.as_str()
            ),
        ));
    }
    let ids = requirement_identity_candidates(block, diagnostics, kind.as_str());
    if ids.len() != 1 {
        diagnostics.push(diagnostic(
            "delta_merge.requirement_identity_cardinality",
            format!(
                "{} operation `{heading}` must carry exactly one valid bare r[...] requirement ID before its first scenario; found {}",
                kind.as_str(),
                ids.len()
            ),
        ));
        return None;
    }
    Some(DeltaOperation {
        kind,
        requirement_id: ids[0].clone(),
        heading,
        block_hash: blake3_hex(block.as_bytes()),
        block: block.to_string(),
    })
}

fn parse_accepted_document(
    accepted: &str,
    diagnostics: &mut Vec<DeltaMergeDiagnostic>,
) -> AcceptedDocument {
    let requirements_heading_count =
        heading_count_outside_fences(accepted, ACCEPTED_REQUIREMENTS_HEADING);
    if requirements_heading_count > 1 {
        diagnostics.push(diagnostic(
            "delta_merge.duplicate_accepted_requirements_section",
            format!(
                "accepted spec contains {requirements_heading_count} `{ACCEPTED_REQUIREMENTS_HEADING}` sections; retain one unambiguous accepted requirements section"
            ),
        ));
    }
    let Some((prefix, requirements)) = split_requirements(accepted) else {
        // Preserve historical wrapperless accepted prose only when it carries no
        // requirement blocks. A wrapperless requirement inventory is ambiguous.
        if heading_prefix_count_outside_fences(accepted, REQUIREMENT_HEADING_PREFIX) > 0 {
            diagnostics.push(diagnostic(
                "delta_merge.accepted_missing_requirements_section",
                "accepted spec contains requirement blocks without an exact `## Requirements` section",
            ));
        }
        return AcceptedDocument {
            prefix: accepted.trim_end().to_string(),
            preamble: String::new(),
            blocks: Vec::new(),
        };
    };
    let (preamble, raw_blocks) = accepted_requirement_blocks(&requirements);
    let mut blocks = Vec::new();
    let mut locations = BTreeMap::<String, Vec<String>>::new();

    for block in raw_blocks {
        let heading = block.lines().next().unwrap_or_default().trim().to_string();
        let ids = requirement_identity_candidates(&block, diagnostics, "accepted requirement");
        if ids.len() != 1 {
            diagnostics.push(diagnostic(
                "delta_merge.accepted_identity_cardinality",
                format!(
                    "accepted requirement `{heading}` must carry exactly one valid bare r[...] requirement ID before its first scenario; found {}",
                    ids.len()
                ),
            ));
            continue;
        }
        let requirement_id = ids[0].clone();
        locations
            .entry(requirement_id.clone())
            .or_default()
            .push(heading.clone());
        blocks.push(AcceptedBlock {
            requirement_id,
            heading,
            block,
        });
    }

    for (requirement_id, headings) in locations {
        if headings.len() > 1 {
            diagnostics.push(diagnostic(
                "delta_merge.duplicate_accepted_id",
                format!(
                    "accepted requirement ID `{requirement_id}` occurs in conflicting blocks: {}",
                    headings.join(", ")
                ),
            ));
        }
    }

    AcceptedDocument {
        prefix,
        preamble,
        blocks,
    }
}

fn accepted_requirement_blocks(requirements: &str) -> (String, Vec<String>) {
    let lines = requirements
        .lines()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let mut preamble = Vec::new();
    let mut blocks = Vec::new();
    let mut current = Vec::new();
    let mut fenced = false;

    for line in lines {
        if fence_boundary(&line) {
            fenced = !fenced;
        }
        let starts_requirement = !fenced && line.starts_with(REQUIREMENT_HEADING_PREFIX);
        if starts_requirement {
            if !current.is_empty() {
                blocks.push(normalize_block(&current));
                current.clear();
            }
            current.push(line);
        } else if current.is_empty() {
            preamble.push(line);
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        blocks.push(normalize_block(&current));
    }
    (preamble.join("\n").trim().to_string(), blocks)
}

fn requirement_identity_candidates(
    block: &str,
    diagnostics: &mut Vec<DeltaMergeDiagnostic>,
    context: &str,
) -> Vec<String> {
    let mut ids = Vec::new();
    let mut fenced = false;
    for line in block
        .lines()
        .skip_while(|line| !line.starts_with(REQUIREMENT_HEADING_PREFIX))
    {
        if fence_boundary(line) {
            fenced = !fenced;
            continue;
        }
        if !fenced && line.starts_with(SCENARIO_HEADING_PREFIX) {
            break;
        }
        if fenced {
            continue;
        }
        for candidate in marker_candidates(line) {
            if valid_requirement_id(&candidate) {
                ids.push(candidate);
            } else {
                diagnostics.push(diagnostic(
                    "delta_merge.invalid_requirement_id",
                    format!(
                        "{context} contains invalid bare requirement ID `{candidate}`; use ASCII letters, digits, `.`, `_`, or `-`"
                    ),
                ));
            }
        }
    }
    ids
}

fn marker_candidates(line: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find(REQUIREMENT_ID_PREFIX) {
        let after_prefix = &rest[start + REQUIREMENT_ID_PREFIX.len()..];
        let Some(end) = after_prefix.find(REQUIREMENT_ID_SUFFIX) else {
            break;
        };
        candidates.push(after_prefix[..end].to_string());
        rest = &after_prefix[end + REQUIREMENT_ID_SUFFIX.len_utf8()..];
    }
    candidates
}

fn valid_requirement_id(candidate: &str) -> bool {
    !candidate.is_empty()
        && candidate
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn diagnose_duplicate_operation_ids(
    operations: &[DeltaOperation],
    diagnostics: &mut Vec<DeltaMergeDiagnostic>,
) {
    let mut locations = BTreeMap::<&str, Vec<String>>::new();
    for operation in operations {
        locations
            .entry(&operation.requirement_id)
            .or_default()
            .push(format!(
                "{} `{}`",
                operation.kind.as_str(),
                operation.heading
            ));
    }
    for (requirement_id, operations) in locations {
        if operations.len() > 1 {
            diagnostics.push(diagnostic(
                "delta_merge.duplicate_operation_id",
                format!(
                    "delta requirement ID `{requirement_id}` occurs in conflicting operations: {}",
                    operations.join(", ")
                ),
            ));
        }
    }
}

fn apply_operations(
    mut document: AcceptedDocument,
    operations: &[DeltaOperation],
    diagnostics: &mut Vec<DeltaMergeDiagnostic>,
) -> (String, Vec<DeltaOperationOutcome>) {
    let original = render_accepted_document(&document);
    let mut outcomes = Vec::new();

    for operation in operations {
        let position = document
            .blocks
            .iter()
            .position(|block| block.requirement_id == operation.requirement_id);
        match (operation.kind, position) {
            (DeltaOperationKind::Added, None) => {
                document.blocks.push(AcceptedBlock {
                    requirement_id: operation.requirement_id.clone(),
                    heading: operation.heading.clone(),
                    block: operation.block.clone(),
                });
                outcomes.push(outcome(operation, DeltaOperationState::Applied));
            }
            (DeltaOperationKind::Added, Some(index)) => {
                if document.blocks[index].block == operation.block {
                    outcomes.push(outcome(operation, DeltaOperationState::AlreadyApplied));
                } else {
                    diagnostics.push(diagnostic(
                        "delta_merge.add_conflict",
                        format!(
                            "ADDED operation for `{}` conflicts with existing heading `{}`; use MODIFIED for a reviewed replacement",
                            operation.requirement_id, document.blocks[index].heading
                        ),
                    ));
                }
            }
            (DeltaOperationKind::Modified, Some(index)) => {
                if document.blocks[index].block == operation.block {
                    outcomes.push(outcome(operation, DeltaOperationState::AlreadyApplied));
                } else {
                    document.blocks[index] = AcceptedBlock {
                        requirement_id: operation.requirement_id.clone(),
                        heading: operation.heading.clone(),
                        block: operation.block.clone(),
                    };
                    outcomes.push(outcome(operation, DeltaOperationState::Applied));
                }
            }
            (DeltaOperationKind::Modified, None) => diagnostics.push(diagnostic(
                "delta_merge.unknown_modify_target",
                format!(
                    "MODIFIED operation targets absent requirement ID `{}`; use ADDED for a new requirement",
                    operation.requirement_id
                ),
            )),
            (DeltaOperationKind::Removed, Some(index)) => {
                document.blocks.remove(index);
                outcomes.push(outcome(operation, DeltaOperationState::Applied));
            }
            (DeltaOperationKind::Removed, None) => {
                outcomes.push(outcome(operation, DeltaOperationState::AlreadyApplied));
            }
        }
    }

    if diagnostics.is_empty() {
        (render_accepted_document(&document), outcomes)
    } else {
        (original, outcomes)
    }
}

fn outcome(operation: &DeltaOperation, state: DeltaOperationState) -> DeltaOperationOutcome {
    DeltaOperationOutcome {
        kind: operation.kind,
        requirement_id: operation.requirement_id.clone(),
        state,
    }
}

fn empty_accepted_spec(spec_id: &str) -> String {
    format!(
        "# {} Specification\n\n## Purpose\n\nDefines the `{}` capability.\n\n{ACCEPTED_REQUIREMENTS_HEADING}\n",
        title_from_spec_id(spec_id),
        spec_id
    )
}

fn title_from_spec_id(spec_id: &str) -> String {
    spec_id
        .split(['-', '_', '/', '.'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn split_requirements(markdown: &str) -> Option<(String, String)> {
    let offset = markdown.find(ACCEPTED_REQUIREMENTS_HEADING)?;
    let prefix = markdown[..offset].trim_end().to_string();
    let requirements = markdown[offset + ACCEPTED_REQUIREMENTS_HEADING.len()..]
        .trim()
        .to_string();
    Some((prefix, requirements))
}

fn render_accepted_document(document: &AcceptedDocument) -> String {
    let mut parts = Vec::new();
    if !document.preamble.is_empty() {
        parts.push(document.preamble.clone());
    }
    parts.extend(document.blocks.iter().map(|block| block.block.clone()));
    let requirements = parts.join("\n\n");
    if requirements.is_empty() {
        format!(
            "{}\n\n{ACCEPTED_REQUIREMENTS_HEADING}\n",
            document.prefix.trim_end()
        )
    } else {
        format!(
            "{}\n\n{ACCEPTED_REQUIREMENTS_HEADING}\n\n{}\n",
            document.prefix.trim_end(),
            requirements.trim()
        )
    }
}

fn normalize_block(lines: &[String]) -> String {
    lines.join("\n").trim().to_string()
}

fn heading_count_outside_fences(markdown: &str, heading: &str) -> usize {
    let mut count = 0;
    let mut fenced = false;
    for line in markdown.lines() {
        if fence_boundary(line) {
            fenced = !fenced;
            continue;
        }
        if !fenced && line.trim_end() == heading {
            count += 1;
        }
    }
    count
}

fn heading_prefix_count_outside_fences(markdown: &str, heading_prefix: &str) -> usize {
    let mut count = 0;
    let mut fenced = false;
    for line in markdown.lines() {
        if fence_boundary(line) {
            fenced = !fenced;
            continue;
        }
        if !fenced && line.starts_with(heading_prefix) {
            count += 1;
        }
    }
    count
}

fn fence_boundary(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

fn diagnostic(code: impl Into<String>, message: impl Into<String>) -> DeltaMergeDiagnostic {
    DeltaMergeDiagnostic {
        code: code.into(),
        message: message.into(),
    }
}

fn sort_diagnostics(diagnostics: &mut Vec<DeltaMergeDiagnostic>) {
    diagnostics.sort();
    diagnostics.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCEPTED: &str = "# Demo Specification\n\n## Purpose\n\nKeep this purpose.\n\n## Requirements\n\n### Requirement: Existing\n\nr[demo.existing] The system MUST keep old content.\n\n#### Scenario: Old\n\nr[demo.existing.old]\n- GIVEN old\n- WHEN old\n- THEN old\n";

    fn diagnostic_codes(merged: &DeltaMerge) -> Vec<&str> {
        merged
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect()
    }

    #[test]
    fn applies_add_modify_remove_and_renamed_heading_by_requirement_id() {
        // r[verify cairn.sync_archive.delta_merge.preflight.valid]
        // r[verify cairn.sync_archive.delta_merge.identity.rename]
        // r[verify cairn.sync_archive.delta_merge.tests]
        // r[verify cairn.sync_archive.delta_merge.tests.positive]
        let delta = "## MODIFIED Requirements\n\n### Requirement: Renamed existing\n\nr[demo.existing] The system MUST use updated content.\n\n## ADDED Requirements\n\n### Requirement: New\n\nr[demo.new] The system MUST add new content.\n";
        let merged = merge_delta_spec(Some(ACCEPTED), delta, "demo");
        assert!(!merged.blocked, "{:?}", merged.diagnostics);
        assert!(merged.changed);
        assert!(!merged.markdown.contains("keep old content"));
        assert!(merged
            .markdown
            .contains("### Requirement: Renamed existing"));
        assert!(merged.markdown.contains("r[demo.new]"));
        assert_eq!(merged.markdown.matches("r[demo.existing]").count(), 1);
    }

    #[test]
    fn creates_stable_accepted_spec_for_new_domain() {
        let delta = "## ADDED Requirements\n\n### Requirement: First\n\nr[new.domain.first] The system MUST create a wrapped spec.\n";
        let merged = merge_delta_spec(None, delta, "new-domain");
        assert!(!merged.blocked, "{:?}", merged.diagnostics);
        assert!(merged.markdown.starts_with("# New Domain Specification"));
        assert!(merged.markdown.contains("## Purpose"));
        assert!(merged.markdown.contains("## Requirements"));
        assert!(!merged.markdown.contains(ADDED_REQUIREMENTS_HEADING));
    }

    #[test]
    fn removal_and_idempotent_replay_have_explicit_outcomes() {
        // r[verify cairn.sync_archive.delta_merge.operations.idempotent]
        let remove = "## REMOVED Requirements\n\n### Requirement: Existing\n\nr[demo.existing]\n";
        let removed = merge_delta_spec(Some(ACCEPTED), remove, "demo");
        assert!(!removed.blocked, "{:?}", removed.diagnostics);
        assert!(!removed.markdown.contains("r[demo.existing]"));
        assert_eq!(removed.outcomes[0].state, DeltaOperationState::Applied);
        let replayed = merge_delta_spec(Some(&removed.markdown), remove, "demo");
        assert!(!replayed.changed);
        assert_eq!(
            replayed.outcomes[0].state,
            DeltaOperationState::AlreadyApplied
        );
    }

    #[test]
    fn full_spec_empty_and_fenced_fake_sections_fail_closed() {
        // r[verify cairn.sync_archive.delta_merge.preflight.full_spec]
        // r[verify cairn.sync_archive.delta_merge.preflight.empty]
        // r[verify cairn.sync_archive.delta_merge.tests.negative]
        for delta in [
            "# Demo Specification\n\n## Requirements\n\n### Requirement: Proposed\n\nr[demo.proposed] The system MUST not disappear.\n",
            "",
            "```markdown\n## ADDED Requirements\n### Requirement: Fake\nr[demo.fake] The system MUST be ignored.\n```\n",
        ] {
            let merged = merge_delta_spec(Some(ACCEPTED), delta, "demo");
            assert!(merged.blocked);
            assert!(!merged.changed);
            assert_eq!(merged.markdown, ACCEPTED);
            assert!(diagnostic_codes(&merged).contains(&"delta_merge.no_operations"));
        }
    }

    #[test]
    fn missing_multiple_invalid_and_duplicate_operation_ids_fail_closed() {
        // r[verify cairn.sync_archive.delta_merge.identity.malformed]
        // r[verify cairn.sync_archive.delta_merge.identity.duplicate]
        let fixtures = [
            "## ADDED Requirements\n\n### Requirement: Missing\n\nThe system MUST fail.\n",
            "## REMOVED Requirements\n\n### Requirement: Malformed removal\n\nThe system MUST fail without an identity.\n",
            "## ADDED Requirements\n\n### Requirement: Multiple\n\nr[demo.one] r[demo.two] The system MUST fail.\n",
            "## ADDED Requirements\n\n### Requirement: Invalid\n\nr[demo/bad] The system MUST fail.\n",
            "## ADDED Requirements\n\n### Requirement: One\n\nr[demo.same] The system MUST fail.\n\n## MODIFIED Requirements\n\n### Requirement: Two\n\nr[demo.same] The system MUST also fail.\n",
        ];
        for delta in fixtures {
            let merged = merge_delta_spec(Some(ACCEPTED), delta, "demo");
            assert!(merged.blocked, "{delta}");
            assert!(!merged.changed);
            assert_eq!(merged.markdown, ACCEPTED);
        }
    }

    #[test]
    fn duplicate_accepted_identity_reports_every_heading() {
        let accepted = format!(
            "{ACCEPTED}\n### Requirement: Duplicate\n\nr[demo.existing] The system MUST collide.\n"
        );
        let delta = "## MODIFIED Requirements\n\n### Requirement: Updated\n\nr[demo.existing] The system MUST update.\n";
        let merged = merge_delta_spec(Some(&accepted), delta, "demo");
        assert!(merged.blocked);
        let duplicate = merged
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "delta_merge.duplicate_accepted_id")
            .expect("duplicate diagnostic");
        assert!(duplicate.message.contains("Existing"));
        assert!(duplicate.message.contains("Duplicate"));
    }

    #[test]
    fn conflicting_add_and_unknown_modify_preserve_accepted_bytes() {
        // r[verify cairn.sync_archive.delta_merge.operations.add]
        // r[verify cairn.sync_archive.delta_merge.operations.modify]
        let fixtures = [
            (
                "## ADDED Requirements\n\n### Requirement: Existing conflict\n\nr[demo.existing] The system MUST conflict.\n",
                "delta_merge.add_conflict",
            ),
            (
                "## MODIFIED Requirements\n\n### Requirement: Unknown\n\nr[demo.unknown] The system MUST not append.\n",
                "delta_merge.unknown_modify_target",
            ),
        ];
        for (delta, expected_code) in fixtures {
            let merged = merge_delta_spec(Some(ACCEPTED), delta, "demo");
            assert!(merged.blocked);
            assert_eq!(merged.markdown, ACCEPTED);
            assert!(diagnostic_codes(&merged).contains(&expected_code));
        }
    }

    #[test]
    fn identical_add_is_an_already_applied_noop() {
        let existing_block = "### Requirement: Existing\n\nr[demo.existing] The system MUST keep old content.\n\n#### Scenario: Old\n\nr[demo.existing.old]\n- GIVEN old\n- WHEN old\n- THEN old";
        let delta = format!("## ADDED Requirements\n\n{existing_block}\n");
        let accepted_with_extra_trailing_space = format!("{ACCEPTED}\n\n");
        let merged = merge_delta_spec(Some(&accepted_with_extra_trailing_space), &delta, "demo");
        assert!(!merged.blocked, "{:?}", merged.diagnostics);
        assert!(!merged.changed);
        assert_eq!(merged.markdown, accepted_with_extra_trailing_space);
        assert_eq!(
            merged.outcomes[0].state,
            DeltaOperationState::AlreadyApplied
        );
    }

    #[test]
    fn ambiguous_accepted_requirement_shapes_fail_closed() {
        let delta = "## ADDED Requirements\n\n### Requirement: New\n\nr[demo.new] The system MUST add content.\n";
        let wrapperless =
            "# Demo\n\n### Requirement: Existing\n\nr[demo.existing] The system MUST exist.\n";
        let duplicate_sections = format!("{ACCEPTED}\n## Requirements\n");
        for accepted in [wrapperless, duplicate_sections.as_str()] {
            let merged = merge_delta_spec(Some(accepted), delta, "demo");
            assert!(merged.blocked);
            assert!(!merged.changed);
            assert_eq!(merged.markdown, accepted);
        }
    }

    #[test]
    fn changed_delta_or_accepted_bytes_change_bound_identities() {
        // r[verify cairn.sync_archive.delta_merge.receipt_binding.delta]
        // r[verify cairn.sync_archive.delta_merge.receipt_binding.pre_state]
        let first_delta = "## ADDED Requirements\n\n### Requirement: New\n\nr[demo.new] The system MUST use first content.\n";
        let second_delta = first_delta.replace("first", "second");
        let first = merge_delta_spec(Some(ACCEPTED), first_delta, "demo");
        let changed_delta = merge_delta_spec(Some(ACCEPTED), &second_delta, "demo");
        let changed_accepted = merge_delta_spec(
            Some(&ACCEPTED.replace("Keep this purpose", "Keep another purpose")),
            first_delta,
            "demo",
        );
        assert_ne!(first.delta_hash, changed_delta.delta_hash);
        assert_ne!(first.result_hash, changed_delta.result_hash);
        assert_ne!(first.accepted_hash, changed_accepted.accepted_hash);
        assert_ne!(first.result_hash, changed_accepted.result_hash);
    }
}
