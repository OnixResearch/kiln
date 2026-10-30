use serde_json::json;
use tempfile::TempDir;

use super::*;
use crate::events::{Admission, admit_notify, admit_trigger};
use crate::nsid;
use crate::pipeline::*;
use crate::resolver::StaticKeyResolver;
use crate::service::TangledService;
use crate::store::{SqliteStore, StoreLimits};
use crate::xrpc::{Method, XrpcRequest, XrpcResponse};

const NOW: i64 = 1_800_000_000;
const WORKER_PASSES: usize = 12;

type Service = TangledService<FixtureSource, FixtureWorkflow, FixtureStatus, StaticKeyResolver>;

struct Harness {
    _root: TempDir,
    service: Service,
    workflow: FixtureWorkflow,
    status: FixtureStatus,
    nonce: u64,
}

fn store(root: &TempDir) -> SqliteStore {
    ok(SqliteStore::open(
        &root.path().join("tangled.sqlite"),
        StoreLimits {
            maximum_pipelines: 16,
            maximum_tokens: 64,
            maximum_record_bytes: 1_048_576,
        },
    ))
}

fn harness_with(workflow: FixtureWorkflow, status: FixtureStatus) -> Harness {
    let root = ok(TempDir::new());
    let source = FixtureSource {
        head: Some(object('f', 64)),
        ..FixtureSource::default()
    };
    let service = TangledService::new(
        profile(),
        store(&root),
        source,
        workflow.clone(),
        status.clone(),
        StaticKeyResolver::new(issuer_keys()),
    );
    Harness {
        _root: root,
        service,
        workflow,
        status,
        nonce: 0,
    }
}

fn harness() -> Harness {
    harness_with(FixtureWorkflow::new(), FixtureStatus::default())
}

impl Harness {
    fn token(&mut self, issuer: &str, method: &str) -> String {
        self.nonce += 1;
        let signer = if issuer == KNOT {
            Signer::K256(knot_key())
        } else {
            Signer::P256(actor_key())
        };
        sign_token(
            &signer,
            &claims(issuer, method, NOW, &format!("nonce-{}", self.nonce)),
        )
    }

    fn post(
        &mut self,
        issuer: &str,
        method: &str,
        body: &serde_json::Value,
    ) -> (u16, serde_json::Value) {
        let token = self.token(issuer, method);
        self.post_with(
            Some(format!("Bearer {token}")),
            method,
            ok(serde_json::to_vec(body)),
        )
    }

    fn post_with(
        &mut self,
        authorization: Option<String>,
        method: &str,
        body: Vec<u8>,
    ) -> (u16, serde_json::Value) {
        let request = XrpcRequest {
            method: Method::Post,
            nsid: method.into(),
            query: vec![],
            authorization,
            body,
        };
        decode(self.service.handle(&request, NOW))
    }

    fn get(&mut self, method: &str, query: &[(&str, &str)]) -> (u16, serde_json::Value) {
        let request = XrpcRequest {
            method: Method::Get,
            nsid: method.into(),
            query: query
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            authorization: None,
            body: vec![],
        };
        decode(self.service.handle(&request, NOW))
    }

    /// Worker passes until nothing is left; each pass visits every pipeline with work.
    fn drain(&mut self) {
        for _ in 0..WORKER_PASSES {
            while ok(self.service.tick(NOW + 1)) {}
        }
    }

    fn pipeline(&mut self, id: &str) -> serde_json::Value {
        let (status, view) = self.get(nsid::GET_PIPELINE, &[("repo", REPO), ("pipeline", id)]);
        assert_eq!(status, 200, "{view}");
        view
    }
}

fn decode(response: XrpcResponse) -> (u16, serde_json::Value) {
    (response.status, ok(serde_json::from_slice(&response.body)))
}

fn push(old: &str, new: &str) -> serde_json::Value {
    json!({ "repo": REPO, "event": { "$type": nsid::PUSH, "repo": REPO, "ref": "refs/heads/main", "oldSha": old, "newSha": new, "actor": TRUSTED } })
}

fn first_pipeline(body: &serde_json::Value) -> String {
    some(field(body, "/ci/pipelines/0").as_str()).to_string()
}

fn statuses(view: &serde_json::Value) -> Vec<String> {
    some(field(view, "/workflows").as_array())
        .iter()
        .map(|w| some(field(w, "/status").as_str()).to_string())
        .collect()
}

// r[verify kiln.tangled.source.scenario.exact] r[verify kiln.tangled.status.scenario.published] r[verify kiln.tangled.replay.scenario.push]
#[test]
fn knot_push_runs_exact_revision_publishes_status_and_dedupes_replay() {
    let mut h = harness();
    let (old, new) = ("a".repeat(64), "b".repeat(64));
    let (status, body) = h.post(KNOT, nsid::NOTIFY_EVENT, &push(&old, &new));
    assert_eq!(status, 200, "{body}");
    let id = first_pipeline(&body);
    assert!(id.starts_with(PIPELINE_ID_PREFIX));
    let message = some(field(&body, "/ci/message").as_str());
    assert!(
        message.contains("cache.yml") && message.contains("caches are not mapped"),
        "{message}"
    );

    h.drain();
    let plans = h.workflow.dispatched();
    assert_eq!(
        plans.len(),
        1,
        "only test.yml matches a main push touching src/"
    );
    let plan = some(plans.first());
    let source = some(plan.source.as_ref());
    assert_eq!(source.revision.native_id.as_str(), new);
    assert_eq!(source.repository.native_id.as_str(), REPO);
    assert_eq!(plan.workflow_policy_id.as_str(), "tangled-trusted-v1");

    let view = h.pipeline(&id);
    assert_eq!(statuses(&view), vec!["success"]);
    assert_eq!(field(&view, "/commit"), new.as_str());
    assert_eq!(field(&view, "/trigger/$type"), nsid::PUSH);
    assert_eq!(
        field(&view, "/workflows/0/definition/source/commit"),
        new.as_str()
    );
    let delivered = h.status.views();
    assert_eq!(some(delivered.last()), &view, "the final view is published");
    assert_eq!(statuses(some(delivered.first())), vec!["running"]);

    // Redelivery with a fresh token returns the same pipeline and starts nothing.
    let (status, again) = h.post(KNOT, nsid::NOTIFY_EVENT, &push(&old, &new));
    assert_eq!(status, 200);
    assert_eq!(first_pipeline(&again), id);
    h.drain();
    assert_eq!(h.workflow.dispatched().len(), 1);
    assert_eq!(ok(h.service.store().pipeline_count()), 1);
    assert_eq!(ok(h.service.store().run_count()), 1);
}

// r[verify kiln.tangled.auth.scenario.bad-signature] r[verify kiln.tangled.replay]
#[test]
fn replayed_tokens_bad_signatures_and_unpinned_issuers_are_refused() {
    let mut h = harness();
    let token = h.token(KNOT, nsid::NOTIFY_EVENT);
    let body = ok(serde_json::to_vec(&push(&"a".repeat(40), &"b".repeat(40))));
    let (status, _) = h.post_with(
        Some(format!("Bearer {token}")),
        nsid::NOTIFY_EVENT,
        body.clone(),
    );
    assert_eq!(status, 200);
    let (status, error) = h.post_with(
        Some(format!("Bearer {token}")),
        nsid::NOTIFY_EVENT,
        body.clone(),
    );
    assert_eq!(
        (status, field(&error, "/error").as_str()),
        (401, Some("AuthenticationRequired"))
    );
    assert!(some(field(&error, "/message").as_str()).starts_with("tangled_auth_replay"));

    let rogue = sign_token(
        &Signer::K256(rogue_key()),
        &claims(KNOT, nsid::NOTIFY_EVENT, NOW, "rogue"),
    );
    let (status, error) = h.post_with(
        Some(format!("Bearer {rogue}")),
        nsid::NOTIFY_EVENT,
        body.clone(),
    );
    assert_eq!(status, 401);
    assert!(some(field(&error, "/message").as_str()).starts_with("tangled_auth_signature"));

    let foreign = sign_token(
        &Signer::K256(rogue_key()),
        &claims("did:web:evil.example.com", nsid::NOTIFY_EVENT, NOW, "x"),
    );
    let (status, error) = h.post_with(
        Some(format!("Bearer {foreign}")),
        nsid::NOTIFY_EVENT,
        body.clone(),
    );
    assert_eq!(status, 403);
    assert!(some(field(&error, "/message").as_str()).starts_with("tangled_auth_issuer"));

    let wrong_method = h.token(KNOT, nsid::APPROVE_PIPELINE);
    let (status, _) = h.post_with(
        Some(format!("Bearer {wrong_method}")),
        nsid::NOTIFY_EVENT,
        body.clone(),
    );
    assert_eq!(status, 401);
    let (status, _) = h.post_with(None, nsid::NOTIFY_EVENT, body);
    assert_eq!(status, 401);
    assert_eq!(ok(h.service.store().pipeline_count()), 1);
}

// r[verify kiln.tangled.events.scenario.malformed] r[verify kiln.tangled.identity.scenario.unknown-repo]
#[test]
fn malformed_xrpc_and_unknown_repositories_create_nothing() {
    let mut h = harness();
    let (status, error) = h.post(KNOT, nsid::NOTIFY_EVENT, &json!({ "repo": REPO }));
    assert_eq!(
        (status, field(&error, "/error").as_str()),
        (400, Some("InvalidRequest"))
    );
    let token = h.token(KNOT, nsid::NOTIFY_EVENT);
    let (status, _) = h.post_with(
        Some(format!("Bearer {token}")),
        nsid::NOTIFY_EVENT,
        b"{".to_vec(),
    );
    assert_eq!(status, 400);
    let mut unknown = push(&"a".repeat(40), &"b".repeat(40));
    set(&mut unknown, "repo", json!(OTHER_REPO));
    if let Some(event) = unknown.get_mut("event") {
        set(event, "repo", json!(OTHER_REPO));
    }
    let (status, error) = h.post(KNOT, nsid::NOTIFY_EVENT, &unknown);
    assert_eq!(
        (status, field(&error, "/error").as_str()),
        (404, Some("RepoNotFound"))
    );
    let (status, _) = h.post(KNOT, nsid::NOTIFY_EVENT, &push(&"a".repeat(40), "zz"));
    assert_eq!(status, 422);
    let (status, error) = h.get(nsid::GET_PIPELINE, &[("repo", REPO)]);
    assert_eq!(
        (status, field(&error, "/error").as_str()),
        (400, Some("InvalidRequest"))
    );
    let (status, _) = h.get(
        nsid::GET_PIPELINE,
        &[("repo", REPO), ("repo", REPO), ("pipeline", "x")],
    );
    assert_eq!(status, 400);
    let (status, error) = h.get(
        nsid::GET_PIPELINE,
        &[("repo", REPO), ("pipeline", "kiln-missing")],
    );
    assert_eq!(
        (status, field(&error, "/error").as_str()),
        (404, Some("PipelineNotFound"))
    );
    let (status, _) = h.get("org.tangled.ci.cancelPipeline", &[]);
    assert_eq!(status, 501);
    let (status, _) = h.get(nsid::NOTIFY_EVENT, &[]);
    assert_eq!(status, 405);
    assert_eq!(ok(h.service.store().pipeline_count()), 0);
    assert_eq!(h.service.store().run_count().unwrap_or(usize::MAX), 0);
}

#[test]
fn acquisition_failures_are_retryable_and_create_nothing() {
    let mut h = harness();
    let root = ok(TempDir::new());
    let source = FixtureSource {
        failure: Some(crate::error::retryable(
            "fixture_knot_down",
            "Knot unreachable",
        )),
        ..FixtureSource::default()
    };
    h.service = TangledService::new(
        profile(),
        store(&root),
        source,
        h.workflow.clone(),
        h.status.clone(),
        StaticKeyResolver::new(issuer_keys()),
    );
    let (status, error) = h.post(
        KNOT,
        nsid::NOTIFY_EVENT,
        &push(&"a".repeat(40), &"b".repeat(40)),
    );
    assert_eq!(
        (status, field(&error, "/error").as_str()),
        (503, Some("ServiceUnavailable"))
    );
    assert_eq!(ok(h.service.store().pipeline_count()), 0);
}

// r[verify kiln.tangled.untrusted] r[verify kiln.tangled.untrusted.scenario.held]
#[test]
fn untrusted_pull_request_is_blocked_until_an_approver_approves() {
    let mut h = harness();
    let pull = json!({ "repo": REPO, "trigger": {
        "$type": nsid::PULL_REQUEST, "action": "opened", "pull": format!("at://{REPO}/org.tangled.track.ticket/3lzq"),
        "targetBranch": "main", "sourceBranch": "feature", "sourceRepo": OTHER_REPO, "sourceSha": "c".repeat(64)
    }});
    let (status, body) = h.post(TRUSTED, nsid::TRIGGER_PIPELINE, &pull);
    assert_eq!(status, 200, "{body}");
    let id = first_pipeline(&body);
    assert!(some(field(&body, "/ci/message").as_str()).contains("blocked"));
    h.drain();
    assert!(
        h.workflow.dispatched().is_empty(),
        "nothing runs before approval"
    );
    let view = h.pipeline(&id);
    assert_eq!(statuses(&view), vec!["blocked"]);
    assert_eq!(field(&view, "/sourceRepo"), OTHER_REPO);
    assert_eq!(statuses(some(h.status.views().last())), vec!["blocked"]);

    let approve = json!({ "repo": REPO, "pipeline": id });
    let (status, error) = h.post(TRUSTED, nsid::APPROVE_PIPELINE, &approve);
    assert_eq!(status, 403, "{error}");
    assert!(some(field(&error, "/message").as_str()).starts_with("tangled_approval_unauthorized"));
    let (status, _) = h.post(APPROVER, nsid::RETRY_PIPELINE, &approve);
    assert_eq!(status, 400, "a blocked pipeline is not retryable");
    let (status, _) = h.post(APPROVER, nsid::APPROVE_PIPELINE, &approve);
    assert_eq!(status, 200);
    let (status, error) = h.post(APPROVER, nsid::APPROVE_PIPELINE, &approve);
    assert_eq!(
        (status, field(&error, "/error").as_str()),
        (400, Some("PipelineNotBlocked"))
    );
    h.drain();
    let plans = h.workflow.dispatched();
    assert_eq!(plans.len(), 1);
    let plan = some(plans.first());
    assert_eq!(plan.workflow_policy_id.as_str(), "tangled-untrusted-v1");
    assert_eq!(
        some(plan.source.as_ref()).revision.native_id.as_str(),
        "c".repeat(64)
    );
    assert_eq!(statuses(&h.pipeline(&id)), vec!["success"]);

    // A retry of an approved pipeline stays untrusted.
    let (status, retried) = h.post(
        APPROVER,
        nsid::RETRY_PIPELINE,
        &json!({ "repo": REPO, "pipeline": id, "workflows": ["test.yml"] }),
    );
    assert_eq!(status, 200, "{retried}");
    let retry_id = some(field(&retried, "/pipeline").as_str()).to_string();
    assert_ne!(retry_id, id);
    h.drain();
    let plans = h.workflow.dispatched();
    assert_eq!(plans.len(), 2);
    assert_eq!(
        some(plans.last()).workflow_policy_id.as_str(),
        "tangled-untrusted-v1"
    );
    assert_eq!(
        field(&h.pipeline(&retry_id), "/commit"),
        "c".repeat(64).as_str()
    );
}

// r[verify kiln.tangled.xrpc.scenario.retry] r[verify kiln.tangled.xrpc]
#[test]
fn retry_list_and_get_follow_the_lexicons() {
    let mut h = harness();
    let (_, body) = h.post(
        KNOT,
        nsid::NOTIFY_EVENT,
        &push(&"0".repeat(64), &"b".repeat(64)),
    );
    let id = first_pipeline(&body);
    h.drain();
    // New branch: changed files unknown, so the docs workflow runs too.
    assert_eq!(h.workflow.dispatched().len(), 2);
    let (status, retried) = h.post(
        TRUSTED,
        nsid::RETRY_PIPELINE,
        &json!({ "repo": REPO, "pipeline": id, "workflows": ["docs.yml"] }),
    );
    assert_eq!(status, 200);
    let retry_id = some(field(&retried, "/pipeline").as_str()).to_string();
    h.drain();
    let view = h.pipeline(&retry_id);
    assert_eq!(some(field(&view, "/workflows").as_array()).len(), 1);
    assert_eq!(field(&view, "/workflows/0/id"), "docs.yml");
    assert_eq!(h.workflow.dispatched().len(), 3);
    let (status, error) = h.post(
        TRUSTED,
        nsid::RETRY_PIPELINE,
        &json!({ "repo": REPO, "pipeline": id, "workflows": ["nope.yml"] }),
    );
    assert_eq!(
        (status, field(&error, "/error").as_str()),
        (400, Some("InvalidRequest"))
    );
    let (status, error) = h.post(
        TRUSTED,
        nsid::RETRY_PIPELINE,
        &json!({ "repo": REPO, "pipeline": "kiln-missing" }),
    );
    assert_eq!(
        (status, field(&error, "/error").as_str()),
        (404, Some("PipelineNotFound"))
    );
    let (status, _) = h.post(
        KNOT,
        nsid::RETRY_PIPELINE,
        &json!({ "repo": REPO, "pipeline": id }),
    );
    assert_eq!(status, 403);

    let (status, listed) = h.get(nsid::LIST_WORKFLOWS, &[("repo", REPO)]);
    assert_eq!(status, 200);
    let ids: Vec<&str> = some(field(&listed, "/workflows").as_array())
        .iter()
        .filter_map(|w| field(w, "/id").as_str())
        .collect();
    assert_eq!(
        ids,
        vec!["test.yml", "docs.yml"],
        "unreviewable definitions are not listed"
    );
    assert_eq!(
        field(&listed, "/workflows/0/source/$type"),
        nsid::FILE_SOURCE
    );
    assert_eq!(
        field(&listed, "/workflows/0/source/commit"),
        "f".repeat(64).as_str()
    );
    let (status, error) = h.get(nsid::LIST_WORKFLOWS, &[("repo", REPO), ("ref", "nope")]);
    assert_eq!(
        (status, field(&error, "/error").as_str()),
        (404, Some("RefNotFound"))
    );
    let (status, error) = h.get(nsid::LIST_WORKFLOWS, &[("repo", OTHER_REPO)]);
    assert_eq!(
        (status, field(&error, "/error").as_str()),
        (404, Some("RepoNotFound"))
    );
    let (status, got) = h.get(
        nsid::GET_WORKFLOW,
        &[("repo", REPO), ("workflow", "test.yml")],
    );
    assert_eq!(status, 200);
    assert_eq!(
        field(&got, "/workflow/triggers"),
        &json!([nsid::MANUAL, nsid::PULL_REQUEST, nsid::PUSH])
    );
    let (status, error) = h.get(
        nsid::GET_WORKFLOW,
        &[("repo", REPO), ("workflow", "cache.yml")],
    );
    assert_eq!(
        (status, field(&error, "/error").as_str()),
        (404, Some("WorkflowNotFound"))
    );
}

// r[verify kiln.tangled.status]
#[test]
fn uncertain_dispatch_and_delivery_never_report_success() {
    let mut workflow = FixtureWorkflow::new();
    workflow.lose_dispatch = true;
    let status = FixtureStatus {
        lose_response: true,
        ..FixtureStatus::default()
    };
    let mut h = harness_with(workflow, status);
    let (_, body) = h.post(
        KNOT,
        nsid::NOTIFY_EVENT,
        &push(&"a".repeat(40), &"b".repeat(40)),
    );
    let id = first_pipeline(&body);
    assert!(ok(h.service.tick(NOW)));
    let view = h.pipeline(&id);
    assert_eq!(statuses(&view), vec!["pending"]);
    assert!(some(field(&view, "/workflows/0/error").as_str()).contains("unknown"));
    // Recovery observes the accepted dispatch, then completion.
    h.drain();
    assert!(
        h.workflow
            .remote
            .lock()
            .map(|remote| remote.recoveries)
            .unwrap_or(0)
            >= 1
    );
    assert_eq!(statuses(&h.pipeline(&id)), vec!["success"]);
    // Every delivery lost its response: re-sent, never recorded as published.
    let deliveries = h.status.deliveries.lock().map(|d| d.len()).unwrap_or(0);
    assert!(deliveries >= 2);
}

#[test]
fn failed_runs_map_to_failed_and_unknown_outcomes_stay_pending() {
    let profile = profile();
    let admission = ok(admit_notify(
        &profile,
        &verified(KNOT, "j"),
        &ok(serde_json::to_vec(&push(&"a".repeat(40), &"b".repeat(40)))),
    ));
    let Admission::Event(event) = admission else {
        unreachable!()
    };
    let PlanOutcome::Pipeline(mut record) = ok(plan_pipeline(
        &profile,
        &event,
        &acquired(&event.commit),
        NOW,
    )) else {
        unreachable!()
    };
    let run = some(record.workflows.first_mut());
    let plan = run.plan.clone();
    let mut fresh = run.clone();
    ok(run.accept(
        Ok(observation(
            &plan,
            true,
            kiln_core::domain::Outcome::DomainFailure,
            1,
        )),
        NOW,
    ));
    assert_eq!(workflow_status(None, run).0, "failed");
    // Core history refuses to reopen a terminal run.
    assert!(
        run.accept(Err(crate::error::unknown("lost", "lost")), NOW)
            .is_err()
    );
    ok(restore(&mut record));
    // A lost response leaves a run unknown: pending with an error, never success.
    ok(fresh.accept(Err(crate::error::unknown("lost", "lost")), NOW));
    let (status, error) = workflow_status(None, &fresh);
    assert_eq!(status, "pending");
    assert!(some(error).contains("unknown"));
    // A successful observation that arrives later reconciles it.
    ok(fresh.accept(
        Ok(observation(
            &plan,
            true,
            kiln_core::domain::Outcome::Success,
            1,
        )),
        NOW,
    ));
    assert_eq!(workflow_status(None, &fresh).0, "success");

    // Manual runs name dispatchable workflows only.
    let manual = |names: serde_json::Value| {
        ok(serde_json::to_vec(
            &json!({ "repo": REPO, "trigger": { "$type": nsid::MANUAL, "repo": REPO, "commit": "e".repeat(40) }, "workflows": names }),
        ))
    };
    let Admission::Event(event) = ok(admit_trigger(
        &profile,
        &verified(TRUSTED, "m"),
        &manual(json!(["docs.yml"])),
    )) else {
        unreachable!()
    };
    assert_eq!(
        err(plan_pipeline(
            &profile,
            &event,
            &acquired(&event.commit),
            NOW
        ))
        .code,
        "tangled_manual_workflow"
    );
    let Admission::Event(event) = ok(admit_trigger(
        &profile,
        &verified(TRUSTED, "m2"),
        &manual(json!([])),
    )) else {
        unreachable!()
    };
    let PlanOutcome::Pipeline(record) = ok(plan_pipeline(
        &profile,
        &event,
        &acquired(&event.commit),
        NOW,
    )) else {
        unreachable!()
    };
    assert_eq!(record.workflows.len(), 1);
    // A source for a different commit is refused.
    assert!(plan_pipeline(&profile, &event, &acquired(&object('9', 40)), NOW).is_err());
}
