//! Authenticated GitHub issue intake for implementation work.
//!
//! Admission proves only that the delivery signature, repository, actor and
//! payload bounds matched the reviewed policy and that one normalized
//! `ImplementationWork` was produced. Acceptance obligations, permitted paths,
//! effects, verification profile and budget come from operator policy, never
//! from issue text. Work claims and execution are separate consumers.

use std::collections::BTreeSet;

use kiln_core::implementation::{
    AcceptanceObligation, IMPLEMENTATION_WORK_SCHEMA, ImplementationWork, SourceRef,
    TerminalPolicy, WorkBudget, WorkSourceKind,
};
use kiln_core::types::{ContentId, ExternalRef};
use kiln_provider_api::PortResult;
use serde::Deserialize;

use crate::domain;
use crate::events::{
    DeliveryHeaders, EventPolicy, admit_event_policy, authenticate, valid_delivery,
    valid_event_name,
};

const MAX_BODY_BYTES: usize = 1_048_576;
const MAX_ISSUE_BODY_BYTES: usize = 262_144;
const MAX_ISSUE_TITLE_BYTES: usize = 512;
const MAX_ACTOR_BYTES: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImplementationIssuePolicy {
    pub repository: EventPolicy,
    pub project: String,
    /// Operator-supplied outcome. Issue text is not execution permission.
    pub requested_outcome: String,
    pub acceptance: Vec<AcceptanceObligation>,
    pub dependencies: Vec<ContentId>,
    pub permitted_paths: Vec<String>,
    pub permitted_effects: BTreeSet<String>,
    pub verification_profile: ContentId,
    pub budget: WorkBudget,
    pub terminal_policy: TerminalPolicy,
    pub priority: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IssueAdmission {
    Work(Box<ImplementationWork>),
    /// The issue was closed. Cancellation semantics belong to a later
    /// consumer; closing never proves implementation completion.
    Closed {
        native_id: String,
        sequence: u64,
        source_version: ContentId,
    },
}

#[derive(Deserialize)]
struct Envelope {
    action: String,
    issue: Issue,
    repository: Repository,
    sender: Actor,
}

#[derive(Deserialize)]
struct Actor {
    login: String,
}

#[derive(Deserialize)]
struct Issue {
    number: u64,
    title: String,
    #[serde(default)]
    body: Option<String>,
    state: String,
    updated_at: String,
}

#[derive(Deserialize)]
struct Repository {
    id: u64,
    name: String,
    owner: Actor,
}

/// Admit one authenticated GitHub `issues` delivery.
///
/// Returns `None` for unrelated events or unsupported actions. Duplicate,
/// out-of-order and linked-work deduplication belong to
/// [`kiln_core::implementation::WorkLedger`].
// r[impl kiln.implementation.contract] r[impl kiln.implementation.issue_intake] r[impl kiln.github.authentication]
pub fn admit_implementation_issue(
    policy: &ImplementationIssuePolicy,
    enrolled_actors: &BTreeSet<String>,
    headers: &DeliveryHeaders,
    body: &[u8],
    secret: &[u8],
) -> PortResult<Option<IssueAdmission>> {
    admit_event_policy(&policy.repository)?;
    if body.is_empty() || body.len() > MAX_BODY_BYTES {
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
    if headers.event != "issues" {
        let _: serde::de::IgnoredAny = serde_json::from_slice(body).map_err(|_| {
            domain(
                "github_event_payload",
                "body",
                "GitHub event payload is malformed or inconsistent",
            )
        })?;
        return Ok(None);
    }
    let envelope: Envelope = serde_json::from_slice(body).map_err(|_| {
        domain(
            "github_event_payload",
            "body",
            "GitHub issue payload is malformed or inconsistent",
        )
    })?;
    if envelope.repository.id != policy.repository.repository_id
        || !envelope
            .repository
            .owner
            .login
            .eq_ignore_ascii_case(&policy.repository.owner)
        || !envelope
            .repository
            .name
            .eq_ignore_ascii_case(&policy.repository.repository)
    {
        return Err(domain(
            "github_repository_mismatch",
            "repository",
            "GitHub event repository does not match the admitted policy",
        ));
    }
    let action = match envelope.action.as_str() {
        "opened" | "edited" | "reopened" | "closed" => envelope.action.as_str(),
        _ => return Ok(None),
    };
    if envelope.issue.number == 0 {
        return Err(invalid_issue());
    }
    if !bounded(&envelope.issue.title, MAX_ISSUE_TITLE_BYTES)
        || envelope
            .issue
            .body
            .as_deref()
            .is_some_and(|body| body.len() > MAX_ISSUE_BODY_BYTES)
        || !valid_actor(&envelope.sender.login)
        || !enrolled_actors.contains(&envelope.sender.login)
    {
        return Err(domain(
            "github_implementation_actor",
            "sender.login",
            "issue actor is malformed or not enrolled for implementation",
        ));
    }
    let sequence = timestamp_millis(&envelope.issue.updated_at).ok_or_else(invalid_issue)?;
    let repository_key = format!(
        "{}/{}",
        policy.repository.owner.to_ascii_lowercase(),
        policy.repository.repository.to_ascii_lowercase()
    );
    let native_id = format!("{repository_key}#{}", envelope.issue.number);
    let source_version = ContentId::from_blake3_hex(blake3::hash(body).to_hex().to_string())
        .map_err(|_| invalid_issue())?;
    if action == "closed" {
        if envelope.issue.state != "closed" {
            return Err(invalid_issue());
        }
        return Ok(Some(IssueAdmission::Closed {
            native_id,
            sequence,
            source_version,
        }));
    }
    if envelope.issue.state != "open" {
        return Err(invalid_issue());
    }
    let alias = SourceRef {
        kind: WorkSourceKind::GitHubIssue,
        source: ExternalRef::new("github", "issue", &native_id).map_err(|_| invalid_issue())?,
        native_id: native_id.clone(),
        version: source_version,
    };
    Ok(Some(IssueAdmission::Work(Box::new(ImplementationWork {
        schema: IMPLEMENTATION_WORK_SCHEMA.to_string(),
        revision: sequence,
        actor: envelope.sender.login,
        repository: repository_key,
        project: policy.project.clone(),
        title: envelope.issue.title,
        requested_outcome: policy.requested_outcome.clone(),
        acceptance: policy.acceptance.clone(),
        dependencies: policy.dependencies.clone(),
        permitted_paths: policy.permitted_paths.clone(),
        permitted_effects: policy.permitted_effects.clone(),
        verification_profile: policy.verification_profile.clone(),
        budget: policy.budget,
        terminal_policy: policy.terminal_policy,
        priority: policy.priority,
        aliases: vec![alias],
    }))))
}

fn invalid_issue() -> kiln_provider_api::PortFailure {
    domain(
        "github_issue_payload",
        "issue",
        "GitHub issue payload is malformed or inconsistent",
    )
}

fn bounded(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.contains('\0')
}

fn valid_actor(value: &str) -> bool {
    bounded(value, MAX_ACTOR_BYTES) && !value.chars().any(char::is_control)
}

/// Strict `YYYY-MM-DDTHH:MM:SSZ` to Unix milliseconds.
fn timestamp_millis(value: &str) -> Option<u64> {
    let bytes = value.as_bytes();
    if bytes.len() != 20
        || bytes.get(4) != Some(&b'-')
        || bytes.get(7) != Some(&b'-')
        || bytes.get(10) != Some(&b'T')
        || bytes.get(13) != Some(&b':')
        || bytes.get(16) != Some(&b':')
        || bytes.get(19) != Some(&b'Z')
    {
        return None;
    }
    let number = |start: usize| -> Option<i64> {
        let slice = bytes.get(start..start + 2)?;
        if !slice.iter().all(u8::is_ascii_digit) {
            return None;
        }
        let text = core::str::from_utf8(slice).ok()?;
        text.parse().ok()
    };
    let year = {
        let slice = bytes.get(0..4)?;
        if !slice.iter().all(u8::is_ascii_digit) {
            return None;
        }
        core::str::from_utf8(slice).ok()?.parse::<i64>().ok()?
    };
    let month = number(5)?;
    let day = number(8)?;
    let hour = number(11)?;
    let minute = number(14)?;
    let second = number(17)?;
    if !(1970..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    // Howard Hinnant's days-from-civil algorithm, valid for the admitted range.
    let year = year - if month <= 2 { 1 } else { 0 };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let adjusted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * adjusted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    let seconds = days * 86_400 + hour * 3_600 + minute * 60 + second;
    u64::try_from(seconds).ok().map(|seconds| seconds * 1_000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    fn policy() -> ImplementationIssuePolicy {
        ImplementationIssuePolicy {
            repository: EventPolicy {
                owner: "OnixResearch".to_string(),
                repository: "kiln".to_string(),
                repository_id: 42,
                allow_fork_pull_requests: false,
                untrusted_pull_requests: Default::default(),
            },
            project: "kiln".to_string(),
            requested_outcome: "implement the admitted work".to_string(),
            acceptance: vec![AcceptanceObligation {
                id: "accepted".to_string(),
                statement: "the change is verified".to_string(),
            }],
            dependencies: Vec::new(),
            permitted_paths: vec!["crates".to_string()],
            permitted_effects: BTreeSet::from(["write_source".to_string()]),
            verification_profile: ContentId::from_blake3_hex("a".repeat(64))
                .unwrap_or_else(|_| unreachable!()),
            budget: WorkBudget {
                attempts: 2,
                minutes_per_attempt: 60,
            },
            terminal_policy: TerminalPolicy::VerifiedPullRequest,
            priority: 7,
        }
    }

    fn headers(body: &[u8], secret: &[u8]) -> DeliveryHeaders {
        let mut mac = Hmac::<Sha256>::new_from_slice(secret).unwrap_or_else(|_| unreachable!());
        mac.update(body);
        DeliveryHeaders {
            delivery: "00000000-0000-0000-0000-000000000001".to_string(),
            event: "issues".to_string(),
            signature: format!("sha256={}", hex(&mac.finalize().into_bytes())),
        }
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn body(action: &str, state: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "action": action,
            "repository": {"id": 42, "name": "kiln", "owner": {"login": "OnixResearch"}},
            "sender": {"login": "operator"},
            "issue": {
                "number": 44,
                "title": "Admit implementation work",
                "body": "issue intent",
                "state": state,
                "updated_at": "2026-11-11T10:00:00Z"
            }
        }))
        .unwrap_or_else(|_| unreachable!())
    }

    fn actors() -> BTreeSet<String> {
        BTreeSet::from(["operator".to_string()])
    }

    // r[verify kiln.implementation.issue_intake.scenario.admit]
    #[test]
    fn authenticated_issue_becomes_one_bounded_work_record() {
        let body = body("opened", "open");
        let admission = admit_implementation_issue(
            &policy(),
            &actors(),
            &headers(&body, b"secret"),
            &body,
            b"secret",
        )
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
        let IssueAdmission::Work(work) = admission else {
            unreachable!("open issue must produce work")
        };
        assert_eq!(work.revision, 1_794_391_200_000);
        assert_eq!(work.actor, "operator");
        assert_eq!(work.repository, "onixresearch/kiln");
        assert_eq!(work.aliases.len(), 1);
        assert_eq!(
            work.aliases.first().map(|alias| alias.kind),
            Some(WorkSourceKind::GitHubIssue)
        );
        assert!(work.identity().is_ok());
    }

    // r[verify kiln.implementation.issue_intake.scenario.refused]
    #[test]
    fn forged_signature_actor_and_repository_are_refused() {
        let body = body("opened", "open");
        let mut forged = headers(&body, b"secret");
        forged.signature = format!("sha256={}", "0".repeat(64));
        assert!(
            admit_implementation_issue(&policy(), &actors(), &forged, &body, b"secret").is_err()
        );
        let mut stranger = policy();
        stranger.repository.repository_id = 43;
        assert!(
            admit_implementation_issue(
                &stranger,
                &actors(),
                &headers(&body, b"secret"),
                &body,
                b"secret"
            )
            .is_err()
        );
        let unenrolled = BTreeSet::from(["someone-else".to_string()]);
        assert!(
            admit_implementation_issue(
                &policy(),
                &unenrolled,
                &headers(&body, b"secret"),
                &body,
                b"secret"
            )
            .is_err()
        );
    }

    // r[verify kiln.implementation.issue_intake.scenario.lifecycle]
    #[test]
    fn edits_are_versioned_and_closing_never_claims_completion() {
        let edit = body("edited", "open");
        let admission = admit_implementation_issue(
            &policy(),
            &actors(),
            &headers(&edit, b"secret"),
            &edit,
            b"secret",
        )
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
        assert!(
            matches!(admission, IssueAdmission::Work(work) if work.revision == 1_794_391_200_000)
        );
        let closed = body("closed", "closed");
        let admission = admit_implementation_issue(
            &policy(),
            &actors(),
            &headers(&closed, b"secret"),
            &closed,
            b"secret",
        )
        .unwrap_or_else(|_| unreachable!())
        .unwrap_or_else(|| unreachable!());
        assert!(matches!(admission, IssueAdmission::Closed { .. }));
    }

    #[test]
    fn non_issue_events_and_unsupported_actions_are_ignored() {
        let body = body("labeled", "open");
        assert_eq!(
            admit_implementation_issue(
                &policy(),
                &actors(),
                &headers(&body, b"secret"),
                &body,
                b"secret"
            )
            .unwrap_or_else(|_| unreachable!()),
            None
        );
        let mut unrelated = headers(&body, b"secret");
        unrelated.event = "push".to_string();
        assert_eq!(
            admit_implementation_issue(&policy(), &actors(), &unrelated, &body, b"secret")
                .unwrap_or_else(|_| unreachable!()),
            None
        );
    }
}
