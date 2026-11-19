//! Restart-safe mirror journal for attempt claims and releases.
//!
//! The journal records one claim and one release per exact attempt fence. A
//! reconcile pass posts only missing transitions, so restart, retry and
//! reconciliation do not duplicate an article. An `unknown` post is recorded and
//! never blindly repeated.

use std::collections::BTreeMap;
use std::path::Path;

use kiln_core::implementation::swarm::{
    AttemptSnapshot, ClaimScope, claim_subject, format_claim_body, format_release_body,
    from_address, release_subject,
};
use serde::{Deserialize, Serialize};

use crate::{Article, PostOutcome, PostReceipt, SwarmClient, SwarmFailure};

const JOURNAL_SCHEMA: &str = "kiln.swarm-mirror-journal.v1";
const MESSAGE_ID_SKEW_MS: u64 = 10 * 60 * 1000;

/// Transport operations the reconciler needs. Implemented by the real client
/// and by test doubles.
pub trait MirrorTransport {
    /// Read the newest `limit` articles of the claim group.
    ///
    /// # Errors
    /// Returns a failure when the group is unavailable.
    fn read_recent(&self, limit: usize) -> Result<Vec<Article>, SwarmFailure>;

    /// Post one article and return its recorded outcome.
    fn post(&self, subject: &str, references: Option<&str>, body: &str) -> PostReceipt;
}

impl MirrorTransport for SwarmClient {
    fn read_recent(&self, limit: usize) -> Result<Vec<Article>, SwarmFailure> {
        Self::read_recent(self, limit)
    }

    fn post(&self, subject: &str, references: Option<&str>, body: &str) -> PostReceipt {
        Self::post(self, subject, references, body)
    }
}

/// One recorded post.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MirrorPost {
    pub subject: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    pub outcome: String,
    pub posted_at_ms: u64,
}

/// Journal entry for one exact attempt fence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MirrorEntry {
    pub attempt: String,
    pub fence: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim: Option<MirrorPost>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<MirrorPost>,
}

/// Durable mirror journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MirrorJournal {
    pub schema: String,
    #[serde(default)]
    pub entries: BTreeMap<String, MirrorEntry>,
}

impl Default for MirrorJournal {
    fn default() -> Self {
        Self {
            schema: JOURNAL_SCHEMA.to_owned(),
            entries: BTreeMap::new(),
        }
    }
}

impl MirrorJournal {
    /// Load a journal, returning an empty journal when the file is absent.
    ///
    /// # Errors
    /// Returns a diagnostic for unreadable or invalid journal bytes.
    pub fn load(path: &Path) -> Result<Self, String> {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|_| "swarm mirror journal is invalid".to_owned()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(_) => Err("swarm mirror journal is unreadable".to_owned()),
        }
    }

    /// Atomically write the journal through a sibling temporary file.
    ///
    /// # Errors
    /// Returns a diagnostic when the journal cannot be serialized or replaced.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|_| "swarm mirror journal cannot be encoded".to_owned())?;
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, bytes)
            .map_err(|_| "swarm mirror journal cannot be written".to_owned())?;
        std::fs::rename(&temporary, path)
            .map_err(|_| "swarm mirror journal cannot be replaced".to_owned())
    }
}

/// Reconcile inputs that do not vary per attempt.
#[derive(Debug, Clone)]
pub struct MirrorContext<'a> {
    pub from: &'a str,
    pub host: &'a str,
    pub now_ms: u64,
    pub read_limit: usize,
}

/// One action taken by a reconcile pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirrorAction {
    pub attempt: String,
    pub fence: u64,
    pub kind: &'static str,
    pub outcome: PostOutcome,
}

/// Post missing claim and release transitions, updating the journal in place.
///
/// One claim is posted per exact attempt fence and one release per closed
/// attempt. `unknown` and `rejected` outcomes are recorded and not retried.
#[must_use]
pub fn reconcile(
    transport: &dyn MirrorTransport,
    journal: &mut MirrorJournal,
    attempts: &[AttemptSnapshot],
    context: &MirrorContext<'_>,
) -> Vec<MirrorAction> {
    let mut actions = Vec::new();
    for attempt in attempts {
        let key = format!("{}:{}", attempt.identity, attempt.fence);
        let entry = journal.entries.entry(key).or_insert_with(|| MirrorEntry {
            attempt: attempt.identity.clone(),
            fence: attempt.fence,
            claim: None,
            release: None,
        });
        let scope = ClaimScope {
            repository: attempt.repository.clone(),
            scope: attempt.scope.clone(),
        };
        if entry.claim.is_none() && !attempt.terminal {
            let Some(subject) = claim_subject(&scope.repository, &scope.scope).ok() else {
                continue;
            };
            let facts = kiln_core::implementation::swarm::ClaimBodyFacts {
                scope: &scope,
                work: &attempt.identity,
                attempt: &attempt.identity,
                base_revision: &attempt.base_revision,
                lease_minutes: attempt.lease_minutes,
                host: context.host,
            };
            let body = match format_claim_body(&facts) {
                Ok(body) => body,
                Err(_) => continue,
            };
            let receipt = transport.post(&subject, None, &body);
            let message_id = if receipt.outcome == PostOutcome::Accepted {
                resolve_message_id(transport, context, &subject, context.now_ms)
            } else {
                None
            };
            entry.claim = Some(MirrorPost {
                subject,
                message_id,
                outcome: receipt.outcome.as_str().to_owned(),
                posted_at_ms: context.now_ms,
            });
            actions.push(MirrorAction {
                attempt: attempt.identity.clone(),
                fence: attempt.fence,
                kind: "claim",
                outcome: receipt.outcome,
            });
        }
        if entry.release.is_none()
            && attempt.terminal
            && entry
                .claim
                .as_ref()
                .is_some_and(|claim| claim.outcome != PostOutcome::Rejected.as_str())
        {
            let Some(subject) = release_subject(&scope.repository, &scope.scope).ok() else {
                continue;
            };
            let body = match format_release_body(
                &scope,
                &attempt.identity,
                "attempt closed",
                context.host,
            ) {
                Ok(body) => body,
                Err(_) => continue,
            };
            let references = entry
                .claim
                .as_ref()
                .and_then(|claim| claim.message_id.as_deref());
            let receipt = transport.post(&subject, references, &body);
            entry.release = Some(MirrorPost {
                subject,
                message_id: None,
                outcome: receipt.outcome.as_str().to_owned(),
                posted_at_ms: context.now_ms,
            });
            actions.push(MirrorAction {
                attempt: attempt.identity.clone(),
                fence: attempt.fence,
                kind: "release",
                outcome: receipt.outcome,
            });
        }
    }
    actions
}

fn resolve_message_id(
    transport: &dyn MirrorTransport,
    context: &MirrorContext<'_>,
    subject: &str,
    posted_at_ms: u64,
) -> Option<String> {
    let articles = transport.read_recent(context.read_limit).ok()?;
    let ours = from_address(context.from);
    articles
        .iter()
        .filter(|article| {
            article.subject == subject
                && from_address(&article.from) == ours
                && article.view().is_some_and(|view| {
                    view.date_unix_ms.saturating_add(MESSAGE_ID_SKEW_MS) >= posted_at_ms
                })
        })
        .max_by_key(|article| article.number)
        .map(|article| article.message_id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AttemptSnapshot;
    use std::cell::RefCell;

    struct Fake {
        articles: RefCell<Vec<Article>>,
        posts: RefCell<Vec<(String, Option<String>)>>,
        reads: RefCell<Vec<usize>>,
        outcome: PostOutcome,
    }

    impl Fake {
        fn new(outcome: PostOutcome) -> Self {
            Self {
                articles: RefCell::new(Vec::new()),
                posts: RefCell::new(Vec::new()),
                reads: RefCell::new(Vec::new()),
                outcome,
            }
        }
    }

    impl MirrorTransport for Fake {
        /// Return the newest `limit` articles, as the real newest window does.
        fn read_recent(&self, limit: usize) -> Result<Vec<Article>, SwarmFailure> {
            self.reads.borrow_mut().push(limit);
            let articles = self.articles.borrow();
            let start = articles.len().saturating_sub(limit);
            Ok(articles.iter().skip(start).cloned().collect())
        }

        fn post(&self, subject: &str, references: Option<&str>, _body: &str) -> PostReceipt {
            self.posts
                .borrow_mut()
                .push((subject.to_owned(), references.map(str::to_owned)));
            PostReceipt {
                outcome: self.outcome,
                detail: None,
            }
        }
    }

    fn attempt(terminal: bool) -> AttemptSnapshot {
        AttemptSnapshot {
            identity: "attempt-a".to_owned(),
            fence: 3,
            terminal,
            repository: "OnixResearch/kiln".to_owned(),
            scope: "crates/kiln-core".to_owned(),
            base_revision: "3a6047ef".to_owned(),
            lease_minutes: 60,
        }
    }

    fn context() -> MirrorContext<'static> {
        MirrorContext {
            from: "kiln-aspen1 <kiln-aspen1@swarm.local>",
            host: "aspen1",
            now_ms: 1_000_000,
            read_limit: 20,
        }
    }

    #[test]
    fn claim_then_release_posts_once_each() {
        let transport = Fake::new(PostOutcome::Accepted);
        transport.articles.borrow_mut().push(Article {
            number: 7,
            message_id: "<claim@aspen1>".to_owned(),
            from: "kiln-aspen1 <kiln-aspen1@swarm.local>".to_owned(),
            subject: "CLAIM OnixResearch/kiln crates/kiln-core".to_owned(),
            date: "Wed, 18 Nov 2026 23:01:00 +0000".to_owned(),
            references: String::new(),
        });
        let mut journal = MirrorJournal::default();
        let first = reconcile(&transport, &mut journal, &[attempt(false)], &context());
        assert_eq!(first.len(), 1);
        let entry = journal
            .entries
            .get("attempt-a:3")
            .unwrap_or_else(|| unreachable!());
        assert_eq!(
            entry
                .claim
                .as_ref()
                .and_then(|claim| claim.message_id.as_deref()),
            Some("<claim@aspen1>")
        );

        let second = reconcile(&transport, &mut journal, &[attempt(true)], &context());
        assert_eq!(second.len(), 1);
        let posts = transport.posts.borrow();
        assert_eq!(posts.len(), 2);
        let release = posts.get(1).unwrap_or_else(|| unreachable!());
        assert_eq!(release.0, "RELEASE OnixResearch/kiln crates/kiln-core");
        assert_eq!(release.1.as_deref(), Some("<claim@aspen1>"));

        let third = reconcile(&transport, &mut journal, &[attempt(true)], &context());
        assert_eq!(third.len(), 0);
    }

    #[test]
    // r[verify kiln.implementation.swarm.transport.scenario.window]
    fn message_id_resolves_from_the_newest_window() {
        let transport = Fake::new(PostOutcome::Accepted);
        {
            let mut articles = transport.articles.borrow_mut();
            for number in 1..=40 {
                articles.push(Article {
                    number,
                    message_id: format!("<other{number}@aspen3>"),
                    from: "omp <aspen3@swarm.local>".to_owned(),
                    subject: "CLAIM OnixResearch/kiln crates/other".to_owned(),
                    date: "Wed, 18 Nov 2026 23:01:00 +0000".to_owned(),
                    references: String::new(),
                });
            }
            articles.push(Article {
                number: 41,
                message_id: "<newest@aspen1>".to_owned(),
                from: "kiln-aspen1 <kiln-aspen1@swarm.local>".to_owned(),
                subject: "CLAIM OnixResearch/kiln crates/kiln-core".to_owned(),
                date: "Wed, 18 Nov 2026 23:01:00 +0000".to_owned(),
                references: String::new(),
            });
        }
        let mut journal = MirrorJournal::default();
        let actions = reconcile(&transport, &mut journal, &[attempt(false)], &context());
        assert_eq!(actions.len(), 1);
        assert_eq!(*transport.reads.borrow(), vec![context().read_limit]);
        let message_id = journal
            .entries
            .get("attempt-a:3")
            .and_then(|entry| entry.claim.as_ref())
            .and_then(|claim| claim.message_id.clone());
        assert_eq!(message_id.as_deref(), Some("<newest@aspen1>"));
    }

    #[test]
    fn unknown_claim_is_not_reposted() {
        let transport = Fake::new(PostOutcome::Unknown);
        let mut journal = MirrorJournal::default();
        assert_eq!(
            reconcile(&transport, &mut journal, &[attempt(false)], &context()).len(),
            1
        );
        assert_eq!(
            reconcile(&transport, &mut journal, &[attempt(false)], &context()).len(),
            0
        );
        assert_eq!(transport.posts.borrow().len(), 1);
    }

    #[test]
    fn journal_round_trips_atomically() {
        let directory = std::env::temp_dir().join(format!("kiln-mirror-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap_or_else(|_| unreachable!());
        let path = directory.join("journal.json");
        let mut journal = MirrorJournal::default();
        journal.entries.insert(
            "a:1".to_owned(),
            MirrorEntry {
                attempt: "a".to_owned(),
                fence: 1,
                claim: Some(MirrorPost {
                    subject: "CLAIM x y".to_owned(),
                    message_id: Some("<m@x>".to_owned()),
                    outcome: "accepted".to_owned(),
                    posted_at_ms: 1,
                }),
                release: None,
            },
        );
        journal.save(&path).unwrap_or_else(|_| unreachable!());
        let loaded = MirrorJournal::load(&path).unwrap_or_else(|_| unreachable!());
        assert_eq!(loaded, journal);
        std::fs::remove_dir_all(&directory).unwrap_or_else(|_| unreachable!());
    }
}
