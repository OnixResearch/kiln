//! Structural preview evidence through the authenticated review shell over a
//! real Git provider and the frozen fixture in
//! `tests/fixtures/structural-preview`: fidelity with the review's structural
//! mode, off by default, explicit query refusal, incompatible subjects and
//! unchanged review state.
//!
//! The fixture is Kiln-authored. Its `expected.json` and `settings.json` are
//! frozen: refresh them only for a reviewed engine, bound or grammar pin
//! change, and bump the matching rules version when outcomes change.
use std::collections::BTreeMap;
use std::error::Error;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use git2::{Oid, Repository, Signature, Time};
use kiln_core::build::{
    BuildCacheKind, BuildEffectPlan, BuildEvaluationObservation, BuildEvaluationOutcome,
    BuildProductObservation, BuildStoreObjectObservation,
};
use kiln_core::change_review::CiFact;
use kiln_core::preview::PreviewDescriptor;
use kiln_core::types::{
    Alias, AttemptId, CandidateId, ContentId, EffectId, PipelineRevisionId, ProviderProfileId,
};
use serde_json::{Value, json};
use tempfile::TempDir;

use super::{Profile, RequestContext, Workspace};
use crate::engine::{ApiResponse, PreviewBuildEvidence, Role};
use crate::preview::observation;

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
const ALICE: &str = "local:author";
const BOB: &str = "local:reviewer";
const MAIN: &str = "refs/heads/main";
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/structural-preview"
);
const ROOT: &str = "/nix/store/00000000000000000000000000000000-cli";
const BUILD_RUN: &str = "build-run-2";
const CI_RUN: &str = "ci-run-2";
/// Observation time, in seconds; records expire 30 days later.
const OBSERVED_AT: u64 = 1_000;
/// Request time, within the records' retention.
const NOW_MS: i64 = 2_000_000;

fn content(byte: &str) -> Result<ContentId> {
    Ok(ContentId::from_blake3_hex(byte.repeat(64)).map_err(|e| e.message)?)
}

/// The fixture side's files, sorted by name; the fixture is flat.
fn side(name: &str) -> Result<Vec<(String, Vec<u8>)>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(Path::new(FIXTURE).join(name))? {
        let entry = entry?;
        let file = entry
            .file_name()
            .into_string()
            .map_err(|_| "fixture file name is not UTF-8")?;
        files.push((file, std::fs::read(entry.path())?));
    }
    files.sort();
    Ok(files)
}

fn frozen(name: &str) -> Result<Value> {
    Ok(serde_json::from_slice(&std::fs::read(
        Path::new(FIXTURE).join(name),
    )?)?)
}

fn commit(
    repository: &Repository,
    parent: Option<Oid>,
    files: &[(String, Vec<u8>)],
) -> Result<Oid> {
    let mut builder = repository.treebuilder(None)?;
    for (path, bytes) in files {
        builder.insert(path, repository.blob(bytes)?, 0o100644)?;
    }
    let tree = repository.find_tree(builder.write()?)?;
    let signature = Signature::new(
        "Fixture",
        "fixture@example.invalid",
        &Time::new(1_700_000_000, 0),
    )?;
    let parent = parent.map(|id| repository.find_commit(id)).transpose()?;
    let parents: Vec<_> = parent.iter().collect();
    Ok(repository.commit(None, &signature, &signature, "fixture", &tree, &parents)?)
}

/// An admitted descriptor for `revision`, invoked with `arguments`.
fn descriptor(
    repository: &str,
    revision: &str,
    run: &str,
    arguments: &[&str],
) -> Result<PreviewDescriptor> {
    let id = "a".repeat(64);
    Ok(serde_json::from_value(json!({
        "version":1,"binding":{"project":"project","repository":repository,"revision":revision,"run_id":run,"attempt_id":"attempt","candidate_id":"candidate","build_receipt":id,"product":"out","artifact_identity":id,"store_identity":id},
        "platform":"x86_64-linux","root_path":ROOT,"executable":format!("{ROOT}/bin/cli"),"arguments":arguments,
        "closure":[{"path":ROOT,"nar_sha256":id,"nar_size":16,"references":[],"signatures":["cache-1:signature"]}],"closure_complete":true,
        "engine":{"protocol":"trynix-startvm-v1","identity":"b".repeat(64),"platform":"x86_64-linux","guest_memory_bytes":536870912,"network_access":false,"supports_exact_closure":true,"supports_disposable_session":true},
        "trust_identity":id,"public_keys":["operator-key"],"issued_at":100,
        "limits":{"runtime_bytes":1024,"run_seconds":60,"output_bytes":64,"expires_at":200,"retain_after_session":false}
    }))?)
}

/// The canonical build record that makes the candidate's observations current.
fn canonical_build() -> Result<PreviewBuildEvidence> {
    let plan = BuildEffectPlan {
        effect_id: EffectId::new("effect").map_err(|e| e.message)?,
        attempt_id: AttemptId::new("attempt").map_err(|e| e.message)?,
        idempotency_key: content("1")?,
        candidate_id: CandidateId::new("candidate").map_err(|e| e.message)?,
        pipeline_revision: PipelineRevisionId::new("pipeline").map_err(|e| e.message)?,
        plan_identity: content("2")?,
        policy_identity: content("3")?,
        provider_profile: ProviderProfileId::new("nix").map_err(|e| e.message)?,
        platform: Alias::new("x86_64-linux").map_err(|e| e.message)?,
        required_products: vec![Alias::new("out").map_err(|e| e.message)?],
        constraints: None,
    };
    let observation = BuildEvaluationObservation {
        effect_id: plan.effect_id.clone(),
        attempt_id: plan.attempt_id.clone(),
        idempotency_key: plan.idempotency_key.clone(),
        candidate_id: plan.candidate_id.clone(),
        pipeline_revision: plan.pipeline_revision.clone(),
        plan_identity: plan.plan_identity.clone(),
        policy_identity: plan.policy_identity.clone(),
        provider_profile: plan.provider_profile.clone(),
        platform: plan.platform.clone(),
        outcome: BuildEvaluationOutcome::Success,
        products: vec![BuildProductObservation {
            name: Alias::new("out").map_err(|e| e.message)?,
            artifact_identity: content("a")?,
            store_identity: content("a")?,
            store: Some(BuildStoreObjectObservation {
                nar_sha256: "b".repeat(64),
                nar_size: 16,
                references: Vec::new(),
                references_complete: true,
                deriver: None,
                content_address: None,
                signatures: Vec::new(),
            }),
        }],
        constraints: None,
        inputs: Vec::new(),
        input_closure_identity: None,
        input_closure_complete: true,
        derivation_identity: None,
        builder_identity: content("4")?,
        worker_identity: content("5")?,
        store_identity: content("6")?,
        cache_kind: BuildCacheKind::Local,
        cache_source_identity: None,
        log_identities: Vec::new(),
        metrics: Vec::new(),
        rebuild: Vec::new(),
        receipt_identity: Some(content("a")?),
    };
    Ok(PreviewBuildEvidence {
        source_run_id: CI_RUN.into(),
        request_id: "request".into(),
        profile_identity: content("7")?,
        node_id: Alias::new("build").map_err(|e| e.message)?,
        plan,
        product_paths: [("out".to_owned(), ROOT.to_owned())].into(),
        observation: Some(observation),
    })
}

struct Fixture {
    _temp: TempDir,
    workspace: Workspace,
    change: String,
    baseline_revision: String,
    candidate_revision: String,
    baseline_commit: String,
    candidate_commit: String,
    /// Retained observation identities: baseline, candidate, candidate run with other arguments.
    observations: [String; 3],
    builds: BTreeMap<String, PreviewBuildEvidence>,
    ci: Vec<CiFact>,
}

impl Fixture {
    fn new() -> Result<Self> {
        let temp = tempfile::tempdir()?;
        std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o700))?;
        let path = temp.path().join("review.git");
        let repository = Repository::init_bare(&path)?;
        let base = commit(&repository, None, &[])?;
        let baseline = commit(&repository, Some(base), &side("baseline")?)?;
        let candidate = commit(&repository, Some(baseline), &side("candidate")?)?;
        repository.reference(MAIN, base, false, "fixture")?;
        let repository_id = kiln_codec::identity_from_bytes(
            path.canonicalize()?.to_str().ok_or("path")?.as_bytes(),
        )?
        .as_str()
        .to_owned();
        let profile: Profile = serde_json::from_value(json!({
            "schema": "kiln.code-review-profile.v1",
            "database": temp.path().join("review.sqlite"),
            "project": "project",
            "provider": {"repository_id": repository_id, "repository_path": path, "target_refs": [MAIN],
                "capabilities": ["inspect", "compare", "register"]},
            "policy": {"id": "policy", "eligible_reviewers": [BOB], "submitters": [ALICE], "minimum_approvals": 1, "require_ci": false},
            "read_principals": [ALICE, BOB], "write_principals": [ALICE, BOB],
            "signature_name": "Review service", "signature_email": "review@example.invalid"
        }))?;
        let directory: PathBuf = temp.path().join("observations");
        std::fs::create_dir(&directory)?;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
        let mut workspace = Workspace::new(Some(profile))?;
        workspace.configure_preview_observations(Some(&directory))?;
        let report = serde_json::to_vec(&json!({
            "phases":[
                {"phase":"artifact discovery","outcome":"ready","elapsed_ms":12.0},
                {"phase":"payload hydration","outcome":"ready","elapsed_ms":20.0},
                {"phase":"guest boot","outcome":"ready","elapsed_ms":30.0},
                {"phase":"program readiness","outcome":"ready","elapsed_ms":40.0}
            ],
            "result":{"status":0,"output":"done","error":null}
        }))?;
        let mut observations = Vec::new();
        for (descriptor_identity, revision, run, arguments) in [
            ("d", baseline, "build-run-1", ["noop"]),
            ("e", candidate, BUILD_RUN, ["noop"]),
            ("f", candidate, BUILD_RUN, ["other"]),
        ] {
            let descriptor = descriptor(&repository_id, &revision.to_string(), run, &arguments)?;
            let record = observation::compose(
                &report,
                &descriptor_identity.repeat(64),
                &descriptor,
                OBSERVED_AT,
            )?;
            observation::retain(&directory, &record)?;
            observations.push(record.identity);
        }
        let observations: [String; 3] =
            observations.try_into().map_err(|_| "three observations")?;
        let ci = vec![CiFact {
            project: "project".into(),
            repository: repository_id,
            revision: candidate.to_string(),
            run_id: CI_RUN.into(),
            ordinal: 1,
            success: true,
            full_run: true,
            clean: true,
            scope_identity: None,
            terminal: true,
            untrusted: None,
            checks: Vec::new(),
        }];
        let mut fixture = Self {
            _temp: temp,
            workspace,
            change: String::new(),
            baseline_revision: String::new(),
            candidate_revision: String::new(),
            baseline_commit: baseline.to_string(),
            candidate_commit: candidate.to_string(),
            observations,
            builds: [(BUILD_RUN.to_owned(), canonical_build()?)].into(),
            ci,
        };
        let created = ok(fixture.post(
            ALICE,
            &json!({"request_id":"create","change_id":null,"expected_version":null,
                "action":{"type":"create","commit":fixture.baseline_commit,"target_ref":MAIN,"title":"Structural preview","description":"","wip":false}}),
        )?)?;
        fixture.change = text(&created, "/change/id");
        let version = fixture.detail()?.pointer("/change/version").cloned();
        ok(fixture.post(
            ALICE,
            &json!({"request_id":"upload","change_id":fixture.change,"expected_version":version,
                "action":{"type":"upload","commit":fixture.candidate_commit}}),
        )?)?;
        let detail = fixture.detail()?;
        fixture.baseline_revision = text(&detail, "/change/revisions/0/id");
        fixture.candidate_revision = text(&detail, "/change/revisions/1/id");
        Ok(fixture)
    }

    fn context(&self, actor: &'static str) -> RequestContext<'_> {
        RequestContext {
            role: Role::Viewer,
            principal: actor,
            now_ms: NOW_MS,
            ci: &self.ci,
        }
    }

    fn post(&self, actor: &'static str, request: &Value) -> Result<ApiResponse> {
        Ok(self.workspace.response_with_preview_builds(
            "POST",
            "/api/code-review/commands",
            &serde_json::to_vec(request)?,
            self.context(actor),
            &self.builds,
        ))
    }

    fn get(&self, path: &str) -> ApiResponse {
        self.workspace.response_with_preview_builds(
            "GET",
            &format!("/api/code-review/changes/{}{path}", self.change),
            &[],
            self.context(BOB),
            &self.builds,
        )
    }

    fn detail(&self) -> Result<Value> {
        ok(self.get(""))
    }

    fn compare(&self, baseline: usize, candidate: usize, extra: &str) -> ApiResponse {
        let identity = |index: usize| self.observations.get(index).cloned().unwrap_or_default();
        self.get(&format!(
            "/preview-comparison?baseline={}&candidate={}{extra}",
            identity(baseline),
            identity(candidate)
        ))
    }
}

fn ok(response: ApiResponse) -> Result<Value> {
    if response.status != 200 {
        return Err(format!(
            "status {}: {}",
            response.status,
            String::from_utf8_lossy(&response.body)
        )
        .into());
    }
    Ok(serde_json::from_slice(&response.body)?)
}

fn text(value: &Value, pointer: &str) -> String {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn at(value: &Value, pointer: &str) -> Value {
    value.pointer(pointer).cloned().unwrap_or(Value::Null)
}

fn file<'a>(files: &'a Value, path: &str) -> Result<&'a Value> {
    files
        .as_array()
        .and_then(|files| files.iter().find(|file| file["path"] == path))
        .ok_or_else(|| format!("{path} missing from {files}").into())
}

// r[verify kiln.preview.structural]
// r[verify kiln.preview.structural.scenario.fidelity]
// r[verify kiln.preview.structural.scenario.off]
#[test]
fn structural_evidence_equals_structural_mode_on_the_frozen_fixture() -> Result {
    let fixture = Fixture::new()?;
    let plain = ok(fixture.compare(0, 1, ""))?;
    let structural = ok(fixture.compare(0, 1, "&structural=1"))?;
    let review = ok(fixture.get(&format!(
        "/diff?revision={}&base={}&mode=structural",
        fixture.candidate_revision, fixture.baseline_revision
    )))?;
    // Off by default: selecting the lane adds only the record.
    let mut without = structural.clone();
    let removed = without
        .as_object_mut()
        .and_then(|object| object.remove("structural"));
    assert!(plain.get("structural").is_none());
    assert_eq!(plain, without);
    let record = removed.ok_or("no structural record")?;
    let evidence = at(&record, "/evidence");
    // The record binds this comparison and both exact revision commits.
    assert_eq!(
        (
            at(&evidence, "/schema"),
            at(&evidence, "/comparison"),
            text(&evidence, "/baseline_revision"),
            text(&evidence, "/candidate_revision"),
            at(&evidence, "/status"),
            at(&evidence, "/reasons"),
        ),
        (
            json!("kiln.preview.structural.v1"),
            at(&structural, "/comparison/identity"),
            fixture.baseline_commit.clone(),
            fixture.candidate_commit.clone(),
            json!("structural"),
            json!([]),
        )
    );
    assert_eq!(
        (at(&review, "/old_commit"), at(&review, "/new_commit")),
        (
            json!(fixture.baseline_commit),
            json!(fixture.candidate_commit)
        )
    );
    assert_eq!(text(&record, "/identity").len(), 64);
    assert!(
        text(&evidence, "/provenance").contains("not CI, review, merge or completion evidence")
    );
    // Fidelity: the same rows or the same named fallback as structural mode.
    let files = at(&evidence, "/files");
    assert_eq!(files, at(&review, "/structural/files"));
    // Frozen outcomes and settings.
    let pretty = |value: &Value| serde_json::to_string_pretty(value).unwrap_or_default();
    assert_eq!(
        files,
        frozen("expected.json")?,
        "refresh expected.json only for a reviewed change:\n{}",
        pretty(&files)
    );
    let settings = at(&evidence, "/settings");
    assert_eq!(
        settings,
        frozen("settings.json")?,
        "refresh settings.json only for a reviewed change:\n{}",
        pretty(&settings)
    );
    // A pure reformat shows no change; a moved unchanged function is moved.
    let reformat = file(&files, "reformat.rs")?;
    assert_eq!(
        (
            &reformat["status"],
            &reformat["hunks"],
            &reformat["moved"],
            &reformat["novel_old_tokens"],
            &reformat["novel_new_tokens"]
        ),
        (
            &json!("structural"),
            &json!([]),
            &json!([]),
            &json!(0),
            &json!(0)
        )
    );
    let moved = file(&files, "moved.rs")?;
    assert_eq!(
        (&moved["status"], &moved["hunks"]),
        (&json!("structural"), &json!([]))
    );
    assert_eq!(
        moved["moved"],
        json!([{"old_start":9,"old_end":11,"new_start":1,"new_end":3}])
    );
    // Negative control: the edit is a change.
    let edited = file(&files, "edited.rs")?;
    assert_eq!(
        (&edited["novel_old_tokens"], &edited["novel_new_tokens"]),
        (&json!(1), &json!(1))
    );
    for (path, reason) in [
        ("broken.rs", "parse_error"),
        ("notes.txt", "unsupported_language"),
        ("pixel.bin", "binary"),
    ] {
        let fallback = file(&files, path)?;
        assert_eq!(
            (&fallback["status"], &fallback["reason"]),
            (&json!("fallback"), &json!(reason))
        );
        assert!(fallback.get("hunks").is_none());
    }
    Ok(())
}

// r[verify kiln.preview.structural]
// r[verify kiln.preview.structural.scenario.off]
// r[verify kiln.preview.structural.scenario.fallback]
#[test]
fn structural_evidence_is_explicit_withheld_for_unlike_cohorts_and_read_only() -> Result {
    let fixture = Fixture::new()?;
    let before = fixture.detail()?;
    for value in ["0", "yes", ""] {
        let response = fixture.compare(0, 1, &format!("&structural={value}"));
        assert_eq!(response.status, 400, "{value}");
        let body: Value = serde_json::from_slice(&response.body)?;
        assert_eq!(
            at(&body, "/code"),
            json!("preview_comparison_query_invalid")
        );
    }
    // Different program arguments: the comparison is incompatible and the
    // record withholds every file instead of comparing unlike cohorts.
    let unlike = ok(fixture.compare(0, 2, "&structural=1"))?;
    assert_eq!(
        (
            at(&unlike, "/comparison/compatible"),
            at(&unlike, "/comparison/reasons")
        ),
        (json!(false), json!(["arguments_differ"]))
    );
    assert_eq!(
        (
            at(&unlike, "/structural/evidence/status"),
            at(&unlike, "/structural/evidence/reasons"),
            at(&unlike, "/structural/evidence/files"),
        ),
        (
            json!("incompatible"),
            json!(["arguments_differ"]),
            json!([])
        )
    );
    // Subjects are never substituted: a historical candidate is refused.
    assert_eq!(fixture.compare(1, 0, "&structural=1").status, 409);
    ok(fixture.compare(0, 1, "&structural=1"))?;
    // Reading evidence changes no review, CI or merge state.
    assert_eq!(fixture.detail()?, before);
    Ok(())
}
