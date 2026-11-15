//! Native exact-revision review and bounded read-only inspection examples.
use std::collections::BTreeSet;
use std::path::Path;

use git2::{Oid, Repository, Signature};
use kiln_core::change_review::{CiCheck, CiFact};
use kiln_web_service::engine::{Engine, Role};
use kiln_web_service::{cobs, code_review, reviews, sources};
use serde_json::{Value, json};

use super::{
    ACTOR, Example, InteractionRequirements, NamedCase, PresentationRequirements, Result, accepted,
    now_ms,
};
const REVIEWER: &str = "viewer:reviewer";
const TARGET: &str = "refs/heads/example-review-target";
const RELEASE: &str = "refs/heads/example-release-target";

pub struct Seed {
    pub workspace: code_review::Workspace,
    pub sources: sources::Browser,
    pub receipts: reviews::Browser,
    pub cobs: cobs::Browser,
    pub examples: Vec<Example>,
}
fn text<'a>(value: &'a Value, path: &str) -> Result<&'a str> {
    Ok(value
        .pointer(path)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Example response lacks {path}"))?)
}
// Read the public, retained run projection. Never accept browser-supplied CI facts.
pub fn ci_facts(engine: &mut Engine) -> Result<Vec<CiFact>> {
    let mut cursor = String::new();
    let mut facts = Vec::new();
    loop {
        let path = if cursor.is_empty() {
            "/api/runs".into()
        } else {
            format!("/api/runs?cursor={cursor}")
        };
        let page = accepted(engine.api("GET", &path, &[], Role::Operator)?)?;
        for run in page
            .get("runs")
            .ok_or("Retained run list absent")?
            .as_array()
            .ok_or("Retained run list absent")?
        {
            let outcome = text(run, "/outcome")?;
            let nodes = run["nodes"].as_array().ok_or("Run nodes absent")?;
            let checks = nodes
                .iter()
                .map(|node| {
                    Ok(CiCheck {
                        name: text(node, "/id")?.into(),
                        kind: text(node, "/kind")?.into(),
                        outcome: match text(node, "/state")? {
                            "Completed" => "pass",
                            "Failed" => "fail",
                            "Cancelled" => "cancelled",
                            "Skipped" => "skipped",
                            _ => "unknown",
                        }
                        .into(),
                        artifact_digest: None,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            facts.push(CiFact {
                project: text(run, "/project_id")?.into(),
                repository: text(run, "/repository/native_id")?.into(),
                revision: text(run, "/source_revision")?.into(),
                run_id: text(run, "/id")?.into(),
                ordinal: run["ordinal"].as_u64().ok_or("Run ordinal absent")?,
                success: outcome == "Success",
                full_run: run["selected_nodes"]
                    .as_array()
                    .ok_or("Run selection absent")?
                    .is_empty(),
                clean: run.get("dirty_snapshot").is_none_or(Value::is_null),
                scope_identity: run
                    .get("scope_identity")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                terminal: matches!(outcome, "Success" | "DomainFailure" | "Cancelled"),
                untrusted: run
                    .get("untrusted")
                    .map(|value| serde_json::from_value(value.clone()))
                    .transpose()?,
                checks,
            });
        }
        match page
            .get("next_cursor")
            .ok_or("Run cursor field absent")?
            .as_str()
        {
            Some(next) if next != cursor => cursor = next.into(),
            Some(_) => return Err("Retained run cursor did not advance".into()),
            None => break,
        }
    }
    Ok(facts)
}
struct Writer<'a> {
    workspace: &'a code_review::Workspace,
    ci: &'a [CiFact],
    sequence: u64,
    time: i64,
}
impl Writer<'_> {
    fn command(&mut self, change: Option<&Value>, action: Value, actor: &str) -> Result<Value> {
        self.sequence += 1;
        let command = json!({"request_id":format!("example-review-{}",self.sequence),"change_id":change.map(|value|&value["id"]),"expected_version":change.map(|value|&value["version"]),"action":action});
        let response = self.workspace.response(
            "POST",
            "/api/code-review/commands",
            &serde_json::to_vec(&command)?,
            code_review::RequestContext {
                role: Role::Operator,
                principal: actor,
                now_ms: self.time,
                ci: self.ci,
            },
        );
        self.time += 1;
        accepted(response)
            .map_err(|error| format!("Review seed operation {}: {error}", self.sequence).into())
    }
    fn change(&mut self, change: &Value, action: Value, actor: &str) -> Result<Value> {
        let result = self.command(Some(change), action, actor)?;
        let id = text(&result, "/change/id")?;
        let detail = accepted(self.workspace.response(
            "GET",
            &format!("/api/code-review/changes/{id}"),
            &[],
            code_review::RequestContext {
                role: Role::Operator,
                principal: actor,
                now_ms: self.time,
                ci: self.ci,
            },
        ))?;
        Ok(detail
            .get("change")
            .ok_or("Review detail omitted its change")?
            .clone())
    }
    fn create(
        &mut self,
        commit: &str,
        target: &str,
        title: &str,
        wip: bool,
        actor: &str,
    ) -> Result<Value> {
        Ok(self.command(None,json!({"type":"create","commit":commit,"target_ref":target,"title":title,"description":"Synthetic exploration change over disposable local source. CI observations are simulated and grant no production acceptance authority.","wip":wip,"authorship":{"kind":"human"}}),actor)?.get("change").ok_or("Review change absent")?.clone())
    }
}
fn review_link(change: &Value) -> Result<String> {
    Ok(format!(
        "/?page=code-review&review_change={}",
        text(change, "/id")?
    ))
}
fn commit_source(repository: &Repository, message: &str, version: u32) -> Result<Oid> {
    let path = repository.workdir().ok_or("Source worktree unavailable")?;
    std::fs::create_dir_all(path.join("src"))?;
    std::fs::create_dir_all(path.join("config"))?;
    std::fs::write(
        path.join("README.md"),
        "# Example release service\n\nA source-browsing example, never executed.\n\n## Explore\nBrowse files, history, commits and exact changes.\n",
    )?;
    std::fs::write(
        path.join("src/lib.rs"),
        format!(
            "/// The example release version.\npub fn version() -> u32 {{\n    {version}\n}}\n\npub fn greeting(name: &str) -> String {{\n    format!(\"Hello, {{name}}\")\n}}\n"
        ),
    )?;
    std::fs::write(
        path.join("config/release.ncl"),
        format!("{{ version = {version}, channel = \"example\", publish = false }}\n"),
    )?;
    if version < 2 {
        std::fs::write(path.join("notes.txt"), "Release notes are illustrative.\n")?;
    } else if path.join("notes.txt").exists() {
        std::fs::rename(path.join("notes.txt"), path.join("release-notes.txt"))?;
    }
    std::fs::write(path.join("sample.bin"), [0_u8, 1, 2, 255, version as u8])?;
    let mut index = repository.index()?;
    index.add_all(["*"], git2::IndexAddOption::DEFAULT, None)?;
    index.update_all(["*"], None)?;
    let tree = repository.find_tree(index.write_tree()?)?;
    let parent = repository
        .head()
        .ok()
        .and_then(|head| head.target())
        .map(|id| repository.find_commit(id))
        .transpose()?;
    let parents: Vec<_> = parent.iter().collect();
    let signature = Signature::now("Synthetic source author", "author@examples.invalid")?;
    let commit = repository.commit(
        Some("refs/heads/main"),
        &signature,
        &signature,
        message,
        &tree,
        &parents,
    )?;
    repository.set_head("refs/heads/main")?;
    Ok(commit)
}
fn raw_operation(
    repository: &Repository,
    kind: &str,
    title: &str,
    parent: Option<Oid>,
) -> Result<Oid> {
    let mut builder = repository.treebuilder(None)?;
    let manifest = serde_json::to_vec(&json!({"typeName":kind,"version":1}))?;
    builder.insert("manifest", repository.blob(&manifest)?, 0o100644)?;
    let action = serde_json::to_vec(
        &json!({"type":if parent.is_some(){"comment"}else{"create"},"title":title,"body":"Synthetic local operation. Raw history is not reduced lifecycle state; signatures and actor authority are not verified by this browser."}),
    )?;
    builder.insert("0", repository.blob(&action)?, 0o100644)?;
    let tree = repository.find_tree(builder.write()?)?;
    let signature = Signature::now("Synthetic raw history", "history@examples.invalid")?;
    let parent = parent.map(|id| repository.find_commit(id)).transpose()?;
    let parents: Vec<_> = parent.iter().collect();
    Ok(repository.commit(None, &signature, &signature, title, &tree, &parents)?)
}

// r[impl kiln.web.exploration.scenario.coverage] r[impl kiln.web.exploration.scenario.local]
pub fn seed(root: &Path, engine: &mut Engine) -> Result<Seed> {
    let ci = ci_facts(engine)?;
    let passed = ci
        .iter()
        .find(|fact| fact.project == "checks" && fact.success && fact.full_run)
        .ok_or("Passing exact CI example absent")?;
    let failed = ci
        .iter()
        .find(|fact| {
            fact.project == "checks" && fact.checks.iter().any(|check| check.outcome == "fail")
        })
        .ok_or("Failed exact CI example absent")?;
    let repository_path = root.join("ci-source");
    let repository = Repository::open(&repository_path)?;
    let success = repository.find_commit(Oid::from_str(&passed.revision)?)?;
    let base = success.parent_id(0)?;
    repository.reference(TARGET, base, false, "Synthetic review target")?;
    repository.reference(RELEASE, base, false, "Synthetic release target")?;
    let repository_id = kiln_codec::identity_from_bytes(
        repository_path
            .canonicalize()?
            .to_str()
            .ok_or("Repository path encoding")?
            .as_bytes(),
    )?
    .as_str()
    .to_owned();
    let policy = kiln_core::change_review::Policy {
        id: "synthetic-exact-ci-policy".into(),
        eligible_reviewers: BTreeSet::from([ACTOR.into(), REVIEWER.into()]),
        submitters: BTreeSet::from([ACTOR.into(), REVIEWER.into()]),
        minimum_approvals: 1,
        require_ci: true,
        ownership: Default::default(),
        optional_ci: Vec::new(),
        agent_votes: kiln_core::change_review::AgentVotes::Advisory,
        required_checks: Vec::new(),
    };
    let profile: code_review::Profile = serde_json::from_value(json!({
        "schema":"kiln.code-review-profile.v1","database":root.join("review.sqlite"),"project":"checks",
        "provider":{"repository_id":repository_id,"repository_path":repository_path,"target_refs":[TARGET,RELEASE],
            "capabilities":["inspect","compare","register","submit","rebase","revert","cherry_pick","apply_suggestions","build_integration","speculative_integration"]},
        "policy":policy,"read_principals":[ACTOR,REVIEWER],"write_principals":[ACTOR,REVIEWER],
        "signature_name":"Synthetic review service","signature_email":"review@examples.invalid",
        "queue":{"schema":"kiln.review-queue.v1","max_entries":100,"max_candidate_builds":16}
    }))?;
    let workspace = code_review::Workspace::new(Some(profile))?;
    let mut writer = Writer {
        workspace: &workspace,
        ci: &ci,
        sequence: 0,
        time: i64::try_from(now_ms()?)? - 3_600_000,
    };
    let merged = writer.create(
        &passed.revision,
        TARGET,
        "Retain exact source identities",
        false,
        ACTOR,
    )?;
    let merged = writer.change(
        &merged,
        json!({"type":"set_reviewer","principal":REVIEWER,"role":"reviewer"}),
        ACTOR,
    )?;
    let merged = writer.change(&merged, json!({"type":"vote","value":"approve"}), REVIEWER)?;
    let merged = writer.change(&merged, json!({"type":"prepare_submit"}), ACTOR)?;
    let candidate = text(&merged, "/candidate/id")?.to_owned();
    let merged = writer.change(
        &merged,
        json!({"type":"submit","candidate_id":candidate}),
        ACTOR,
    )?;
    let open = writer.create(
        &failed.revision,
        TARGET,
        "Explain the version mismatch",
        false,
        REVIEWER,
    )?;
    let open = writer.change(
        &open,
        json!({"type":"set_reviewer","principal":ACTOR,"role":"reviewer"}),
        REVIEWER,
    )?;
    let anchor = json!({"revision":text(&open,"/revisions/0/id")?,"base_commit":passed.revision,"file":"src/lib.rs","side":"revision","start_line":1,"end_line":1});
    let open=writer.change(&open,json!({"type":"start_thread","anchor":anchor,"body":"Please make the manifest and version agree. The linked failing run illustrates the regression.","blocking":true}),ACTOR)?;
    let thread = text(&open, "/threads/0/id")?.to_owned();
    let open=writer.change(&open,json!({"type":"reply","thread":thread,"body":"I retained the exact source and will upload a corrected revision."}),REVIEWER)?;
    let next = ci
        .iter()
        .find(|fact| fact.project == "docs" && fact.success)
        .ok_or("Second review revision absent")?;
    let open = writer.change(
        &open,
        json!({"type":"upload","commit":next.revision}),
        REVIEWER,
    )?;
    let open = writer.change(
        &open,
        json!({"type":"vote","value":"needs_revision"}),
        ACTOR,
    )?;
    let revision = open
        .get("revisions")
        .ok_or("Review revisions absent")?
        .as_array()
        .and_then(|items| items.last())
        .ok_or("Current review revision absent")?;
    let anchor = json!({"revision":revision["id"],"base_commit":passed.revision,"file":"src/lib.rs","side":"revision","start_line":1,"end_line":1});
    let drafted=writer.command(Some(&open),json!({"type":"save_draft","anchor":anchor,"thread":null,"body":"Private draft: explain why this version is correct before publishing approval.","blocking":false}),ACTOR)?;
    let open = drafted.get("change").ok_or("Review change absent")?.clone();
    let open = writer.change(
        &open,
        json!({"type":"mark_reviewed","file":"README.md","reviewed":true}),
        ACTOR,
    )?;
    writer.command(None,json!({"type":"save_view","name":"Needs discussion","query":{"kind":"unresolved","text":"","status":"open"}}),ACTOR)?;
    let wip = writer.create(
        &failed.revision,
        TARGET,
        "Draft: release-note rendering",
        true,
        ACTOR,
    )?;
    let child = writer.create(
        &next.revision,
        TARGET,
        "Dependent: document the revised version",
        false,
        ACTOR,
    )?;
    let current = open
        .get("revisions")
        .ok_or("Review revisions absent")?
        .as_array()
        .and_then(|items| items.last())
        .ok_or("Current revision absent")?;
    let child=writer.change(&child,json!({"type":"set_dependencies","dependencies":[{"change":text(&open,"/id")?,"revision":text(current,"/id")?}]}),ACTOR)?;
    let abandoned = writer.create(
        &failed.revision,
        TARGET,
        "Abandoned: hide unknown outcomes",
        false,
        ACTOR,
    )?;
    let abandoned = writer.change(&abandoned, json!({"type":"abandon"}), ACTOR)?;
    let agent=writer.command(None,json!({"type":"create","commit":next.revision,"target_ref":TARGET,"title":"Agent-authored example: readable evidence","description":"Self-declared illustrative harness/model metadata, not a real model run.","wip":true,"authorship":{"kind":"agent","harness":"example-harness","model":"example-model"}}),ACTOR)?.get("change").ok_or("Review change absent")?.clone();
    let queued = writer.create(
        &passed.revision,
        RELEASE,
        "Queued local backport of exact identity handling",
        false,
        REVIEWER,
    )?;
    let queued = writer.change(
        &queued,
        json!({"type":"set_reviewer","principal":ACTOR,"role":"reviewer"}),
        REVIEWER,
    )?;
    let queued = writer.change(&queued, json!({"type":"vote","value":"approve"}), ACTOR)?;
    let queued = writer.change(
        &queued,
        json!({"type":"enqueue","expected_target":base.to_string()}),
        ACTOR,
    )?;
    let open_id = text(&open, "/id")?;
    let revisions = open
        .get("revisions")
        .ok_or("Review revisions absent")?
        .as_array()
        .ok_or("Revisions absent")?;
    let from = text(revisions.first().ok_or("First revision absent")?, "/id")?;
    let to = text(revisions.last().ok_or("Revision absent")?, "/id")?;
    let mut examples = vec![
        Example::new(
            "Code review: exact revisions, discussion and submit requirements",
            review_link(&open)?,
            "You are the requested reviewer. Inspect failed/missing exact CI, blocking discussion, a private draft and viewed files; publish feedback or vote. No operator source is touched.",
        ),
        Example::new(
            "Interdiff: changes since the previous revision",
            format!("/?page=code-review&review_change={open_id}&review_interdiff={from}..{to}"),
            "Compare two exact review revisions with base movement removed. Switch unified/split, whitespace and structural diff modes.",
        ),
        Example::new(
            "Structural review and line permalinks",
            format!(
                "/?page=code-review&review_change={open_id}&review_revision={to}&review_file=src%2Flib.rs&review_mode=structural"
            ),
            "Inspect syntax-aware Rust changes, exact line anchors and the retained historical discussion.",
        ),
        Example::new(
            "Awaiting my review and personal saved review queries",
            "/?page=code-review&review_view=awaiting_review",
            "The operator is an assigned reviewer. Try saved Needs discussion, notifications and the personal review filters.",
        ),
        Example::new(
            "WIP change owned by you",
            review_link(&wip)?,
            "Edit title/description, mark ready, contribute another exact local commit or abandon/restore the disposable change.",
        ),
        Example::new(
            "Review dependency stack",
            review_link(&child)?,
            "Inspect the pinned parent revision and why an open dependency blocks submission.",
        ),
        Example::new(
            "Submitted exact-CI candidate",
            review_link(&merged)?,
            "Inspect approvals, exact passing simulated CI and the locally submitted candidate. The target ref belongs only to this temporary repository.",
        ),
        Example::new(
            "Abandoned change and restore",
            review_link(&abandoned)?,
            "Read retained history and restore the local change without deleting its earlier state.",
        ),
        Example::new(
            "Self-declared agent authorship",
            review_link(&agent)?,
            "Inspect the illustrative harness/model declaration. It is not authenticated model execution.",
        ),
        Example::new(
            "Review merge-queue entry",
            review_link(&queued)?,
            "Inspect the queue registration and exact target; dequeue it locally. No production queue worker or remote forge is involved.",
        ),
    ];
    examples.push(Example::named(
        NamedCase {
            case_id: "case.review.multiple-revisions".into(),
            recipe: ("explore.case.review.multiple-revisions".into(), "v1".into()),
            presentation: PresentationRequirements {
                theme: "system".into(),
                density: "comfortable".into(),
                viewport: "wide".into(),
                themes: vec!["light".into(), "dark".into(), "system".into()],
                densities: vec!["comfortable".into(), "compact".into()],
                viewports: vec!["narrow".into(), "wide".into()],
            },
            interaction: InteractionRequirements { keyboard: true, narrow: true, dark: true },
            subjects: vec![
                ("review_change".into(), open_id.to_owned()),
                ("from_revision".into(), from.to_owned()),
                ("to_revision".into(), to.to_owned()),
            ],
            seeder_source: include_bytes!("review.rs").to_vec(),
        },
        "Code review: multiple revisions",
        review_link(&open)?,
        "Inspect the review change with multiple exact revisions and interdiff using actual seeded identities.",
    ));
    let source_path = root.join("source-examples");
    let source = Repository::init(&source_path)?;
    let first = commit_source(&source, "Initial example service", 1)?;
    let second = commit_source(&source, "Rename release notes and update version", 2)?;
    let latest = commit_source(&source, "Explain the release helper", 3)?;
    source.reference("refs/heads/stable", first, false, "Synthetic stable branch")?;
    source.reference("refs/tags/v0.1.0-example", second, false, "Synthetic tag")?;
    let sources = sources::Browser::new(Some(sources::Profile {
        schema: "kiln.web-sources.v1".into(),
        sources: vec![
            sources::SourcePolicy {
                id: "example-service".into(),
                label: "Example release service · synthetic Git".into(),
                kind: "git".into(),
                path: source_path,
                channel: None,
                executable: None,
            },
            sources::SourcePolicy {
                id: "ci-inputs".into(),
                label: "Exact CI/review inputs · synthetic Git".into(),
                kind: "git".into(),
                path: root.join("ci-source"),
                channel: None,
                executable: None,
            },
        ],
    }))?;
    for (title, suffix, note) in [
        (
            "Source repositories, refs and directories",
            String::new(),
            "Choose a local Git repository, branch/tag and directory. Source browsing performs no checkout, fetch or write.",
        ),
        (
            "Source file: highlighted Rust and exact line",
            format!("&rev={latest}&spath=src&sfile=src%2Flib.rs&sline=3"),
            "Read the exact file and copy a line permalink; inspect its history and commit.",
        ),
        (
            "Source history and exact commits",
            "&sview=history".into(),
            "Browse three retained commits and follow the commit/file links.",
        ),
        (
            "Source changes: rename and binary file",
            format!("&rev={second}&sview=changes"),
            "Inspect the exact commit diff, a renamed text file and the explicit binary-file state.",
        ),
        (
            "Source configuration and Markdown",
            format!("&rev={latest}&spath=config&sfile=config%2Frelease.ncl"),
            "Read Nickel source and return to the Markdown README; neither file is evaluated/executed.",
        ),
    ] {
        examples.push(Example::new(
            title,
            format!("/?page=source&src=example-service{suffix}"),
            note,
        ));
    }
    let receipt_directory = root.join("patchgate-receipts");
    std::fs::create_dir(&receipt_directory)?;
    let decisions = [
        "accepted",
        "rejected",
        "escalated",
        "blocked",
        "waiting",
        "pending",
        "removed",
        "reverted",
        "unknown",
    ];
    for (index, decision) in decisions.iter().enumerate() {
        let receipt = super::receipt::document(decision, index)?;
        let name = format!("example-{decision}.json");
        std::fs::write(
            receipt_directory.join(&name),
            serde_json::to_vec_pretty(&receipt)?,
        )?;
        examples.push(Example::new(format!("Patchgate decision: {decision}"),format!("/?page=reviews&receipt={name}"),"Inspect the decision, producer-native reason/time/paths, partial coverage, exact retained bytes and non-claims. This is a synthetic producer record, not a Patchgate run."));
    }
    let receipts = reviews::Browser::new(Some(reviews::Profile {
        schema: "kiln.web-reviews.v1".into(),
        label: "Synthetic Patchgate decision examples".into(),
        directory: receipt_directory,
    }))?;
    let cob_path = root.join("raw-history.git");
    let raw = Repository::init_bare(&cob_path)?;
    for (kind, title) in [
        ("xyz.radicle.issue", "Example issue discussion"),
        (
            "xyz.onixresearch.cairn-change",
            "Example lifecycle proposal",
        ),
        ("xyz.radicle.patch", "Example exact-source patch"),
    ] {
        let initial = raw_operation(&raw, kind, title, None)?;
        let update = raw_operation(
            &raw,
            kind,
            "Retain the follow-up explanation",
            Some(initial),
        )?;
        let concurrent = raw_operation(&raw, kind, "Independent local observation", Some(initial))?;
        raw.reference(
            &format!("refs/cobs/{kind}/{initial}"),
            update,
            false,
            "Synthetic raw history",
        )?;
        raw.reference(
            &format!("refs/namespaces/example-peer/refs/cobs/{kind}/{initial}"),
            concurrent,
            false,
            "Synthetic peer history",
        )?;
        examples.push(Example::new(format!("Raw collaboration: {title}"),format!("/?page=cobs&cob_repository=examples&cob_type={kind}&cob={initial}&cob_repo=examples&cob_kind={kind}"),"Inspect branches, ref tips and raw operations. These deliberately unverified local records are not Cairn-reduced lifecycle state or remote completeness evidence."));
    }
    let cobs = cobs::Browser::new(Some(cobs::Profile {
        schema: "kiln.web-cobs.v1".into(),
        repositories: vec![cobs::RepositoryPolicy {
            id: "examples".into(),
            label: "Synthetic collaboration histories".into(),
            repository_id: "example-local-history".into(),
            git_directory: cob_path,
        }],
    }))?;
    Ok(Seed {
        workspace,
        sources,
        receipts,
        cobs,
        examples,
    })
}
