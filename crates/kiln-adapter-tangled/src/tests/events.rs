use serde_json::json;

use super::*;
use crate::error::RejectionClass;
use crate::events::*;
use crate::nsid;
use crate::when::PullRequestType;

fn push_body(repo: &str, reference: &str, old: &str, new: &str) -> Vec<u8> {
    ok(serde_json::to_vec(&json!({
        "repo": repo,
        "event": { "$type": nsid::PUSH, "repo": repo, "ref": reference, "oldSha": old, "newSha": new, "actor": TRUSTED }
    })))
}

fn event(admission: Admission) -> AdmittedEvent {
    match admission {
        Admission::Event(event) => *event,
        Admission::Ignored(reason) => unreachable!("ignored: {reason}"),
    }
}

fn pull_body(source_repo: Option<&str>, source_branch: Option<&str>) -> Vec<u8> {
    let mut trigger = json!({
        "$type": nsid::PULL_REQUEST,
        "action": "opened",
        "pull": format!("at://{REPO}/org.tangled.track.ticket/3lzq4w5qk2c2a"),
        "targetBranch": "main",
        "sourceSha": "c".repeat(64),
    });
    if let Some(repo) = source_repo {
        set(&mut trigger, "sourceRepo", json!(repo));
    }
    if let Some(branch) = source_branch {
        set(&mut trigger, "sourceBranch", json!(branch));
    }
    ok(serde_json::to_vec(
        &json!({ "repo": REPO, "trigger": trigger }),
    ))
}

// r[verify kiln.tangled.events] r[verify kiln.tangled.replay]
#[test]
fn knot_push_binds_exact_revision_and_stable_replay_key() {
    let profile = profile();
    let old = "a".repeat(64);
    let new = "b".repeat(64);
    let first = event(ok(admit_notify(
        &profile,
        &verified(KNOT, "one"),
        &push_body(REPO, "refs/heads/main", &old, &new),
    )));
    assert_eq!(first.commit.as_str(), new);
    assert_eq!(
        first.base.as_ref().map(|base| base.as_str().to_string()),
        Some(old.clone())
    );
    assert_eq!(first.trust, Trust::Trusted);
    assert_eq!(field(&first.native, "/$type"), nsid::PUSH);
    // A redelivery under a fresh token is the same push.
    let again = event(ok(admit_notify(
        &profile,
        &verified(KNOT, "two"),
        &push_body(REPO, "refs/heads/main", &old, &new),
    )));
    assert_eq!(first.replay_key, again.replay_key);
    let other = event(ok(admit_notify(
        &profile,
        &verified(KNOT, "three"),
        &push_body(REPO, "refs/heads/dev", &old, &new),
    )));
    assert_ne!(first.replay_key, other.replay_key);
    // A new branch has no base.
    let created = event(ok(admit_notify(
        &profile,
        &verified(KNOT, "four"),
        &push_body(REPO, "refs/tags/v1", &"0".repeat(40), &"d".repeat(40)),
    )));
    assert!(created.base.is_none());
}

// r[verify kiln.tangled.identity.scenario.unknown-repo] r[verify kiln.tangled.events.scenario.malformed]
#[test]
fn unknown_repositories_foreign_issuers_and_malformed_pushes_are_refused() {
    let profile = profile();
    let (old, new) = ("a".repeat(40), "b".repeat(40));
    let unknown = err(admit_notify(
        &profile,
        &verified(KNOT, "j"),
        &push_body(OTHER_REPO, "refs/heads/main", &old, &new),
    ));
    assert_eq!(unknown.class, RejectionClass::NotFound("RepoNotFound"));
    let issuer = err(admit_notify(
        &profile,
        &verified("did:web:evil.example.com", "j"),
        &push_body(REPO, "refs/heads/main", &old, &new),
    ));
    assert_eq!(issuer.class, RejectionClass::Forbidden);
    let actor_push = err(admit_notify(
        &profile,
        &verified(TRUSTED, "j"),
        &push_body(REPO, "refs/heads/main", &old, &new),
    ));
    assert_eq!(actor_push.code, "tangled_push_issuer");
    let hex = err(admit_notify(
        &profile,
        &verified(KNOT, "j"),
        &push_body(REPO, "refs/heads/main", &old, "not-hex"),
    ));
    assert_eq!(hex.class, RejectionClass::Unprocessable);
    let mixed = err(admit_notify(
        &profile,
        &verified(KNOT, "j"),
        &push_body(REPO, "refs/heads/main", &old, &"b".repeat(64)),
    ));
    assert_eq!(mixed.code, "tangled_event_object_format");
    let mismatch = ok(serde_json::to_vec(&json!({
        "repo": REPO,
        "event": { "$type": nsid::PUSH, "repo": OTHER_REPO, "ref": "refs/heads/main", "oldSha": old, "newSha": new, "actor": TRUSTED }
    })));
    assert_eq!(
        err(admit_notify(&profile, &verified(KNOT, "j"), &mismatch)).code,
        "tangled_event_repo_mismatch"
    );
    let no_actor = ok(serde_json::to_vec(&json!({
        "repo": REPO,
        "event": { "$type": nsid::PUSH, "repo": REPO, "ref": "refs/heads/main", "oldSha": old, "newSha": new }
    })));
    assert_eq!(
        err(admit_notify(&profile, &verified(KNOT, "j"), &no_actor)).code,
        "tangled_event_actor"
    );
    for body in [
        b"not json".to_vec(),
        br#"{"repo":"did:plc:abcdefghijklmnopqrstuvwx"}"#.to_vec(),
        br#"{"repo":"did:plc:abcdefghijklmnopqrstuvwx","event":{"$type":"org.tangled.event.unknown"}}"#.to_vec(),
        br#"{"repo":"did:plc:abcdefghijklmnopqrstuvwx","repo":"x","event":{"$type":"org.tangled.event.push"}}"#.to_vec(),
        push_body(REPO, "refs/heads/../x", &old, &new),
    ] {
        assert_eq!(err(admit_notify(&profile, &verified(KNOT, "j"), &body)).class, RejectionClass::InvalidRequest);
    }
}

#[test]
fn deletions_skip_ci_and_non_branch_refs_start_nothing() {
    let profile = profile();
    let ignored = |body: Vec<u8>| {
        matches!(
            ok(admit_notify(&profile, &verified(KNOT, "j"), &body)),
            Admission::Ignored(_)
        )
    };
    assert!(ignored(push_body(
        REPO,
        "refs/heads/main",
        &"a".repeat(40),
        &"0".repeat(40)
    )));
    assert!(ignored(push_body(
        REPO,
        "refs/notes/commits",
        &"a".repeat(40),
        &"b".repeat(40)
    )));
    let skip = ok(serde_json::to_vec(&json!({
        "repo": REPO,
        "event": { "$type": nsid::PUSH, "repo": REPO, "ref": "refs/heads/main", "oldSha": "a".repeat(40), "newSha": "b".repeat(40), "actor": TRUSTED, "pushOptions": ["ci-skip"] }
    })));
    assert!(ignored(skip));
}

#[test]
fn manual_runs_need_push_access_and_no_inputs() {
    let profile = profile();
    let manual = |inputs: serde_json::Value| {
        ok(serde_json::to_vec(&json!({
            "repo": REPO,
            "event": { "$type": nsid::MANUAL, "repo": REPO, "commit": "e".repeat(40), "ref": "main", "workflows": ["test.yml"], "inputs": inputs }
        })))
    };
    let admitted = event(ok(admit_notify(
        &profile,
        &verified(TRUSTED, "m1"),
        &manual(json!([])),
    )));
    assert_eq!(admitted.workflows, vec!["test.yml".to_string()]);
    assert_eq!(admitted.actor, TRUSTED);
    // Manual replay keys are bound to the token, so each command is new.
    let second = event(ok(admit_notify(
        &profile,
        &verified(TRUSTED, "m2"),
        &manual(json!([])),
    )));
    assert_ne!(admitted.replay_key, second.replay_key);
    assert_eq!(
        err(admit_notify(
            &profile,
            &verified(KNOT, "m"),
            &manual(json!([]))
        ))
        .code,
        "tangled_manual_actor"
    );
    assert_eq!(
        err(admit_notify(
            &profile,
            &verified(OUTSIDER, "m"),
            &manual(json!([]))
        ))
        .code,
        "tangled_manual_actor"
    );
    let inputs = manual(json!([{ "key": "target", "value": "prod" }]));
    assert_eq!(
        err(admit_notify(&profile, &verified(TRUSTED, "m"), &inputs)).code,
        "tangled_manual_inputs"
    );
}

// r[verify kiln.tangled.untrusted]
#[test]
fn pull_requests_follow_untrusted_classification() {
    let profile = profile();
    let fork = event(ok(admit_trigger(
        &profile,
        &verified(TRUSTED, "p"),
        &pull_body(Some(OTHER_REPO), Some("feature")),
    )));
    assert_eq!(
        fork.trust,
        Trust::Untrusted {
            reason: UntrustedReason::Fork
        }
    );
    let patch = event(ok(admit_trigger(
        &profile,
        &verified(TRUSTED, "p"),
        &pull_body(None, None),
    )));
    assert_eq!(
        patch.trust,
        Trust::Untrusted {
            reason: UntrustedReason::Fork
        }
    );
    let relayed = event(ok(admit_trigger(
        &profile,
        &verified(KNOT, "p"),
        &pull_body(None, Some("feature")),
    )));
    assert_eq!(
        relayed.trust,
        Trust::Untrusted {
            reason: UntrustedReason::AuthorWithoutPushAccess
        }
    );
    let branch = event(ok(admit_trigger(
        &profile,
        &verified(TRUSTED, "p"),
        &pull_body(Some(REPO), Some("feature")),
    )));
    assert_eq!(branch.trust, Trust::Trusted);
    assert_eq!(
        branch.trigger,
        Trigger::PullRequest {
            action: PullRequestType::Opened,
            target_branch: "main".into(),
            source_branch: Some("feature".into()),
            pull: format!("at://{REPO}/org.tangled.track.ticket/3lzq4w5qk2c2a"),
        }
    );
    assert_eq!(
        err(admit_trigger(
            &profile,
            &verified(OUTSIDER, "p"),
            &pull_body(None, Some("f"))
        ))
        .code,
        "tangled_pull_caller"
    );
    let foreign_pull = ok(serde_json::to_vec(&json!({ "repo": REPO, "trigger": {
        "$type": nsid::PULL_REQUEST, "action": "opened", "pull": format!("at://{OTHER_REPO}/org.tangled.track.ticket/3l"),
        "targetBranch": "main", "sourceSha": "c".repeat(40)
    }})));
    assert_eq!(
        err(admit_trigger(
            &profile,
            &verified(TRUSTED, "p"),
            &foreign_pull
        ))
        .code,
        "tangled_pull_uri"
    );
    let bad_action = ok(serde_json::to_vec(&json!({ "repo": REPO, "trigger": {
        "$type": nsid::PULL_REQUEST, "action": "edited", "pull": format!("at://{REPO}/org.tangled.track.ticket/3l"),
        "targetBranch": "main", "sourceSha": "c".repeat(40)
    }})));
    assert_eq!(
        err(admit_trigger(
            &profile,
            &verified(TRUSTED, "p"),
            &bad_action
        ))
        .code,
        "tangled_pull_action"
    );
}
