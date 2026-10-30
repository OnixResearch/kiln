//! Pure admission of Tangled trigger events after service-auth verification.
//!
//! `notifyEvent` carries `org.tangled.event.push` (from the repository's knot)
//! or `org.tangled.ci.trigger.manual` (from an actor with push access).
//! `triggerPipeline` carries `org.tangled.event.pullRequest` or a manual
//! trigger. Admission binds the pinned repository, the exact object id and a
//! BLAKE3 replay key; it performs no I/O.

use kiln_codec::encode_and_identify;
use kiln_core::types::ContentId;
use kiln_core::untrusted::{PullRequestTrustFacts, TrustDecision, UntrustedPullRequests, classify};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::auth::VerifiedClaims;
use crate::error::{Rejection, RejectionClass, TangledResult};
use crate::identity::{ObjectId, branch_name, tag_name, valid_at_uri, valid_did, valid_reference};
use crate::nsid;
use crate::profile::{AdmittedProfile, RepositoryPin};
use crate::when::{PullRequestType, pull_request_type};
use crate::workflows::{MAXIMUM_NAME_BYTES, workflow_id};
use crate::xrpc::set_field;

/// Lexicon bound on push options.
pub const MAXIMUM_PUSH_OPTIONS: usize = 50;
pub const MAXIMUM_PUSH_OPTION_BYTES: usize = 1024;
/// Upper bound on named workflows in one trigger.
pub const MAXIMUM_NAMED_WORKFLOWS: usize = 64;
const MAXIMUM_REPLAY_BYTES: usize = 65_536;

pub use kiln_core::untrusted::UntrustedReason;

/// GitHub-style author association that #12 counts as push access; Tangled
/// grants it to the profile's pinned trusted actors.
const PUSH_ACCESS_ASSOCIATION: &str = "COLLABORATOR";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "trust", rename_all = "snake_case")]
pub enum Trust {
    Trusted,
    Untrusted { reason: UntrustedReason },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Trigger {
    Push {
        reference: String,
    },
    Manual {
        reference: Option<String>,
    },
    PullRequest {
        action: PullRequestType,
        target_branch: String,
        source_branch: Option<String>,
        pull: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedEvent {
    pub repo_did: String,
    pub trigger: Trigger,
    /// Canonical lexicon object with `$type`, kept as a provider fact.
    pub native: Value,
    pub commit: ObjectId,
    /// Previous head for changed-file facts; `None` for new refs and manual runs.
    pub base: Option<ObjectId>,
    pub source_repo: Option<String>,
    pub actor: String,
    pub trust: Trust,
    /// Workflow ids the caller named; empty means every matching workflow.
    pub workflows: Vec<String>,
    pub replay_key: ContentId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Admission {
    /// Well formed and authorized, but Tangled starts nothing for it.
    Ignored(&'static str),
    Event(Box<AdmittedEvent>),
}

#[derive(Deserialize)]
struct Tagged {
    #[serde(rename = "$type")]
    kind: String,
}

#[derive(Deserialize)]
struct NotifyEnvelope {
    event: Tagged,
}

#[derive(Deserialize)]
struct NotifyPush {
    repo: String,
    event: PushWire,
}

#[derive(Deserialize)]
struct NotifyManual {
    repo: String,
    event: ManualWire,
}

#[derive(Deserialize)]
struct PushWire {
    repo: String,
    #[serde(rename = "ref")]
    reference: String,
    #[serde(rename = "oldSha")]
    old_sha: String,
    #[serde(rename = "newSha")]
    new_sha: String,
    #[serde(default)]
    actor: Option<String>,
    #[serde(rename = "pushOptions", default)]
    push_options: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct Pair {
    #[allow(dead_code)]
    key: String,
    #[allow(dead_code)]
    value: String,
}

#[derive(Deserialize)]
struct ManualWire {
    repo: String,
    commit: String,
    #[serde(rename = "ref", default)]
    reference: Option<String>,
    #[serde(default)]
    workflows: Option<Vec<String>>,
    #[serde(default)]
    inputs: Option<Vec<Pair>>,
}

#[derive(Deserialize)]
struct TriggerEnvelope {
    trigger: Tagged,
}

#[derive(Deserialize)]
struct TriggerManual {
    repo: String,
    trigger: ManualWire,
    #[serde(default)]
    workflows: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct TriggerPull {
    repo: String,
    trigger: PullWire,
    #[serde(default)]
    workflows: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct PullWire {
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    pull: Option<String>,
    #[serde(rename = "sourceRepo", default)]
    source_repo: Option<String>,
    #[serde(rename = "sourceBranch", default)]
    source_branch: Option<String>,
    #[serde(rename = "targetBranch", default)]
    target_branch: Option<String>,
    #[serde(rename = "sourceSha", default)]
    source_sha: Option<String>,
}

fn malformed(code: &'static str) -> Rejection {
    Rejection::invalid(code, "Tangled event is malformed or inconsistent")
}

fn decode<'de, T: Deserialize<'de>>(body: &'de [u8], code: &'static str) -> TangledResult<T> {
    serde_json::from_slice(body).map_err(|_| malformed(code))
}

fn pinned<'a>(profile: &'a AdmittedProfile, repo: &str) -> TangledResult<&'a RepositoryPin> {
    if !valid_did(repo) {
        return Err(malformed("tangled_event_repo"));
    }
    profile.repository(repo).ok_or(Rejection::new(
        RejectionClass::NotFound("RepoNotFound"),
        "tangled_repo_unknown",
        "Repository DID is not served by this Kiln spindle",
    ))
}

fn replay_key<T: Serialize>(material: &T) -> TangledResult<ContentId> {
    encode_and_identify(material, MAXIMUM_REPLAY_BYTES)
        .map(|(_, identity)| identity)
        .map_err(|_| malformed("tangled_event_identity"))
}

fn object_id(value: &str) -> TangledResult<ObjectId> {
    // Spindle answers 422 here: a retry would send the same object ids.
    ObjectId::parse(value).ok_or(Rejection::new(
        RejectionClass::Unprocessable,
        "tangled_event_object_id",
        "Object id is not 40 or 64 lowercase hex characters",
    ))
}

fn named_workflows(values: Option<Vec<String>>) -> TangledResult<Vec<String>> {
    let values = values.unwrap_or_default();
    if values.len() > MAXIMUM_NAMED_WORKFLOWS
        || values.iter().any(|name| {
            name.len() > MAXIMUM_NAME_BYTES
                || workflow_id(&format!(".tangled/workflows/{name}")).is_none()
        })
    {
        return Err(malformed("tangled_event_workflows"));
    }
    let mut sorted = values;
    sorted.sort();
    sorted.dedup();
    Ok(sorted)
}

fn is_push_actor(pin: &RepositoryPin, did: &str) -> bool {
    pin.trusted_actors
        .iter()
        .chain(&pin.approvers)
        .any(|actor| actor == did)
}

// r[impl kiln.tangled.events] r[impl kiln.tangled.identity] r[impl kiln.tangled.replay]
/// Admit an `org.tangled.event.notifyEvent` request body.
pub fn admit_notify(
    profile: &AdmittedProfile,
    claims: &VerifiedClaims,
    body: &[u8],
) -> TangledResult<Admission> {
    let envelope: NotifyEnvelope = decode(body, "tangled_event_body")?;
    match envelope.event.kind.as_str() {
        nsid::PUSH => {
            let request: NotifyPush = decode(body, "tangled_event_push")?;
            admit_push(profile, claims, request)
        }
        nsid::MANUAL => {
            let request: NotifyManual = decode(body, "tangled_event_manual")?;
            admit_manual(profile, claims, &request.repo, request.event, None)
        }
        _ => Err(malformed("tangled_event_type")),
    }
}

fn admit_push(
    profile: &AdmittedProfile,
    claims: &VerifiedClaims,
    request: NotifyPush,
) -> TangledResult<Admission> {
    let push = request.event;
    if push.repo != request.repo {
        return Err(malformed("tangled_event_repo_mismatch"));
    }
    let pin = pinned(profile, &request.repo)?;
    if claims.issuer != pin.knot_did {
        return Err(Rejection::forbidden(
            "tangled_push_issuer",
            "Only the repository's pinned knot may notify pushes",
        ));
    }
    let actor = push
        .actor
        .filter(|actor| valid_did(actor))
        .ok_or(malformed("tangled_event_actor"))?;
    let options = push.push_options.unwrap_or_default();
    if !valid_reference(&push.reference)
        || options.len() > MAXIMUM_PUSH_OPTIONS
        || options
            .iter()
            .any(|option| option.len() > MAXIMUM_PUSH_OPTION_BYTES)
    {
        return Err(malformed("tangled_event_push"));
    }
    let old = object_id(&push.old_sha)?;
    let new = object_id(&push.new_sha)?;
    if old.format() != new.format() {
        return Err(malformed("tangled_event_object_format"));
    }
    if branch_name(&push.reference).is_none() && tag_name(&push.reference).is_none() {
        return Ok(Admission::Ignored(
            "references outside heads and tags start no pipeline",
        ));
    }
    if new.is_zero() {
        return Ok(Admission::Ignored("deleted references start no pipeline"));
    }
    if options
        .iter()
        .any(|option| matches!(option.as_str(), "skip-ci" | "ci-skip"))
    {
        return Ok(Admission::Ignored("push requested skip-ci"));
    }
    let replay_key = replay_key(&(
        "kiln.tangled.push.v1",
        &request.repo,
        &push.reference,
        old.as_str(),
        new.as_str(),
    ))?;
    let mut native = json!({
        "$type": nsid::PUSH,
        "repo": request.repo,
        "ref": push.reference,
        "oldSha": old.as_str(),
        "newSha": new.as_str(),
        "actor": actor,
    });
    if !options.is_empty() {
        set_field(&mut native, "pushOptions", json!(options));
    }
    Ok(Admission::Event(Box::new(AdmittedEvent {
        repo_did: request.repo,
        trigger: Trigger::Push {
            reference: push.reference,
        },
        native,
        base: (!old.is_zero()).then_some(old),
        commit: new,
        source_repo: None,
        actor,
        trust: Trust::Trusted,
        workflows: Vec::new(),
        replay_key,
    })))
}

fn admit_manual(
    profile: &AdmittedProfile,
    claims: &VerifiedClaims,
    repo: &str,
    manual: ManualWire,
    outer_workflows: Option<Vec<String>>,
) -> TangledResult<Admission> {
    if manual.repo != repo {
        // A manual run of a fork would need the fork's clone URL, which is not pinned.
        return Err(malformed("tangled_event_repo_mismatch"));
    }
    let pin = pinned(profile, repo)?;
    if !is_push_actor(pin, &claims.issuer) {
        return Err(Rejection::forbidden(
            "tangled_manual_actor",
            "Manual runs need an actor with push access to the repository",
        ));
    }
    if manual
        .inputs
        .as_ref()
        .is_some_and(|inputs| !inputs.is_empty())
    {
        return Err(Rejection::invalid(
            "tangled_manual_inputs",
            "Manual inputs have no reviewed Kiln mapping",
        ));
    }
    let commit = ObjectId::parse(&manual.commit).ok_or(malformed("tangled_event_commit"))?;
    if commit.is_zero()
        || manual
            .reference
            .as_ref()
            .is_some_and(|reference| !valid_reference(reference))
    {
        return Err(malformed("tangled_event_manual"));
    }
    let mut names = manual.workflows.unwrap_or_default();
    names.extend(outer_workflows.unwrap_or_default());
    let workflows = named_workflows(Some(names))?;
    let replay_key = replay_key(&(
        "kiln.tangled.manual.v1",
        repo,
        &claims.issuer,
        &claims.jti,
        commit.as_str(),
        &manual.reference,
        &workflows,
    ))?;
    let mut native = json!({ "$type": nsid::MANUAL, "repo": repo, "commit": commit.as_str() });
    if let Some(reference) = &manual.reference {
        set_field(&mut native, "ref", json!(reference));
    }
    if !workflows.is_empty() {
        set_field(&mut native, "workflows", json!(workflows));
    }
    Ok(Admission::Event(Box::new(AdmittedEvent {
        repo_did: repo.to_string(),
        trigger: Trigger::Manual {
            reference: manual.reference,
        },
        native,
        commit,
        base: None,
        source_repo: None,
        actor: claims.issuer.clone(),
        trust: Trust::Trusted,
        workflows,
        replay_key,
    })))
}

// r[impl kiln.tangled.events] r[impl kiln.tangled.untrusted]
/// Admit an `org.tangled.ci.triggerPipeline` request body.
pub fn admit_trigger(
    profile: &AdmittedProfile,
    claims: &VerifiedClaims,
    body: &[u8],
) -> TangledResult<Admission> {
    let envelope: TriggerEnvelope = decode(body, "tangled_trigger_body")?;
    match envelope.trigger.kind.as_str() {
        nsid::MANUAL => {
            let request: TriggerManual = decode(body, "tangled_trigger_manual")?;
            admit_manual(
                profile,
                claims,
                &request.repo,
                request.trigger,
                request.workflows,
            )
        }
        nsid::PULL_REQUEST => {
            let request: TriggerPull = decode(body, "tangled_trigger_pull")?;
            admit_pull(profile, claims, request)
        }
        _ => Err(malformed("tangled_trigger_type")),
    }
}

fn admit_pull(
    profile: &AdmittedProfile,
    claims: &VerifiedClaims,
    request: TriggerPull,
) -> TangledResult<Admission> {
    let pin = pinned(profile, &request.repo)?;
    let caller_trusted = pin.trusted_actors.contains(&claims.issuer);
    if !caller_trusted && !pin.approvers.contains(&claims.issuer) && claims.issuer != pin.knot_did {
        return Err(Rejection::forbidden(
            "tangled_pull_caller",
            "Pull request triggers need the knot, a trusted actor or an approver",
        ));
    }
    let wire = request.trigger;
    let action = wire
        .action
        .as_deref()
        .and_then(pull_request_type)
        .ok_or(malformed("tangled_pull_action"))?;
    let pull = wire
        .pull
        .filter(|uri| {
            valid_at_uri(uri)
                && uri
                    .strip_prefix("at://")
                    .is_some_and(|rest| rest.starts_with(&format!("{}/", request.repo)))
        })
        .ok_or(malformed("tangled_pull_uri"))?;
    let target_branch = wire
        .target_branch
        .filter(|branch| valid_reference(&format!("refs/heads/{branch}")))
        .ok_or(malformed("tangled_pull_target"))?;
    if wire
        .source_branch
        .as_ref()
        .is_some_and(|branch| !valid_reference(&format!("refs/heads/{branch}")))
        || wire
            .source_repo
            .as_ref()
            .is_some_and(|repo| !valid_did(repo))
    {
        return Err(malformed("tangled_pull_source"));
    }
    let commit = wire
        .source_sha
        .as_deref()
        .and_then(ObjectId::parse)
        .filter(|commit| !commit.is_zero())
        .ok_or(malformed("tangled_pull_sha"))?;
    let same_repository = wire.source_branch.is_some()
        && wire
            .source_repo
            .as_ref()
            .is_none_or(|source| *source == request.repo);
    // #12 classification: same repository, and an actor with push access vouching.
    let facts = PullRequestTrustFacts {
        same_repository,
        author_association: caller_trusted.then_some(PUSH_ACCESS_ASSOCIATION),
    };
    let trust = match classify(UntrustedPullRequests::Approve, facts) {
        TrustDecision::Trusted => Trust::Trusted,
        TrustDecision::Untrusted(reason) => Trust::Untrusted { reason },
        TrustDecision::Refused => {
            return Err(Rejection::forbidden(
                "tangled_pull_refused",
                "Pull request was refused by the untrusted-run policy",
            ));
        }
    };
    let workflows = named_workflows(request.workflows)?;
    let replay_key = replay_key(&(
        "kiln.tangled.pull.v1",
        &request.repo,
        &pull,
        commit.as_str(),
        action,
        &target_branch,
        &wire.source_branch,
        &wire.source_repo,
        &workflows,
    ))?;
    let mut native = json!({
        "$type": nsid::PULL_REQUEST,
        "action": wire.action,
        "pull": pull,
        "targetBranch": target_branch,
        "sourceSha": commit.as_str(),
    });
    if let Some(branch) = &wire.source_branch {
        set_field(&mut native, "sourceBranch", json!(branch));
    }
    if let Some(source) = &wire.source_repo {
        set_field(&mut native, "sourceRepo", json!(source));
    }
    Ok(Admission::Event(Box::new(AdmittedEvent {
        repo_did: request.repo,
        trigger: Trigger::PullRequest {
            action,
            target_branch,
            source_branch: wire.source_branch,
            pull,
        },
        native,
        commit,
        base: None,
        source_repo: wire.source_repo,
        actor: claims.issuer.clone(),
        trust,
        workflows,
        replay_key,
    })))
}
