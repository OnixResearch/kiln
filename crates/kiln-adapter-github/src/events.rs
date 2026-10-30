//! GitHub webhook event admission.
//!
//! `VerifiedEvent` is a construction proof: only this module's admission
//! functions build one, every field is private, and the type is neither
//! serializable nor deserializable.
//!
//! ## Admitted proof-type audit
//!
//! The other `Admitted*` types were audited with this change:
//! `AdmittedGithubConfig`, `AdmittedRadicleConfig` and
//! `AdmittedAspenRuntimeProfile` are proofs of reviewed profile admission, and
//! `AdmittedJournal` is a proof that a journal document was admitted; each is to
//! be sealed in a separate change. `AdmittedResult` is a plain record embedded
//! in persisted semantic-merge state, not the output of a check, so it keeps its
//! fields and deserialization.
//!
// r[impl kiln.github.event_proof.audit]
// r[verify kiln.github.event_proof.audit.scenario.names]
use hmac::{Hmac, Mac};
use kiln_core::types::ContentId;
use kiln_provider_api::PortResult;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::domain;

const MAX_HEADER_BYTES: usize = 64;
const MAX_SECRET_BYTES: usize = 65_536;
const MAX_REFERENCE_BYTES: usize = 1024;

/// Repository names are matched case-insensitively, as on GitHub. Durable
/// repository/candidate keys use ASCII lowercase; native spelling is retained.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventPolicy {
    pub owner: String,
    pub repository: String,
    pub repository_id: u64,
    /// Retired constant, kept only so existing profiles replay byte-identically.
    /// It must stay `false`; `untrusted_pull_requests` is the reviewed policy.
    #[serde(default)]
    pub allow_fork_pull_requests: bool,
    // r[impl kiln.untrusted.policy]
    #[serde(
        default,
        skip_serializing_if = "kiln_core::untrusted::UntrustedPullRequests::is_deny"
    )]
    pub untrusted_pull_requests: kiln_core::untrusted::UntrustedPullRequests,
}

/// The HTTP boundary must reject repeated headers before constructing this type.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryHeaders {
    pub delivery: String,
    pub event: String,
    pub signature: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRepository {
    pub owner: String,
    pub name: String,
    pub id: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EventSelector {
    Push {
        reference: String,
    },
    PullRequest {
        number: u64,
        head_repository: NativeRepository,
        head_reference: String,
        base_reference: String,
    },
}

// r[impl kiln.github.event_proof]
/// An admitted webhook event: signature, selection and identity checks passed.
///
/// Only [`admit_event`] and [`admit_event_with`] construct one. Every field is
/// private, and the type implements neither `Serialize` nor `Deserialize`, so
/// another crate cannot fabricate authentication with a struct literal or JSON.
///
/// ```
/// # use kiln_adapter_github::events::VerifiedEvent;
/// let read: fn(&VerifiedEvent) -> &str = VerifiedEvent::delivery_id;
/// # let _ = read;
/// ```
///
// r[verify kiln.github.event_proof.scenario.forgery]
/// A struct literal fails because the fields are private:
///
/// ```compile_fail
/// # use kiln_adapter_github::events::{EventSelector, NativeRepository, VerifiedEvent};
/// let _ = VerifiedEvent {
///     delivery_id: String::new(),
///     body_identity: unreachable!(),
///     candidate_identity: unreachable!(),
///     repository: String::new(),
///     repository_id: 0,
///     revision: String::new(),
///     native_repository: NativeRepository { owner: String::new(), name: String::new(), id: 0 },
///     selector: EventSelector::Push { reference: String::new() },
///     untrusted: None,
///     pull_request_action: None,
/// };
/// ```
///
/// Deserialization fails because the type is not `Deserialize`:
///
/// ```compile_fail
/// # use kiln_adapter_github::events::VerifiedEvent;
/// let _ = serde_json::from_str::<VerifiedEvent>("{}");
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedEvent {
    delivery_id: String,
    body_identity: ContentId,
    candidate_identity: ContentId,
    /// Canonical target repository, not an unauthenticated header or branch URL.
    repository: String,
    repository_id: u64,
    revision: String,
    native_repository: NativeRepository,
    selector: EventSelector,
    /// Why an admitted pull request is untrusted; absent when trusted.
    untrusted: Option<kiln_core::untrusted::UntrustedReason>,
    /// The pull request action; absent for pushes. Closed and merged pull
    /// requests appear only through [`admit_event_with`] when requested.
    pull_request_action: Option<kiln_core::trigger_filter::PullRequestType>,
}

impl VerifiedEvent {
    /// The authenticated delivery identity used by replay admission.
    pub fn delivery_id(&self) -> &str {
        &self.delivery_id
    }
    /// BLAKE3 of the original body bytes.
    pub fn body_identity(&self) -> &ContentId {
        &self.body_identity
    }
    /// Candidate identity of the selected source.
    pub fn candidate_identity(&self) -> &ContentId {
        &self.candidate_identity
    }
    /// Canonical target repository (`owner/name`).
    pub fn repository(&self) -> &str {
        &self.repository
    }
    /// Numeric repository id the policy admitted.
    pub fn repository_id(&self) -> u64 {
        self.repository_id
    }
    /// The selected exact revision.
    pub fn revision(&self) -> &str {
        &self.revision
    }
    /// The provider-native repository facts the event carried.
    pub fn native_repository(&self) -> &NativeRepository {
        &self.native_repository
    }
    /// The selected push or pull-request selector.
    pub fn selector(&self) -> &EventSelector {
        &self.selector
    }
    /// Why an admitted pull request is untrusted; absent when trusted.
    pub fn untrusted(&self) -> Option<kiln_core::untrusted::UntrustedReason> {
        self.untrusted
    }
    /// The pull request action; absent for pushes.
    pub fn pull_request_action(&self) -> Option<kiln_core::trigger_filter::PullRequestType> {
        self.pull_request_action
    }
}

// These wire-only structs deliberately allow GitHub's unrelated fields. Serde's
// struct decoder rejects duplicates of every selected field, unlike Value maps.
#[derive(Deserialize)]
struct Owner {
    login: String,
}

#[derive(Deserialize)]
struct Repository {
    id: u64,
    name: String,
    full_name: String,
    owner: Owner,
}

#[derive(Deserialize)]
struct Push {
    repository: Repository,
    #[serde(rename = "ref")]
    reference: String,
    after: String,
    deleted: bool,
}

#[derive(Deserialize)]
struct PullRequestEnvelope {
    action: String,
    repository: Repository,
}

#[derive(Deserialize)]
struct PullRequestEvent {
    repository: Repository,
    number: u64,
    pull_request: PullRequest,
}

#[derive(Deserialize)]
struct PullRequest {
    number: u64,
    state: String,
    head: PullRequestSide,
    base: PullRequestSide,
    /// Signed push-access evidence; absent means no evidence (fail closed).
    #[serde(default)]
    author_association: Option<String>,
    /// Whether a closed pull request was merged.
    #[serde(default)]
    merged: Option<bool>,
}

#[derive(Deserialize)]
struct PullRequestSide {
    #[serde(rename = "ref")]
    reference: String,
    sha: String,
    repo: Repository,
}

// r[impl kiln.github.events]
pub fn admit_event_policy(policy: &EventPolicy) -> PortResult<()> {
    if !valid_owner(&policy.owner)
        || !valid_repository_name(&policy.repository)
        || policy.owner.len() + 1 + policy.repository.len() > kiln_core::types::DEFAULT_MAX_ID_BYTES
        || policy.repository_id == 0
        || policy.allow_fork_pull_requests
    {
        return Err(domain(
            "github_event_policy",
            "repository",
            "GitHub repository policy is invalid",
        ));
    }
    Ok(())
}

/// Which optional event classes a registration asked for. The default admits
/// exactly what [`admit_event`] always admitted.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EventAdmission {
    /// Admit `closed` pull requests as `closed` or `merged` actions.
    pub closed_pull_requests: bool,
}

// r[impl kiln.github.authentication]
// r[impl kiln.github.events]
pub fn admit_event(
    policy: &EventPolicy,
    headers: &DeliveryHeaders,
    body: &[u8],
    secret: &[u8],
    maximum_body_bytes: usize,
) -> PortResult<Option<VerifiedEvent>> {
    admit_event_with(
        policy,
        headers,
        body,
        secret,
        maximum_body_bytes,
        EventAdmission::default(),
    )
}

// r[impl kiln.github.authentication]
// r[impl kiln.github.events]
// r[impl kiln.triggers.evaluate]
pub fn admit_event_with(
    policy: &EventPolicy,
    headers: &DeliveryHeaders,
    body: &[u8],
    secret: &[u8],
    maximum_body_bytes: usize,
    admission: EventAdmission,
) -> PortResult<Option<VerifiedEvent>> {
    admit_event_policy(policy)?;
    if maximum_body_bytes == 0 || body.is_empty() || body.len() > maximum_body_bytes {
        return Err(domain(
            "github_body_bound",
            "body",
            "GitHub event body is outside its admitted bound",
        ));
    }
    if !valid_delivery(&headers.delivery) || !valid_event_name(&headers.event) {
        return Err(domain(
            "github_event_headers",
            "headers",
            "GitHub delivery or event header is invalid",
        ));
    }
    authenticate(&headers.signature, body, secret)?;
    // Authentication precedes all payload decoding and identity construction.

    let mut untrusted = None;
    let mut pull_request_action = None;
    let (repository, revision, selector) = match headers.event.as_str() {
        "push" => {
            let push: Push = decode(body)?;
            admit_repository(policy, &push.repository)?;
            if !valid_push_reference(&push.reference) || !valid_sha(&push.after) {
                return Err(invalid_payload());
            }
            let zero = push.after.bytes().all(|byte| byte == b'0');
            if push.deleted {
                if !zero {
                    return Err(invalid_payload());
                }
                return Ok(None);
            }
            if zero {
                return Err(invalid_payload());
            }
            (
                push.repository,
                push.after,
                EventSelector::Push {
                    reference: push.reference,
                },
            )
        }
        "pull_request" => {
            let envelope: PullRequestEnvelope = decode(body)?;
            admit_repository(policy, &envelope.repository)?;
            use kiln_core::trigger_filter::PullRequestType;
            let action = match envelope.action.as_str() {
                "opened" => PullRequestType::Opened,
                "reopened" => PullRequestType::Reopened,
                "synchronize" => PullRequestType::Synchronize,
                "closed" if admission.closed_pull_requests => PullRequestType::Closed,
                _ => return Ok(None),
            };
            let closing = action == PullRequestType::Closed;
            let event: PullRequestEvent = decode(body)?;
            let pr = event.pull_request;
            if event.number == 0 || event.number != pr.number {
                return Err(invalid_payload());
            }
            admit_repository(policy, &pr.base.repo)?;
            if pr.state == "closed" && !closing {
                return Ok(None);
            }
            pull_request_action = Some(if closing && pr.merged == Some(true) {
                PullRequestType::Merged
            } else {
                action
            });
            if pr.state != if closing { "closed" } else { "open" }
                || !valid_branch_reference(&pr.head.reference)
                || !valid_branch_reference(&pr.base.reference)
                || !nonzero_sha(&pr.head.sha)
                || !nonzero_sha(&pr.base.sha)
            {
                return Err(invalid_payload());
            }
            validate_repository(&pr.head.repo)?;
            let same_id = pr.head.repo.id == policy.repository_id;
            let same_name = repository_matches(policy, &pr.head.repo);
            if same_id != same_name {
                return Err(invalid_payload());
            }
            // r[impl kiln.untrusted.classification]
            match kiln_core::untrusted::classify(
                policy.untrusted_pull_requests,
                kiln_core::untrusted::PullRequestTrustFacts {
                    same_repository: same_id,
                    author_association: pr.author_association.as_deref(),
                },
            ) {
                kiln_core::untrusted::TrustDecision::Trusted => {}
                kiln_core::untrusted::TrustDecision::Refused => {
                    return Err(domain(
                        "github_fork_denied",
                        "pull_request.head.repo",
                        "GitHub fork pull request execution is not permitted",
                    ));
                }
                kiln_core::untrusted::TrustDecision::Untrusted(reason) => untrusted = Some(reason),
            }
            (
                event.repository,
                pr.head.sha,
                EventSelector::PullRequest {
                    number: event.number,
                    head_repository: native_repository(pr.head.repo),
                    head_reference: pr.head.reference,
                    base_reference: pr.base.reference,
                },
            )
        }
        _ => {
            // Even ignored events must contain one complete JSON value. Unknown
            // event schemas are not interpreted or granted execution authority.
            let _: serde::de::IgnoredAny = decode(body)?;
            return Ok(None);
        }
    };
    let repository_key = format!(
        "{}/{}",
        policy.owner.to_ascii_lowercase(),
        policy.repository.to_ascii_lowercase()
    );
    let candidate_identity = candidate_identity(
        &repository_key,
        policy.repository_id,
        &revision,
        &selector,
        pull_request_action,
    )?;
    Ok(Some(VerifiedEvent {
        delivery_id: headers.delivery.to_ascii_lowercase(),
        body_identity: content_identity(blake3::hash(body))?,
        candidate_identity,
        repository: repository_key,
        repository_id: repository.id,
        revision,
        native_repository: native_repository(repository),
        selector,
        untrusted,
        pull_request_action,
    }))
}

fn authenticate(signature: &str, body: &[u8], secret: &[u8]) -> PortResult<()> {
    let failure = || {
        domain(
            "github_authentication",
            "signature",
            "GitHub event authentication failed",
        )
    };
    if secret.is_empty() || secret.len() > MAX_SECRET_BYTES {
        return Err(failure());
    }
    let encoded = signature
        .strip_prefix("sha256=")
        .filter(|value| value.len() == 64)
        .ok_or_else(failure)?;
    let mut expected = [0_u8; 32];
    for (pair, output) in encoded.as_bytes().chunks_exact(2).zip(&mut expected) {
        let [high, low] = pair else {
            return Err(failure());
        };
        let high = hex_digit(*high).ok_or_else(failure)?;
        let low = hex_digit(*low).ok_or_else(failure)?;
        *output = high * 16 + low;
    }
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).map_err(|_| failure())?;
    mac.update(body);
    // Mac::verify_slice performs the MAC comparison in constant time.
    mac.verify_slice(&expected).map_err(|_| failure())
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn valid_delivery(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

fn valid_event_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_HEADER_BYTES
        && value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn valid_owner(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 39
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn valid_repository_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && !matches!(value, "." | "..")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn valid_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| hex_digit(byte).is_some())
}

fn nonzero_sha(value: &str) -> bool {
    valid_sha(value) && value.bytes().any(|byte| byte != b'0')
}

fn valid_push_reference(value: &str) -> bool {
    (value.starts_with("refs/heads/") || value.starts_with("refs/tags/"))
        && valid_branch_reference(value)
}

fn valid_branch_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_REFERENCE_BYTES
        && !value.ends_with('.')
        && !value.contains("..")
        && !value.contains("@{")
        && !value.bytes().any(|byte| {
            byte <= b' '
                || byte == 127
                || matches!(byte, b'~' | b'^' | b':' | b'?' | b'*' | b'[' | b'\\')
        })
        && value
            .split('/')
            .all(|part| !part.is_empty() && !part.starts_with('.') && !part.ends_with(".lock"))
}

fn validate_repository(repository: &Repository) -> PortResult<()> {
    if repository.id == 0
        || !valid_owner(&repository.owner.login)
        || !valid_repository_name(&repository.name)
    {
        return Err(invalid_payload());
    }
    let Some((owner, name)) = repository.full_name.split_once('/') else {
        return Err(invalid_payload());
    };
    if !owner.eq_ignore_ascii_case(&repository.owner.login)
        || !name.eq_ignore_ascii_case(&repository.name)
    {
        return Err(invalid_payload());
    }
    Ok(())
}

fn repository_matches(policy: &EventPolicy, repository: &Repository) -> bool {
    policy.owner.eq_ignore_ascii_case(&repository.owner.login)
        && policy.repository.eq_ignore_ascii_case(&repository.name)
}

fn admit_repository(policy: &EventPolicy, repository: &Repository) -> PortResult<()> {
    validate_repository(repository)?;
    if repository.id != policy.repository_id || !repository_matches(policy, repository) {
        return Err(domain(
            "github_repository_mismatch",
            "repository",
            "GitHub event repository does not match the admitted policy",
        ));
    }
    Ok(())
}

fn native_repository(repository: Repository) -> NativeRepository {
    NativeRepository {
        owner: repository.owner.login,
        name: repository.name,
        id: repository.id,
    }
}

fn decode<'de, T: Deserialize<'de>>(body: &'de [u8]) -> PortResult<T> {
    serde_json::from_slice(body).map_err(|_| invalid_payload())
}

fn invalid_payload() -> kiln_provider_api::PortFailure {
    domain(
        "github_event_payload",
        "body",
        "GitHub event payload is malformed or inconsistent",
    )
}

fn content_identity(hash: blake3::Hash) -> PortResult<ContentId> {
    ContentId::from_blake3_hex(hash.to_hex().to_string()).map_err(|_| {
        domain(
            "github_event_identity",
            "identity",
            "GitHub event identity could not be constructed",
        )
    })
}

// r[impl kiln.github.replay]
fn candidate_identity(
    repository: &str,
    repository_id: u64,
    revision: &str,
    selector: &EventSelector,
    action: Option<kiln_core::trigger_filter::PullRequestType>,
) -> PortResult<ContentId> {
    let mut hash = blake3::Hasher::new();
    let mut field = |bytes: &[u8]| {
        hash.update(&(bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    };
    field(b"kiln.github.candidate.v1");
    field(repository.as_bytes());
    field(&repository_id.to_le_bytes());
    field(revision.as_bytes());
    match selector {
        EventSelector::Push { reference } => {
            field(b"push");
            field(reference.as_bytes());
        }
        EventSelector::PullRequest {
            number,
            head_repository,
            head_reference,
            base_reference,
        } => {
            field(b"pull_request");
            field(&number.to_le_bytes());
            field(&head_repository.id.to_le_bytes());
            field(head_repository.owner.to_ascii_lowercase().as_bytes());
            field(head_repository.name.to_ascii_lowercase().as_bytes());
            field(head_reference.as_bytes());
            field(base_reference.as_bytes());
            // Closing actions never replay the open/synchronize candidate of
            // the same head; earlier identities stay unchanged.
            if let Some(
                action @ (kiln_core::trigger_filter::PullRequestType::Closed
                | kiln_core::trigger_filter::PullRequestType::Merged),
            ) = action
            {
                field(action_name(action));
            }
        }
    }
    content_identity(hash.finalize())
}

fn action_name(action: kiln_core::trigger_filter::PullRequestType) -> &'static [u8] {
    use kiln_core::trigger_filter::PullRequestType;
    match action {
        PullRequestType::Opened => b"opened",
        PullRequestType::Reopened => b"reopened",
        PullRequestType::Synchronize => b"synchronize",
        PullRequestType::Closed => b"closed",
        PullRequestType::Merged => b"merged",
    }
}

#[cfg(test)]
mod tests;
