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
    #[serde(default)]
    pub allow_fork_pull_requests: bool,
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedEvent {
    pub delivery_id: String,
    pub body_identity: ContentId,
    pub candidate_identity: ContentId,
    /// Canonical target repository, not an unauthenticated header or branch URL.
    pub repository: String,
    pub repository_id: u64,
    pub revision: String,
    pub native_repository: NativeRepository,
    pub selector: EventSelector,
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
    {
        return Err(domain(
            "github_event_policy",
            "repository",
            "GitHub repository policy is invalid",
        ));
    }
    Ok(())
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
            if !matches!(
                envelope.action.as_str(),
                "opened" | "reopened" | "synchronize"
            ) {
                return Ok(None);
            }
            let event: PullRequestEvent = decode(body)?;
            let pr = event.pull_request;
            if event.number == 0 || event.number != pr.number {
                return Err(invalid_payload());
            }
            admit_repository(policy, &pr.base.repo)?;
            if pr.state == "closed" {
                return Ok(None);
            }
            if pr.state != "open"
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
            if !same_id && !policy.allow_fork_pull_requests {
                return Err(domain(
                    "github_fork_denied",
                    "pull_request.head.repo",
                    "GitHub fork pull request execution is not permitted",
                ));
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
    let candidate_identity =
        candidate_identity(&repository_key, policy.repository_id, &revision, &selector)?;
    Ok(Some(VerifiedEvent {
        delivery_id: headers.delivery.to_ascii_lowercase(),
        body_identity: content_identity(blake3::hash(body))?,
        candidate_identity,
        repository: repository_key,
        repository_id: repository.id,
        revision,
        native_repository: native_repository(repository),
        selector,
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
        }
    }
    content_identity(hash.finalize())
}

#[cfg(test)]
mod tests;
