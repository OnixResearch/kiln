//! Bounded GitHub issue polling recovery for implementation work.
//!
//! Polling is a recovery path, not an authority grant: results are normalized
//! into the same work contract and must still pass enrollment, admission and
//! ledger deduplication. The transport is supplied by the shell; this module
//! bounds pages, records and text and never performs HTTP itself.

use std::collections::{BTreeMap, BTreeSet};

use kiln_core::implementation::{AdmissionDecision, IMPLEMENTATION_WORK_SCHEMA};
use kiln_core::types::ContentId;
use kiln_implementer::Implementer;
use kiln_provider_api::{PortFailure, PortResult};
use serde::Serialize;

use crate::domain;
use crate::implementation::{ImplementationIssuePolicy, IssueAdmission, issue_work};
use crate::infrastructure;

pub const MAX_POLL_PAGES: u32 = 20;
pub const MAX_POLL_RECORDS: usize = 200;
pub const MAX_POLL_PAGE_SIZE: u8 = 100;
const MAX_TITLE_BYTES: usize = 512;
const MAX_BODY_BYTES: usize = 262_144;
const MAX_ACTOR_BYTES: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssuePollRequest {
    pub owner: String,
    pub repository: String,
    pub since_ms: u64,
    pub page: u32,
    pub per_page: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssuePollRecord {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub updated_at_ms: u64,
    pub actor: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssuePollPage {
    pub records: Vec<IssuePollRecord>,
    /// Next page number when the provider reports more records.
    pub next_page: Option<u32>,
}

/// Transport port. Implementations own authentication, bounded HTTP and
/// retries; this module owns normalization and admission bounds.
pub trait IssuePollTransport {
    fn list_issues(&mut self, request: &IssuePollRequest) -> PortResult<IssuePollPage>;
}

/// Poll up to `max_pages` pages and return normalized observations sorted by
/// `(updated_at_ms, number)`.
///
/// Duplicate records across pages keep the highest update sequence. Exceeding
/// the record bound fails instead of silently truncating recovery.
// r[impl kiln.implementation.polling]
pub fn poll_implementation_issues(
    policy: &ImplementationIssuePolicy,
    enrolled_actors: &BTreeSet<String>,
    transport: &mut impl IssuePollTransport,
    since_ms: u64,
    max_pages: u32,
) -> PortResult<Vec<IssueAdmission>> {
    if max_pages == 0 || max_pages > MAX_POLL_PAGES {
        return Err(refusal(
            "github_poll_page_bound",
            "poll.max_pages",
            "polling page bound is zero or exceeds its maximum",
        ));
    }
    let mut records: BTreeMap<u64, IssuePollRecord> = BTreeMap::new();
    let mut page = 1;
    let mut pages = 0;
    while pages < max_pages {
        let response = transport.list_issues(&IssuePollRequest {
            owner: policy.repository.owner.clone(),
            repository: policy.repository.repository.clone(),
            since_ms,
            page,
            per_page: MAX_POLL_PAGE_SIZE,
        })?;
        pages += 1;
        if response.records.len() > usize::from(MAX_POLL_PAGE_SIZE) {
            return Err(refusal(
                "github_poll_record_bound",
                "poll.records",
                "polling page exceeds its record bound",
            ));
        }
        for record in response.records {
            validate_record(&record, enrolled_actors)?;
            match records.get(&record.number) {
                Some(existing) if existing.updated_at_ms >= record.updated_at_ms => {}
                _ => {
                    records.insert(record.number, record);
                }
            }
            if records.len() > MAX_POLL_RECORDS {
                return Err(refusal(
                    "github_poll_record_bound",
                    "poll.records",
                    "polling recovery exceeds its record bound",
                ));
            }
        }
        match response.next_page {
            Some(next) if next > page => {
                if pages >= max_pages {
                    return Err(refusal(
                        "github_poll_page_bound",
                        "poll.max_pages",
                        "polling recovery exceeded its page bound with records remaining",
                    ));
                }
                page = next;
            }
            _ => break,
        }
    }
    let repository_key = format!(
        "{}/{}",
        policy.repository.owner.to_ascii_lowercase(),
        policy.repository.repository.to_ascii_lowercase()
    );
    let mut ordered: Vec<IssuePollRecord> = records.into_values().collect();
    ordered.sort_by_key(|record| (record.updated_at_ms, record.number));
    let mut admissions = Vec::with_capacity(ordered.len());
    for record in ordered {
        let native_id = format!("{repository_key}#{}", record.number);
        let source_version = record_identity(&repository_key, &record)?;
        if record.state == "closed" {
            admissions.push(IssueAdmission::Closed {
                native_id,
                sequence: record.updated_at_ms,
                source_version,
            });
        } else {
            admissions.push(IssueAdmission::Work(Box::new(issue_work(
                policy,
                record.actor,
                repository_key.clone(),
                native_id,
                record.title,
                record.updated_at_ms,
                source_version,
            )?)));
        }
    }
    Ok(admissions)
}

/// Poll and admit one bounded recovery pass into the durable work ledger.
///
/// Repeated polling is idempotent through the work ledger. Closed issues are
/// observed but never cancel or complete work; cancellation remains a separate
/// consumer.
// r[impl kiln.implementation.polling]
pub fn poll_into_implementer(
    policy: &ImplementationIssuePolicy,
    enrolled_actors: &BTreeSet<String>,
    transport: &mut impl IssuePollTransport,
    since_ms: u64,
    max_pages: u32,
    implementer: &mut Implementer,
) -> PortResult<PollSummary> {
    let admissions =
        poll_implementation_issues(policy, enrolled_actors, transport, since_ms, max_pages)?;
    let mut summary = PollSummary::default();
    for admission in admissions {
        match admission {
            IssueAdmission::Work(work) => match implementer.admit(&work, None) {
                Ok(AdmissionDecision::Admitted { .. }) => summary.admitted += 1,
                Ok(AdmissionDecision::Duplicate { .. }) => summary.duplicate += 1,
                Ok(AdmissionDecision::Superseded { .. }) => summary.superseded += 1,
                Ok(AdmissionDecision::Stale { .. }) => summary.stale += 1,
                Err(error) => {
                    return Err(infrastructure(
                        "github_poll_ledger",
                        &format!("polled work could not be admitted: {error}"),
                    ));
                }
            },
            IssueAdmission::Closed { .. } => summary.closed += 1,
        }
    }
    Ok(summary)
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct PollSummary {
    pub admitted: usize,
    pub duplicate: usize,
    pub superseded: usize,
    pub stale: usize,
    pub closed: usize,
}

fn validate_record(record: &IssuePollRecord, enrolled_actors: &BTreeSet<String>) -> PortResult<()> {
    if record.number == 0
        || record.updated_at_ms == 0
        || record.title.trim().is_empty()
        || record.title.len() > MAX_TITLE_BYTES
        || record
            .body
            .as_deref()
            .is_some_and(|body| body.len() > MAX_BODY_BYTES)
        || record.actor.is_empty()
        || record.actor.len() > MAX_ACTOR_BYTES
        || record.actor.chars().any(char::is_control)
        || !matches!(record.state.as_str(), "open" | "closed")
        || !enrolled_actors.contains(&record.actor)
    {
        return Err(refusal(
            "github_poll_record",
            "poll.record",
            "polled issue is malformed, oversized or not enrolled",
        ));
    }
    Ok(())
}

fn record_identity(repository: &str, record: &IssuePollRecord) -> PortResult<ContentId> {
    let mut bytes = Vec::new();
    for value in [
        IMPLEMENTATION_WORK_SCHEMA,
        repository,
        record.state.as_str(),
        record.actor.as_str(),
        record.title.as_str(),
    ] {
        push(&mut bytes, value)?;
    }
    bytes.extend_from_slice(&record.number.to_be_bytes());
    bytes.extend_from_slice(&record.updated_at_ms.to_be_bytes());
    push(&mut bytes, record.body.as_deref().unwrap_or_default())?;
    ContentId::from_blake3_hex(blake3::hash(&bytes).to_hex().to_string()).map_err(|_| {
        refusal(
            "github_poll_identity",
            "poll.record",
            "polled issue identity could not be encoded",
        )
    })
}

fn push(bytes: &mut Vec<u8>, value: &str) -> PortResult<()> {
    let length = u32::try_from(value.len()).map_err(|_| {
        refusal(
            "github_poll_encoding",
            "poll.record",
            "polled issue field exceeds the encoding bound",
        )
    })?;
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn refusal(code: &str, field: &str, message: &str) -> PortFailure {
    domain(code, field, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementation::IssueAdmission;
    use kiln_core::implementation::{AcceptanceObligation, TerminalPolicy, WorkBudget};
    use std::collections::BTreeSet;

    fn policy() -> ImplementationIssuePolicy {
        ImplementationIssuePolicy {
            repository: crate::events::EventPolicy {
                owner: "OnixResearch".to_string(),
                repository: "kiln".to_string(),
                repository_id: 42,
                allow_fork_pull_requests: false,
                untrusted_pull_requests: Default::default(),
            },
            project: "kiln".to_string(),
            base: None,
            requested_outcome: "implement".to_string(),
            acceptance: vec![AcceptanceObligation {
                id: "accepted".to_string(),
                statement: "verified".to_string(),
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
            priority: 1,
        }
    }

    struct Fake {
        pages: BTreeMap<u32, IssuePollPage>,
        calls: usize,
    }

    impl IssuePollTransport for Fake {
        fn list_issues(&mut self, request: &IssuePollRequest) -> PortResult<IssuePollPage> {
            self.calls += 1;
            if request.per_page != MAX_POLL_PAGE_SIZE {
                return Err(refusal("page_size", "poll.per_page", "wrong page size"));
            }
            Ok(self.pages.remove(&request.page).unwrap_or(IssuePollPage {
                records: Vec::new(),
                next_page: None,
            }))
        }
    }

    fn record(number: u64, updated_at_ms: u64, state: &str) -> IssuePollRecord {
        IssuePollRecord {
            number,
            title: format!("issue {number}"),
            body: None,
            state: state.to_string(),
            updated_at_ms,
            actor: "operator".to_string(),
        }
    }

    // r[verify kiln.implementation.polling.scenario.pages]
    #[test]
    fn two_pages_normalize_open_and_closed_issues() {
        let mut pages = BTreeMap::new();
        pages.insert(
            1,
            IssuePollPage {
                records: vec![record(1, 10, "open")],
                next_page: Some(2),
            },
        );
        pages.insert(
            2,
            IssuePollPage {
                records: vec![record(2, 20, "closed")],
                next_page: None,
            },
        );
        let mut transport = Fake { pages, calls: 0 };
        let admissions = poll_implementation_issues(
            &policy(),
            &BTreeSet::from(["operator".to_string()]),
            &mut transport,
            0,
            MAX_POLL_PAGES,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(transport.calls, 2);
        assert_eq!(admissions.len(), 2);
        assert!(matches!(admissions.first(), Some(IssueAdmission::Work(_))));
        assert!(matches!(
            admissions.get(1),
            Some(IssueAdmission::Closed { .. })
        ));
    }

    #[test]
    fn duplicate_pages_keep_the_highest_sequence() {
        let mut pages = BTreeMap::new();
        pages.insert(
            1,
            IssuePollPage {
                records: vec![record(1, 10, "closed"), record(1, 30, "open")],
                next_page: None,
            },
        );
        let mut transport = Fake { pages, calls: 0 };
        let admissions = poll_implementation_issues(
            &policy(),
            &BTreeSet::from(["operator".to_string()]),
            &mut transport,
            0,
            MAX_POLL_PAGES,
        )
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(admissions.len(), 1);
        assert!(matches!(admissions.first(), Some(IssueAdmission::Work(_))));
    }

    // r[verify kiln.implementation.polling.scenario.bound]
    #[test]
    fn unenrolled_actor_and_page_bound_are_refused() {
        let mut pages = BTreeMap::new();
        pages.insert(
            1,
            IssuePollPage {
                records: vec![record(1, 10, "open")],
                next_page: None,
            },
        );
        let mut transport = Fake { pages, calls: 0 };
        assert!(
            poll_implementation_issues(
                &policy(),
                &BTreeSet::from(["someone".to_string()]),
                &mut transport,
                0,
                MAX_POLL_PAGES,
            )
            .is_err()
        );
        let mut transport = Fake {
            pages: BTreeMap::new(),
            calls: 0,
        };
        assert!(
            poll_implementation_issues(
                &policy(),
                &BTreeSet::from(["operator".to_string()]),
                &mut transport,
                0,
                0,
            )
            .is_err()
        );
    }

    struct Repeat {
        page: IssuePollPage,
    }

    impl IssuePollTransport for Repeat {
        fn list_issues(&mut self, _request: &IssuePollRequest) -> PortResult<IssuePollPage> {
            Ok(self.page.clone())
        }
    }

    // r[verify kiln.github.poll.admission] r[verify kiln.github.poll.admission.scenario.repeat]
    #[test]
    fn repeated_polling_admits_one_canonical_work_version() {
        let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let database = directory.path().join("work.sqlite");
        let enrollment = kiln_core::implementation::Enrollment {
            repositories: BTreeSet::from(["onixresearch/kiln".to_string()]),
            actors: BTreeSet::from(["operator".to_string()]),
        };
        let mut implementer =
            Implementer::open(&database, enrollment.clone()).unwrap_or_else(|_| unreachable!());
        let mut transport = Repeat {
            page: IssuePollPage {
                records: vec![record(7, 100, "open")],
                next_page: None,
            },
        };
        let first = poll_into_implementer(
            &policy(),
            &enrollment.actors,
            &mut transport,
            0,
            1,
            &mut implementer,
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(first.admitted, 1);
        let second = poll_into_implementer(
            &policy(),
            &enrollment.actors,
            &mut transport,
            0,
            1,
            &mut implementer,
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(second.duplicate, 1);
        assert_eq!(
            implementer
                .work()
                .unwrap_or_else(|_| unreachable!())
                .current()
                .count(),
            1
        );
    }

    // r[verify kiln.github.poll.admission.scenario.closed]
    #[test]
    fn closed_poll_observation_does_not_complete_or_cancel_work() {
        let directory = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
        let database = directory.path().join("work.sqlite");
        let enrollment = kiln_core::implementation::Enrollment {
            repositories: BTreeSet::from(["onixresearch/kiln".to_string()]),
            actors: BTreeSet::from(["operator".to_string()]),
        };
        let mut implementer =
            Implementer::open(&database, enrollment.clone()).unwrap_or_else(|_| unreachable!());
        let mut transport = Repeat {
            page: IssuePollPage {
                records: vec![record(8, 100, "closed")],
                next_page: None,
            },
        };
        let summary = poll_into_implementer(
            &policy(),
            &enrollment.actors,
            &mut transport,
            0,
            1,
            &mut implementer,
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert_eq!(summary.closed, 1);
        assert_eq!(
            implementer
                .work()
                .unwrap_or_else(|_| unreachable!())
                .current()
                .count(),
            0
        );
    }

    #[test]
    fn record_identity_is_stable_and_content_bound() {
        let mut first = record(7, 100, "open");
        let one = record_identity("onix/kiln", &first).unwrap_or_else(|_| unreachable!());
        let two = record_identity("onix/kiln", &first).unwrap_or_else(|_| unreachable!());
        assert_eq!(one, two);
        first.title = "changed".to_string();
        let three = record_identity("onix/kiln", &first).unwrap_or_else(|_| unreachable!());
        assert_ne!(one, three);
    }
}
