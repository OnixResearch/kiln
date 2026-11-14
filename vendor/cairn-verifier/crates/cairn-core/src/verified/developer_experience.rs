use crate::{blake3_hex, validate_change_name};

pub const TEMPLATE_POLICY_ID: &str = "cairn-default.developer_experience.templates.v1";
pub const SCAFFOLD_POLICY_ID: &str = "cairn-default.developer_experience.prompt_scaffold.v1";
pub const DISCOVERY_POLICY_ID: &str =
    "cairn-default.developer_experience.reverse_spec_discovery.v1";
pub const DISCOVERY_MAX_FILES: usize = 32;
pub const DISCOVERY_MAX_BYTES: usize = 65_536;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateProfile {
    pub id: &'static str,
    pub description: &'static str,
    pub files: Vec<PlannedFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePlanEntry {
    pub path: String,
    pub content_hash: String,
    pub exists: bool,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePlan {
    pub valid: bool,
    pub issues: Vec<String>,
    pub files: Vec<FilePlanEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScaffoldPlan {
    pub valid: bool,
    pub issues: Vec<String>,
    pub change: String,
    pub domain: String,
    pub input_hash: String,
    pub files: Vec<PlannedFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoverySnapshot {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryCandidate {
    pub id: String,
    pub evidence_path: String,
    pub summary: String,
    pub review_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryPlan {
    pub valid: bool,
    pub issues: Vec<String>,
    pub input_hash: String,
    pub selected_paths: Vec<String>,
    pub skipped_paths: Vec<String>,
    pub candidates: Vec<DiscoveryCandidate>,
}

pub fn onboarding_templates() -> Vec<TemplateProfile> {
    // r[impl cairn.developer_experience.onboarding_templates.catalog]
    vec![
        TemplateProfile {
            id: "agent-workflow",
            description: "Minimal agent instructions and Cairn quickstart for AI-assisted lifecycle work.",
            files: vec![file(
                "docs/cairn-agent-workflow.md",
                "# Cairn agent workflow\n\n1. Create or scaffold a native Cairn change.\n2. Run `cairn validate --root .`.\n3. Run proposal, design, and tasks gates before implementation.\n4. Sync and archive only after verification.\n",
            )],
        },
        TemplateProfile {
            id: "generic-cli",
            description: "Generic CLI project quickstart with native Cairn lifecycle commands.",
            files: vec![file(
                "docs/cairn-quickstart.md",
                "# Cairn quickstart\n\n```bash\ncairn init --root .\ncairn change create first-change --root .\ncairn validate --root .\n```\n",
            )],
        },
        TemplateProfile {
            id: "rust-nix",
            description: "Rust/Nix repository quickstart that keeps Cairn checks separate from implementation gates.",
            files: vec![file(
                "docs/cairn-rust-nix.md",
                "# Cairn for Rust/Nix\n\nUse `nix develop -c cargo test --workspace` for implementation checks and `cairn validate --root .` for lifecycle checks.\n",
            )],
        },
        TemplateProfile {
            id: "docs-lifecycle",
            // r[impl cairn.developer_experience.onboarding_template_catalog_expansion.catalog]
            description: "Docs-only repository quickstart for reviewed changes without code-build assumptions.",
            files: vec![file(
                "docs/cairn-docs-lifecycle.md",
                "# Cairn for docs-only repositories\n\nUse `cairn change create <slug> --root .` to propose documentation changes, `cairn gate proposal <slug> --root .` / `cairn gate design <slug> --root .` / `cairn gate tasks <slug> --root .` for review, and `cairn sync <slug> --root . --execute` only after the documentation update is verified.\n",
            )],
        },
        TemplateProfile {
            id: "release-evidence",
            description: "Release-evidence quickstart for local/offline readiness receipts and evidence indexes.",
            files: vec![file(
                "docs/cairn-release-evidence.md",
                "# Cairn release evidence\n\nUse `cairn release-readiness --root .` to inspect local readiness, `cairn release-evidence index --root . --out target/release-evidence.md` to compose a current-head evidence page, and evidence registry commands for local/offline publication materialization. This template does not configure remote publishing, credential handling, hosted UI, or signing.\n",
            )],
        },
    ]
}

pub fn plan_template_application(template_id: &str, existing_paths: &[String]) -> FilePlan {
    // r[impl cairn.developer_experience.onboarding_templates.apply]
    let Some(template) = onboarding_templates()
        .into_iter()
        .find(|candidate| candidate.id == template_id)
    else {
        return FilePlan {
            valid: false,
            issues: vec![format!("unknown onboarding template: {template_id}")],
            files: Vec::new(),
        };
    };
    build_file_plan(template.files, existing_paths, false)
}

pub fn plan_prompt_scaffold(
    change: &str,
    domain: &str,
    request: &str,
    existing_paths: &[String],
) -> ScaffoldPlan {
    // r[impl cairn.developer_experience.prompt_to_change_scaffold.planner]
    // r[impl cairn.developer_experience.prompt_to_change_scaffold.gate_ready]
    let mut issues = Vec::new();
    if let Err(issue) = validate_change_name(change) {
        issues.push(issue);
    }
    let trimmed_request = request.trim();
    if trimmed_request.is_empty() {
        issues.push("request material must not be empty".to_string());
    }
    let domain = normalize_domain(domain);
    let prefix = format!("cairn.developer_experience.{change}").replace('-', "_");
    let input_hash = blake3_hex(trimmed_request.as_bytes());
    let base = format!(".cairn/changes/{change}");
    let files = if issues.is_empty() {
        vec![
            file(
                &format!("{base}/proposal.md"),
                &proposal(change, trimmed_request),
            ),
            file(
                &format!("{base}/design.md"),
                &design(change, trimmed_request),
            ),
            file(&format!("{base}/tasks.md"), &tasks(&prefix)),
            file(
                &format!("{base}/specs/{domain}/spec.md"),
                &spec(&domain, &prefix, trimmed_request),
            ),
        ]
    } else {
        Vec::new()
    };
    let file_plan = build_file_plan(files.clone(), existing_paths, true);
    issues.extend(file_plan.issues);
    ScaffoldPlan {
        valid: issues.is_empty(),
        issues,
        change: change.to_string(),
        domain,
        input_hash,
        files,
    }
}

pub fn plan_reverse_spec_discovery(snapshots: Vec<DiscoverySnapshot>) -> DiscoveryPlan {
    // r[impl cairn.developer_experience.reverse_spec_discovery.planner]
    let mut issues = Vec::with_capacity(2);
    let snapshot_count = snapshots.len();
    let mut selected = Vec::with_capacity(snapshot_count.min(DISCOVERY_MAX_FILES));
    let mut skipped = Vec::with_capacity(snapshot_count.min(DISCOVERY_MAX_FILES));
    let mut total_bytes = 0usize;
    if snapshots.len() > DISCOVERY_MAX_FILES {
        issues.push(format!(
            "discovery input exceeds max file count {DISCOVERY_MAX_FILES}"
        ));
    }
    let mut snapshots = snapshots;
    snapshots.sort_by(|left, right| left.path.cmp(&right.path));
    let mut candidates = Vec::with_capacity(snapshot_count.min(DISCOVERY_MAX_FILES));
    for snapshot in snapshots {
        if !is_safe_relative_path(&snapshot.path) {
            skipped.push(snapshot.path);
            continue;
        }
        total_bytes = total_bytes.saturating_add(snapshot.content.len());
        if total_bytes > DISCOVERY_MAX_BYTES {
            issues.push(format!(
                "discovery input exceeds max bytes {DISCOVERY_MAX_BYTES}"
            ));
            break;
        }
        selected.push(snapshot.path.clone());
        if let Some(summary) = candidate_summary(&snapshot.content) {
            let id = candidate_id(candidates.len() + 1, &snapshot.path);
            candidates.push(DiscoveryCandidate {
                id,
                evidence_path: snapshot.path,
                summary,
                review_status: "review-needed".to_string(),
            });
        }
    }
    let input_hash = blake3_hex(selected.join("\n").as_bytes());
    DiscoveryPlan {
        valid: issues.is_empty(),
        issues,
        input_hash,
        selected_paths: selected,
        skipped_paths: skipped,
        candidates,
    }
}

fn build_file_plan(
    files: Vec<PlannedFile>,
    existing_paths: &[String],
    allow_change_paths: bool,
) -> FilePlan {
    let mut issues = Vec::with_capacity(files.len());
    let mut entries = Vec::with_capacity(files.len());
    for planned in files {
        if !is_safe_relative_path(&planned.path) {
            issues.push(format!("unsafe template path: {}", planned.path));
            continue;
        }
        if !allow_change_paths && planned.path.starts_with(".cairn/specs/") {
            issues.push(format!(
                "template cannot write accepted spec path: {}",
                planned.path
            ));
            continue;
        }
        let path_exists = existing_paths.iter().any(|path| path == &planned.path);
        let action = if path_exists {
            "skip_existing"
        } else {
            "create"
        }
        .to_string();
        entries.push(FilePlanEntry {
            path: planned.path,
            content_hash: blake3_hex(planned.content.as_bytes()),
            exists: path_exists,
            action,
        });
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    FilePlan {
        valid: issues.is_empty(),
        issues,
        files: entries,
    }
}

fn file(path: &str, content: &str) -> PlannedFile {
    PlannedFile {
        path: path.to_string(),
        content: content.to_string(),
    }
}

fn normalize_domain(domain: &str) -> String {
    let mut normalized = domain
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while normalized.contains("--") {
        normalized = normalized.replace("--", "-");
    }
    normalized = normalized.trim_matches('-').to_string();
    if normalized.is_empty() {
        "developer-experience".to_string()
    } else {
        normalized
    }
}

fn is_safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with('\\')
        && !path
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
}

fn proposal(change: &str, request: &str) -> String {
    format!("## Why\n\n{request}\n\n## What Changes\n\n- Add a deterministic Cairn change package for `{change}`.\n\n## Impact\n\n- **Files**: TBD during implementation.\n- **Testing**: Validate, gate, and focused regression checks.\n")
}

fn design(change: &str, request: &str) -> String {
    format!("## Context\n\nRequest material for `{change}`:\n\n> {request}\n\n## Decisions\n\n### 1. Deterministic implementation slice\n\n**Choice:** Keep the implementation narrow and receipt-backed.\n\n**Rationale:** Cairn changes should be independently reviewable and verifiable.\n\n## Risks / Trade-offs\n\n- The scaffold structures intent but does not prove semantic completeness.\n")
}

fn tasks(prefix: &str) -> String {
    format!("## Phase 1: Implementation\n\n- [ ] [serial] Implement the deterministic core behavior. r[{prefix}.core]\n- [ ] [serial] Add CLI/docs surfaces for operator use. r[{prefix}.cli]\n- [ ] [serial] Add positive and negative verification coverage. r[{prefix}.verification]\n- [ ] [serial] Verify, sync, and archive the change. r[{prefix}.verification]\n")
}

fn spec(domain: &str, prefix: &str, request: &str) -> String {
    format!("# {domain} Specification\n\n## Purpose\n\nScaffolded from explicit request material: {request}\n\n## Requirements\n\n### Requirement: Deterministic core behavior\nr[{prefix}.core] The system MUST implement the requested behavior through deterministic, reviewable logic.\n\n#### Scenario: Evaluate core behavior\nr[{prefix}.core.evaluate]\n- GIVEN reviewed inputs for this change\n- WHEN Cairn evaluates the core behavior\n- THEN the result is stable for equivalent inputs\n- AND hidden network, clock, or environment state is not required\n\n### Requirement: Operator CLI surface\nr[{prefix}.cli] The system MUST expose the behavior through an operator-visible Cairn surface.\n\n#### Scenario: Use operator surface\nr[{prefix}.cli.use]\n- GIVEN an operator invokes the documented surface\n- WHEN Cairn completes the command\n- THEN the command emits a deterministic receipt\n- AND the receipt includes stable issue information for invalid input\n\n### Requirement: Verification coverage\nr[{prefix}.verification] The system MUST verify the change with positive and negative regression coverage.\n\n#### Scenario: Run verification\nr[{prefix}.verification.run]\n- GIVEN the repository verification suite\n- WHEN focused checks for `{domain}` run\n- THEN valid fixtures pass\n- AND invalid fixtures fail closed with deterministic issue codes\n")
}

fn candidate_summary(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed
            .strip_prefix("pub fn ")
            .or_else(|| trimmed.strip_prefix("fn "))
        {
            let name = rest.split(['(', '<', ' ']).next().unwrap_or("behavior");
            return Some(format!("Candidate behavior from Rust function `{name}`"));
        }
        if let Some(rest) = trimmed.strip_prefix("### ") {
            return Some(format!("Candidate documented capability `{}`", rest.trim()));
        }
    }
    None
}

fn candidate_id(index: usize, path: &str) -> String {
    let stem = path.rsplit('/').next().unwrap_or("candidate");
    let slug = normalize_domain(stem.trim_end_matches(".rs").trim_end_matches(".md"));
    format!("candidate.{index:03}.{slug}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_plan_rejects_unknown_template() {
        let plan = plan_template_application("missing", &[]);
        assert!(!plan.valid);
        assert!(plan.issues[0].contains("unknown onboarding template"));
    }

    #[test]
    fn scaffold_plan_generates_matching_task_refs() {
        let plan = plan_prompt_scaffold("add-thing", "Developer Experience", "Add the thing", &[]);
        assert!(plan.valid, "{:?}", plan.issues);
        let tasks = plan
            .files
            .iter()
            .find(|file| file.path.ends_with("tasks.md"))
            .unwrap();
        let spec = plan
            .files
            .iter()
            .find(|file| file.path.ends_with("spec.md"))
            .unwrap();
        assert!(tasks
            .content
            .contains("r[cairn.developer_experience.add_thing.core]"));
        assert!(spec
            .content
            .contains("r[cairn.developer_experience.add_thing.core]"));
    }

    #[test]
    fn discovery_marks_candidates_review_needed() {
        let plan = plan_reverse_spec_discovery(vec![DiscoverySnapshot {
            path: "src/lib.rs".to_string(),
            content: "pub fn run() {}".to_string(),
        }]);
        assert!(plan.valid);
        assert_eq!(plan.candidates[0].review_status, "review-needed");
    }
}
