const ACCEPTED_REQUIREMENTS_HEADING: &str = "## Requirements";
const ADDED_REQUIREMENTS_HEADING: &str = "## ADDED Requirements";
const MODIFIED_REQUIREMENTS_HEADING: &str = "## MODIFIED Requirements";
const REMOVED_REQUIREMENTS_HEADING: &str = "## REMOVED Requirements";
const REQUIREMENT_HEADING_PREFIX: &str = "### Requirement:";
const SCENARIO_HEADING_PREFIX: &str = "#### Scenario:";
const REQUIREMENT_ID_PREFIX: &str = "r[";
const REQUIREMENT_ID_SUFFIX: char = ']';

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DeltaOperation {
    pub kind: DeltaOperationKind,
    pub requirement_id: String,
    pub heading: String,
    pub block_hash: String,
    #[serde(skip)]
    pub block: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DeltaOperationOutcome {
    pub kind: DeltaOperationKind,
    pub requirement_id: String,
    pub state: DeltaOperationState,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
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
    /// Pre-state requirement IDs that occur in more than one accepted block,
    /// mapped to their headings. A repair operation may target one of these
    /// blocks by its unique heading because its ID is not a usable target.
    duplicate_ids: std::collections::BTreeMap<String, Vec<String>>,
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
    let accepted_hash = super::evidence::blake3_hex(base.as_bytes());
    let delta_hash = super::evidence::blake3_hex(delta.as_bytes());

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
    let result_hash = super::evidence::blake3_hex(markdown.as_bytes());
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
    let mut seen = std::collections::BTreeSet::new();
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
        block_hash: super::evidence::blake3_hex(block.as_bytes()),
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
            duplicate_ids: std::collections::BTreeMap::new(),
        };
    };
    let (preamble, raw_blocks) = accepted_requirement_blocks(&requirements);
    let mut blocks = Vec::new();
    let mut locations = std::collections::BTreeMap::<String, Vec<String>>::new();

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

    let mut duplicate_ids = std::collections::BTreeMap::<String, Vec<String>>::new();
    for (requirement_id, headings) in locations {
        if headings.len() > 1 {
            duplicate_ids.insert(requirement_id, headings);
        }
    }

    AcceptedDocument {
        prefix,
        preamble,
        blocks,
        duplicate_ids,
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
    let mut locations = std::collections::BTreeMap::<&str, Vec<String>>::new();
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
    let duplicates = document.duplicate_ids.clone();
    let repairs = repair_operation_indices(&document, operations, &duplicates);
    let mut outcomes: Vec<Option<DeltaOperationOutcome>> = vec![None; operations.len()];

    // A duplicated accepted ID is not a usable target, so heading-targeted
    // repairs run first against the pre-state duplicate set. Every other
    // operation then resolves against the repaired document.
    for &index in &repairs {
        let operation = &operations[index];
        let heading_matches = heading_matches(&document, &operation.heading);
        let Some(target) = heading_matches.first().copied() else {
            continue;
        };
        document.blocks[target] = AcceptedBlock {
            requirement_id: operation.requirement_id.clone(),
            heading: operation.heading.clone(),
            block: operation.block.clone(),
        };
        outcomes[index] = Some(outcome(operation, DeltaOperationState::Applied));
    }

    for (index, operation) in operations.iter().enumerate() {
        if repairs.contains(&index) {
            continue;
        }
        outcomes[index] = apply_operation(&mut document, operation, diagnostics);
    }

    if diagnostics.is_empty() {
        diagnostics.extend(merged_duplicate_diagnostics(&document, &duplicates));
    }

    let outcomes = outcomes.into_iter().flatten().collect::<Vec<_>>();
    if diagnostics.is_empty() {
        (render_accepted_document(&document), outcomes)
    } else {
        (original, outcomes)
    }
}

fn repair_operation_indices(
    // r[impl cairn.sync_archive.delta_merge.identity.duplicate.repair]
    document: &AcceptedDocument,
    operations: &[DeltaOperation],
    duplicates: &std::collections::BTreeMap<String, Vec<String>>,
) -> Vec<usize> {
    operations
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| {
            if operation.kind != DeltaOperationKind::Modified {
                return None;
            }
            let matches = heading_matches(document, &operation.heading);
            let [target] = matches.as_slice() else {
                return None;
            };
            let target_id = &document.blocks[*target].requirement_id;
            // Only a block that already owns a duplicated ID may be retargeted
            // by heading, and only onto a different ID. The merged document is
            // rechecked for duplicates afterwards, so a repair that adopts an
            // existing ID is refused instead of creating a second collision.
            (duplicates.contains_key(target_id) && target_id != &operation.requirement_id)
                .then_some(index)
        })
        .collect()
}

fn heading_matches(document: &AcceptedDocument, heading: &str) -> Vec<usize> {
    document
        .blocks
        .iter()
        .enumerate()
        .filter_map(|(index, block)| (block.heading == heading).then_some(index))
        .collect()
}

fn apply_operation(
    document: &mut AcceptedDocument,
    operation: &DeltaOperation,
    diagnostics: &mut Vec<DeltaMergeDiagnostic>,
) -> Option<DeltaOperationOutcome> {
    let matches: Vec<usize> = document
        .blocks
        .iter()
        .enumerate()
        .filter_map(|(index, block)| {
            (block.requirement_id == operation.requirement_id).then_some(index)
        })
        .collect();
    match operation.kind {
        DeltaOperationKind::Added => {
            if matches.is_empty() {
                document.blocks.push(AcceptedBlock {
                    requirement_id: operation.requirement_id.clone(),
                    heading: operation.heading.clone(),
                    block: operation.block.clone(),
                });
                return Some(outcome(operation, DeltaOperationState::Applied));
            }
            if matches.len() == 1 && document.blocks[matches[0]].block == operation.block {
                return Some(outcome(operation, DeltaOperationState::AlreadyApplied));
            }
            if matches.len() > 1 {
                push_duplicate_accepted_id(document, &operation.requirement_id, diagnostics);
            } else {
                diagnostics.push(diagnostic(
                    "delta_merge.add_conflict",
                    format!(
                        "ADDED operation for `{}` conflicts with existing heading `{}`; use MODIFIED for a reviewed replacement",
                        operation.requirement_id, document.blocks[matches[0]].heading
                    ),
                ));
            }
            None
        }
        DeltaOperationKind::Modified => {
            if matches.len() > 1 {
                push_duplicate_accepted_id(document, &operation.requirement_id, diagnostics);
                return None;
            }
            if let Some(&index) = matches.first() {
                if document.blocks[index].block == operation.block {
                    return Some(outcome(operation, DeltaOperationState::AlreadyApplied));
                }
                document.blocks[index] = AcceptedBlock {
                    requirement_id: operation.requirement_id.clone(),
                    heading: operation.heading.clone(),
                    block: operation.block.clone(),
                };
                return Some(outcome(operation, DeltaOperationState::Applied));
            }
            // The ID is absent. A heading-targeted repair is applied in the
            // first pass; reaching here means exactly one accepted block owns
            // the heading but its ID is not duplicated, so the operation must
            // not silently retarget a different requirement by heading.
            let matches = heading_matches(document, &operation.heading);
            if matches.len() > 1 {
                diagnostics.push(diagnostic(
                    "delta_merge.ambiguous_accepted_heading_target",
                    format!(
                        "MODIFIED operation `{}` targets no requirement ID and its heading matches {} accepted blocks; restore unique requirement IDs before retargeting by heading",
                        operation.heading,
                        matches.len()
                    ),
                ));
            } else {
                diagnostics.push(diagnostic(
                    "delta_merge.unknown_modify_target",
                    format!(
                        "MODIFIED operation targets absent requirement ID `{}`; use ADDED for a new requirement",
                        operation.requirement_id
                    ),
                ));
            }
            None
        }
        DeltaOperationKind::Removed => {
            if matches.len() > 1 {
                push_duplicate_accepted_id(document, &operation.requirement_id, diagnostics);
                return None;
            }
            match matches.first() {
                Some(&index) => {
                    document.blocks.remove(index);
                    Some(outcome(operation, DeltaOperationState::Applied))
                }
                None => Some(outcome(operation, DeltaOperationState::AlreadyApplied)),
            }
        }
    }
}

fn push_duplicate_accepted_id(
    document: &AcceptedDocument,
    requirement_id: &str,
    diagnostics: &mut Vec<DeltaMergeDiagnostic>,
) {
    let headings = document
        .blocks
        .iter()
        .filter(|block| block.requirement_id == requirement_id)
        .map(|block| block.heading.clone())
        .collect::<Vec<_>>();
    diagnostics.push(diagnostic(
        "delta_merge.duplicate_accepted_id",
        format!(
            "accepted requirement ID `{requirement_id}` occurs in conflicting blocks: {}",
            headings.join(", ")
        ),
    ));
}

fn merged_duplicate_diagnostics(
    document: &AcceptedDocument,
    duplicates: &std::collections::BTreeMap<String, Vec<String>>,
) -> Vec<DeltaMergeDiagnostic> {
    let mut locations = std::collections::BTreeMap::<&str, Vec<String>>::new();
    for block in &document.blocks {
        locations
            .entry(&block.requirement_id)
            .or_default()
            .push(block.heading.clone());
    }
    locations
        .into_iter()
        .filter(|(_, headings)| headings.len() > 1)
        .map(|(requirement_id, headings)| {
            if duplicates.contains_key(requirement_id) {
                diagnostic(
                    "delta_merge.duplicate_accepted_id",
                    format!(
                        "accepted requirement ID `{requirement_id}` occurs in conflicting blocks: {}",
                        headings.join(", ")
                    ),
                )
            } else {
                diagnostic(
                    "delta_merge.introduced_duplicate_id",
                    format!(
                        "merged document would contain requirement ID `{requirement_id}` in conflicting blocks: {}",
                        headings.join(", ")
                    ),
                )
            }
        })
        .collect()
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

    const DUPLICATED_FETCH_ADD: &str = "# Fetch Add Specification\n\n## Purpose\n\nDefine fetch-add behavior.\n\n## Requirements\n\n### Requirement: Commutativity\n\nr[fetch-add.associativity] The system MUST return the same result for swapped operands.\n\n#### Scenario: Swapped operands\n\nr[fetch-add.associativity.commutative]\n- GIVEN two operands\n- WHEN they are swapped\n- THEN the result MUST be equal\n\n### Requirement: Associativity\n\nr[fetch-add.associativity] The system MUST group additions consistently.\n\n#### Scenario: Grouped additions\n\nr[fetch-add.associativity.grouped]\n- GIVEN three operands\n- WHEN they are grouped differently\n- THEN the result MUST be equal\n";

    fn duplicate_codes(merged: &DeltaMerge) -> Vec<&str> {
        diagnostic_codes(merged)
    }

    #[test]
    fn modified_by_heading_repairs_a_duplicated_accepted_id() {
        // r[verify cairn.sync_archive.delta_merge.identity.duplicate.repair]
        let delta = "## MODIFIED Requirements\n\n### Requirement: Commutativity\n\nr[fetch-add.commutativity] The system MUST return the same result for swapped operands.\n\n#### Scenario: Swapped operands\n\nr[fetch-add.commutativity.commutative]\n- GIVEN two operands\n- WHEN they are swapped\n- THEN the result MUST be equal\n";
        let merged = merge_delta_spec(Some(DUPLICATED_FETCH_ADD), delta, "fetch-add");
        assert!(!merged.blocked, "{:?}", merged.diagnostics);
        assert!(merged.changed);
        assert_eq!(
            merged
                .markdown
                .matches("r[fetch-add.associativity]")
                .count(),
            1
        );
        assert_eq!(
            merged
                .markdown
                .matches("r[fetch-add.commutativity]")
                .count(),
            1
        );
        assert!(merged.markdown.contains("### Requirement: Commutativity"));
        assert!(merged.markdown.contains("### Requirement: Associativity"));
        assert_eq!(
            merged.outcomes,
            vec![DeltaOperationOutcome {
                kind: DeltaOperationKind::Modified,
                requirement_id: "fetch-add.commutativity".to_owned(),
                state: DeltaOperationState::Applied,
            }]
        );
        let replayed = merge_delta_spec(Some(&merged.markdown), delta, "fetch-add");
        assert!(!replayed.blocked, "{:?}", replayed.diagnostics);
        assert!(!replayed.changed);
        assert_eq!(
            replayed.outcomes[0].state,
            DeltaOperationState::AlreadyApplied
        );
    }

    #[test]
    fn repairs_apply_before_id_targeted_operations_on_the_same_document() {
        // r[verify cairn.sync_archive.delta_merge.identity.duplicate.repair]
        // The ID-targeted modification is listed first and must still resolve
        // after the heading-targeted repair, which runs in a separate pass.
        let delta = "## MODIFIED Requirements\n\n### Requirement: Associativity\n\nr[fetch-add.associativity] The system MUST group additions associatively.\n\n### Requirement: Commutativity\n\nr[fetch-add.commutativity] The system MUST return the same result for swapped operands.\n";
        let merged = merge_delta_spec(Some(DUPLICATED_FETCH_ADD), delta, "fetch-add");
        assert!(!merged.blocked, "{:?}", merged.diagnostics);
        assert_eq!(
            merged
                .markdown
                .matches("r[fetch-add.associativity]")
                .count(),
            1
        );
        assert!(merged
            .markdown
            .contains("The system MUST group additions associatively."));
        assert!(merged.markdown.contains("r[fetch-add.commutativity]"));
    }

    #[test]
    fn repair_that_leaves_a_duplicate_is_refused() {
        // r[verify cairn.sync_archive.delta_merge.identity.duplicate.repair]
        let keeps_duplicate = "## MODIFIED Requirements\n\n### Requirement: Commutativity\n\nr[fetch-add.associativity] The system MUST return a different result for swapped operands.\n";
        let merged = merge_delta_spec(Some(DUPLICATED_FETCH_ADD), keeps_duplicate, "fetch-add");
        assert!(merged.blocked);
        assert!(!merged.changed);
        assert_eq!(merged.markdown, DUPLICATED_FETCH_ADD);
        assert!(duplicate_codes(&merged).contains(&"delta_merge.duplicate_accepted_id"));

        let three_ways = format!(
            "{DUPLICATED_FETCH_ADD}\n### Requirement: Third\n\nr[fetch-add.associativity] The system MUST also collide.\n"
        );
        let repairs_one = "## MODIFIED Requirements\n\n### Requirement: Commutativity\n\nr[fetch-add.commutativity] The system MUST return the same result for swapped operands.\n";
        let merged = merge_delta_spec(Some(&three_ways), repairs_one, "fetch-add");
        assert!(merged.blocked);
        assert_eq!(merged.markdown, three_ways);
        assert!(duplicate_codes(&merged).contains(&"delta_merge.duplicate_accepted_id"));
    }

    #[test]
    fn heading_repair_with_an_ambiguous_accepted_heading_is_refused() {
        // r[verify cairn.sync_archive.delta_merge.identity.duplicate.repair]
        let accepted = format!(
            "{DUPLICATED_FETCH_ADD}\n### Requirement: Commutativity\n\nr[fetch-add.third] The system MUST repeat the heading.\n"
        );
        let delta = "## MODIFIED Requirements\n\n### Requirement: Commutativity\n\nr[fetch-add.commutativity] The system MUST return the same result for swapped operands.\n";
        let merged = merge_delta_spec(Some(&accepted), delta, "fetch-add");
        assert!(merged.blocked);
        assert!(!merged.changed);
        assert_eq!(merged.markdown, accepted);
        assert!(duplicate_codes(&merged).contains(&"delta_merge.ambiguous_accepted_heading_target"));
    }

    #[test]
    fn repair_that_introduces_a_new_duplicate_is_refused() {
        // r[verify cairn.sync_archive.delta_merge.identity.duplicate.repair]
        let accepted = format!(
            "{DUPLICATED_FETCH_ADD}\n### Requirement: Existing\n\nr[fetch-add.existing] The system MUST keep another requirement.\n"
        );
        let delta = "## MODIFIED Requirements\n\n### Requirement: Commutativity\n\nr[fetch-add.existing] The system MUST return the same result for swapped operands.\n";
        let merged = merge_delta_spec(Some(&accepted), delta, "fetch-add");
        assert!(merged.blocked);
        assert!(!merged.changed);
        assert_eq!(merged.markdown, accepted);
        assert!(
            duplicate_codes(&merged).contains(&"delta_merge.introduced_duplicate_id"),
            "{:?}",
            merged.diagnostics
        );
    }

    #[test]
    fn unrelated_delta_against_a_duplicated_document_is_refused() {
        // r[verify cairn.sync_archive.delta_merge.identity.duplicate]
        let delta = "## ADDED Requirements\n\n### Requirement: New\n\nr[fetch-add.new] The system MUST add another requirement.\n";
        let merged = merge_delta_spec(Some(DUPLICATED_FETCH_ADD), delta, "fetch-add");
        assert!(merged.blocked);
        assert!(!merged.changed);
        assert_eq!(merged.markdown, DUPLICATED_FETCH_ADD);
        let duplicate = merged
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "delta_merge.duplicate_accepted_id")
            .expect("duplicate diagnostic");
        assert!(duplicate.message.contains("Commutativity"));
        assert!(duplicate.message.contains("Associativity"));
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
