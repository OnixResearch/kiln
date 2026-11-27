//! Swarm link: the newest claim window, foreign claims, fleet mirror posts
//! and decision announcements.
//!
//! Kiln posts with its own From address. A post whose outcome is unknown is
//! never repeated: its message id is learned later from the newest window by
//! subject, From and fence.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::sync::{Arc, Mutex};

use kiln_adapter_swarm::{Article, PostOutcome, PostReceipt, SwarmClient, SwarmConfig};
use kiln_core::fleet::{
    ClaimEffect, ClaimState, ClaimVerb, FOREIGN_CLAIM_STALE_MS, FleetClaimRegistry, ForeignClaim,
    MirrorStatus, parse_scope, scope_subject,
};
use kiln_core::implementation::swarm::{
    ClaimArticle, from_address, parse_rfc2822_ms, parse_subject,
};
use kiln_core::types::ContentId;

use crate::exec::Child;

/// A window younger than this is reused instead of read again.
pub const WINDOW_REFRESH_MS: u64 = 30_000;
/// A window older than this is not trusted for conflicts.
pub const WINDOW_MAX_AGE_MS: u64 = 10 * 60_000;
/// Clock skew allowed between Kiln and the swarm server when matching our
/// own articles to claims.
pub const ARTICLE_CLOCK_SKEW_MS: u64 = 5 * 60_000;
/// Bytes of a decision article body.
pub const MAX_DECISION_BODY_BYTES: usize = 4_096;

/// One configured swarm.
#[derive(Debug)]
pub struct SwarmLink {
    client: SwarmClient,
    own_address: String,
    window: Mutex<Option<(u64, Arc<[Article]>)>>,
}

impl SwarmLink {
    /// # Errors
    /// Returns the swarm configuration diagnostic.
    pub fn new(config: &SwarmConfig) -> Result<Self, String> {
        Ok(Self {
            client: SwarmClient::new(config.clone())?,
            own_address: from_address(&config.from).to_ascii_lowercase(),
            window: Mutex::new(None),
        })
    }

    #[must_use]
    pub fn own_address(&self) -> &str {
        &self.own_address
    }

    /// Read the newest window now.
    ///
    /// # Errors
    /// Returns the read failure; a failed read is never an empty window.
    pub fn refresh(&self, now_ms: u64) -> Result<Arc<[Article]>, String> {
        let limit = self.client.config().read_limit;
        let articles: Arc<[Article]> = self
            .client
            .read_recent(limit)
            .map_err(|failure| format!("swarm read failed: {}", failure.detail))?
            .into();
        if let Ok(mut window) = self.window.lock() {
            *window = Some((now_ms, articles.clone()));
        }
        Ok(articles)
    }

    /// The newest window: reused while fresh, re-read otherwise, and the last
    /// good window while it is younger than [`WINDOW_MAX_AGE_MS`].
    ///
    /// # Errors
    /// Returns the read failure when no window is recent enough.
    pub fn articles(&self, now_ms: u64) -> Result<Arc<[Article]>, String> {
        let cached = self.window.lock().ok().and_then(|window| window.clone());
        if let Some((read_ms, articles)) = &cached
            && now_ms.saturating_sub(*read_ms) < WINDOW_REFRESH_MS
        {
            return Ok(articles.clone());
        }
        match self.refresh(now_ms) {
            Ok(articles) => Ok(articles),
            Err(error) => match cached {
                Some((read_ms, articles)) if now_ms.saturating_sub(read_ms) < WINDOW_MAX_AGE_MS => {
                    Ok(articles)
                }
                _ => Err(error),
            },
        }
    }

    /// Open claims of other authors in the newest window.
    ///
    /// # Errors
    /// Returns the read failure.
    pub fn foreign(&self, now_ms: u64) -> Result<Vec<ForeignClaim>, String> {
        Ok(self.foreign_in(&self.articles(now_ms)?, now_ms))
    }

    pub(crate) fn foreign_in(&self, articles: &[Article], now_ms: u64) -> Vec<ForeignClaim> {
        foreign_claims(articles, &self.own_address, now_ms)
    }

    /// Post one fleet article.
    #[must_use]
    pub fn post(&self, effect: &ClaimEffect) -> PostReceipt {
        let (subject, references, body) = match effect {
            ClaimEffect::PostClaim { subject, body, .. } => (subject, None, body),
            ClaimEffect::PostRelease {
                subject,
                references,
                body,
                ..
            } => (subject, references.as_deref(), body),
        };
        self.client.post(subject, references, body)
    }

    /// Post a free-form article to `group`. The swarm client admits claim
    /// subjects only, so decisions run the same pinned client, environment
    /// and bounds directly.
    ///
    /// # Errors
    /// Returns a bounded diagnostic; an unknown outcome is not retried.
    pub fn post_article(&self, group: &str, subject: &str, body: &str) -> Result<(), String> {
        if body.len() > MAX_DECISION_BODY_BYTES
            || subject.bytes().any(|byte| byte.is_ascii_control())
            || body
                .bytes()
                .any(|byte| byte.is_ascii_control() && byte != b'\n')
        {
            return Err("decision article is oversized or has control characters".to_owned());
        }
        let config = self.client.config();
        let mut args: Vec<OsString> = vec!["--state-dir".into(), config.state_dir.clone().into()];
        if let Some(file) = &config.config_file {
            args.extend(["--config".into(), file.clone().into_os_string()]);
        }
        if let Some(server) = &config.server {
            args.extend(["--server".into(), server.into()]);
        }
        if config.no_relays {
            args.push("--no-relays".into());
        }
        args.extend([
            "post".into(),
            "--group".into(),
            group.into(),
            "--subject".into(),
            subject.into(),
        ]);
        let mut child = Child::new(
            &config.executable,
            &config.state_dir,
            config.timeout_ms,
            config.max_output_bytes,
        )
        .args(args)
        .input(body.as_bytes().to_vec());
        for (name, value) in config.child_environment() {
            child = child.env(name, value);
        }
        child.run()?.require_success().map(|_| ())
    }
}

/// Open `CLAIM` articles in `articles` from authors other than
/// `own_address`, younger than the stale bound and not referenced by a
/// `RELEASE`, whose scope parses as a fleet scope.
#[must_use]
pub fn foreign_claims(
    articles: &[Article],
    own_address: &str,
    now_ms: u64,
) -> Vec<ForeignClaim> {
    let released: BTreeSet<&str> = articles
        .iter()
        .filter(|article| matches!(parse_subject(&article.subject), ClaimArticle::Release(_)))
        .filter(|article| parse_rfc2822_ms(&article.date).is_some())
        .flat_map(|article| article.references.split_whitespace())
        .collect();
    articles
        .iter()
        .filter_map(|article| {
            let ClaimArticle::Claim(scope) = parse_subject(&article.subject) else {
                return None;
            };
            let date_unix_ms = parse_rfc2822_ms(&article.date)?;
            if from_address(&article.from).eq_ignore_ascii_case(own_address)
                || released.contains(article.message_id.as_str())
                || now_ms.saturating_sub(date_unix_ms) > FOREIGN_CLAIM_STALE_MS
            {
                return None;
            }
            let scope = parse_scope(&scope.repository, &scope.scope).ok()?;
            Some(ForeignClaim {
                scope,
                message_id: article.message_id.clone(),
                author: article.from.clone(),
                date_unix_ms,
            })
        })
        .collect()
}

/// Message ids learned for our posts whose id is not yet recorded:
/// `(claim, release, message_id)`. Each article is used for at most one
/// post, the earliest one at or after the post's reference time matching the
/// post's exact fence (`initial_fence()` for `CLAIM`, `fence` for `RELEASE`).
#[must_use]
pub fn learned_message_ids(
    registry: &FleetClaimRegistry,
    articles: &[Article],
    own_address: &str,
) -> Vec<(ContentId, bool, String)> {
    let mut used: BTreeSet<String> = registry
        .claims
        .values()
        .flat_map(|claim| [&claim.claim_mirror, &claim.release_mirror])
        .filter_map(|status| match status {
            MirrorStatus::Posted {
                message_id: Some(id),
            } => Some(id.clone()),
            _ => None,
        })
        .collect();
    let mut learned = Vec::new();
    for claim in registry.claims.values() {
        let release_at = match &claim.state {
            ClaimState::Released { at_ms, .. } | ClaimState::Expired { at_ms } => Some(*at_ms),
            ClaimState::Open => None,
        };
        let posts = [
            (
                false,
                &claim.claim_mirror,
                Some(claim.granted_ms),
                ClaimVerb::Claim,
                claim.initial_fence(),
            ),
            (
                true,
                &claim.release_mirror,
                release_at,
                ClaimVerb::Release,
                claim.fence,
            ),
        ];
        for (release, status, reference, verb, expected_fence) in posts {
            let Some(reference) = reference else {
                continue;
            };
            if !matches!(
                status,
                MirrorStatus::Unknown | MirrorStatus::Posted { message_id: None }
            ) {
                continue;
            }
            let subject = scope_subject(verb, &claim.scope);
            let found = articles
                .iter()
                .filter(|article| {
                    article.subject == subject
                        && from_address(&article.from).eq_ignore_ascii_case(own_address)
                        && fleet_fence(&article.body) == Some(expected_fence)
                        && !used.contains(&article.message_id)
                })
                .filter_map(|article| Some((parse_rfc2822_ms(&article.date)?, article)))
                .filter(|(date, _)| date.saturating_add(ARTICLE_CLOCK_SKEW_MS) >= reference)
                .min_by_key(|(date, article)| (*date, article.number));
            if let Some((_, article)) = found {
                used.insert(article.message_id.clone());
                learned.push((claim.identity.clone(), release, article.message_id.clone()));
            }
        }
    }
    learned
}

fn fleet_fence(body: &str) -> Option<u64> {
    let mut lines = body.lines();
    // Fleet articles have a summary, optional purpose, then holder and fence.
    let summary = lines.next()?;
    let holder = if summary.starts_with("Claiming ") {
        lines.next()?.strip_prefix("purpose: ")?;
        lines.next()?
    } else if summary.starts_with("Releasing ") {
        lines.next()?
    } else {
        return None;
    };
    holder.strip_prefix("holder: ")?;
    lines.next()?.strip_prefix("fence: ")?.parse().ok()
}

/// The mirror status recorded for one post receipt; `None` leaves the post
/// pending because nothing reached the server.
#[must_use]
pub fn mirror_status(receipt: PostReceipt) -> Option<MirrorStatus> {
    match receipt.outcome {
        PostOutcome::Accepted => Some(MirrorStatus::Posted {
            message_id: receipt.message_id,
        }),
        PostOutcome::Rejected => Some(MirrorStatus::Failed),
        PostOutcome::Unknown => Some(MirrorStatus::Unknown),
        PostOutcome::Unavailable => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiln_core::fleet::{ClaimCommand, ClaimContext, ClaimOutcome, FleetAgent, decide};

    const NOW: u64 = 1_795_042_860_000;
    const RECENT: &str = "Wed, 18 Nov 2026 23:00:59 +0000";

    fn article(id: &str, from: &str, subject: &str, date: &str, references: &str) -> Article {
        Article {
            number: 1,
            message_id: id.to_owned(),
            from: from.to_owned(),
            subject: subject.to_owned(),
            date: date.to_owned(),
            references: references.to_owned(),
            body: String::new(),
        }
    }

    #[test]
    // r[verify kiln.fleet.claims.scenario.conflict]
    fn foreign_claims_are_open_recent_and_not_ours() {
        let articles = [
            article(
                "<a@x>",
                "omp <aspen3@swarm.local>",
                "CLAIM aspen3 system",
                RECENT,
                "",
            ),
            article(
                "<b@x>",
                "kiln <kiln@swarm.local>",
                "CLAIM OnixResearch/kiln docs",
                RECENT,
                "",
            ),
            article(
                "<c@x>",
                "omp <aspen1@swarm.local>",
                "CLAIM OnixResearch/kiln crates",
                RECENT,
                "",
            ),
            article(
                "<d@x>",
                "omp <aspen1@swarm.local>",
                "RELEASE OnixResearch/kiln crates",
                RECENT,
                "<c@x>",
            ),
            article(
                "<e@x>",
                "omp <aspen1@swarm.local>",
                "CLAIM OnixResearch/old x",
                "Tue, 17 Nov 2026 22:59:59 +0000",
                "",
            ),
            article("<f@x>", "omp <aspen1@swarm.local>", "hello", RECENT, ""),
        ];
        let foreign = foreign_claims(&articles, "kiln@swarm.local", NOW);
        assert_eq!(foreign.len(), 1);
        assert_eq!(
            foreign.first().map(|claim| claim.message_id.as_str()),
            Some("<a@x>")
        );
    }

    #[test]
    // r[verify kiln.fleet.mirror.scenario.articles]
    fn unknown_posts_learn_their_id_from_our_newest_articles() {
        let scope = parse_scope("OnixResearch/kiln", "docs").unwrap_or_else(|_| unreachable!());
        let holder = FleetAgent {
            tool: "omp".into(),
            host: "aspen3".into(),
            session: "s1".into(),
        };
        let context = ClaimContext {
            now_ms: NOW,
            attempts: &[],
            foreign: &[],
            coordinator: "britton-desktop",
        };
        let (registry, outcome, _) = decide(
            &FleetClaimRegistry::default(),
            ClaimCommand::Acquire {
                scope,
                holder,
                purpose: "docs".into(),
                lease_ms: 60_000,
                attempt: None,
            },
            &context,
        )
        .unwrap_or_else(|_| unreachable!());
        let ClaimOutcome::Granted(claim) = outcome else {
            unreachable!()
        };
        let (registry, _, _) = decide(
            &registry,
            ClaimCommand::MirrorObserved {
                identity: claim.identity.clone(),
                release: false,
                status: MirrorStatus::Unknown,
            },
            &context,
        )
        .unwrap_or_else(|_| unreachable!());
        let observed = |id: &str, from: &str, minute: &str, fence| Article {
            number: 1,
            message_id: id.to_owned(),
            from: from.to_owned(),
            subject: "CLAIM onixresearch/kiln docs".to_owned(),
            date: format!("Wed, 18 Nov 2026 {minute}:00 +0000"),
            references: String::new(),
            body: format!(
                "Claiming onixresearch/kiln docs through the Kiln fleet.\npurpose: docs\nholder: omp on aspen3\nfence: {fence}\n"
            ),
        };
        let articles = [
            observed(
                "<other@x>",
                "omp <aspen3@swarm.local>",
                "23:01",
                claim.fence,
            ),
            observed("<old@x>", "kiln <kiln@swarm.local>", "22:55", claim.fence),
            observed(
                "<other-lease@x>",
                "kiln <kiln@swarm.local>",
                "23:00",
                claim.fence + 1,
            ),
            observed("<ours@x>", "kiln <kiln@swarm.local>", "23:01", claim.fence),
        ];
        let learned = learned_message_ids(&registry, &articles, "kiln@swarm.local");
        assert_eq!(
            learned,
            vec![(claim.identity, false, "<ours@x>".to_owned())]
        );
        let nonmatching = articles.get(..3).unwrap_or(&[]);
        assert!(learned_message_ids(&registry, nonmatching, "kiln@swarm.local").is_empty());
        assert_eq!(
            mirror_status(PostReceipt {
                outcome: PostOutcome::Unavailable,
                detail: None,
                message_id: None,
            }),
            None
        );
        assert_eq!(
            mirror_status(PostReceipt {
                outcome: PostOutcome::Unknown,
                detail: None,
                message_id: None,
            }),
            Some(MirrorStatus::Unknown)
        );
        assert_eq!(
            mirror_status(PostReceipt {
                outcome: PostOutcome::Accepted,
                detail: None,
                message_id: Some("<ack@x>".to_owned()),
            }),
            Some(MirrorStatus::Posted {
                message_id: Some("<ack@x>".to_owned())
            })
        );
    }

    #[test]
    // r[verify kiln.fleet.mirror]
    fn renewed_claim_with_bumped_fence_learns_original_claim_id_and_threads_release() {
        let scope = parse_scope("OnixResearch/kiln", "docs").unwrap_or_else(|_| unreachable!());
        let holder = FleetAgent {
            tool: "omp".into(),
            host: "aspen3".into(),
            session: "s1".into(),
        };
        let context = ClaimContext {
            now_ms: NOW,
            attempts: &[],
            foreign: &[],
            coordinator: "britton-desktop",
        };
        let (registry, outcome, _) = decide(
            &FleetClaimRegistry::default(),
            ClaimCommand::Acquire {
                scope,
                holder,
                purpose: "docs".into(),
                lease_ms: 60_000,
                attempt: None,
            },
            &context,
        )
        .unwrap_or_else(|_| unreachable!());
        let ClaimOutcome::Granted(claim) = outcome else {
            unreachable!()
        };
        let original_fence = claim.fence;
        let (registry, _, _) = decide(
            &registry,
            ClaimCommand::MirrorObserved {
                identity: claim.identity.clone(),
                release: false,
                status: MirrorStatus::Unknown,
            },
            &context,
        )
        .unwrap_or_else(|_| unreachable!());
        let renew_ctx = ClaimContext {
            now_ms: NOW + 1_000,
            ..context
        };
        let (mut registry, _, _) = decide(
            &registry,
            ClaimCommand::Renew {
                identity: claim.identity.clone(),
                fence: original_fence,
                lease_ms: 120_000,
            },
            &renew_ctx,
        )
        .unwrap_or_else(|_| unreachable!());
        // Simulate a fence increment on renewal while initial_fence retains original_fence.
        let renewed_fence = original_fence + 1;
        registry
            .claims
            .get_mut(&claim.identity)
            .unwrap_or_else(|| unreachable!())
            .fence = renewed_fence;

        let articles = [
            Article {
                number: 1,
                message_id: "<wrong-fence@x>".to_owned(),
                from: "kiln <kiln@swarm.local>".to_owned(),
                subject: "CLAIM onixresearch/kiln docs".to_owned(),
                date: "Wed, 18 Nov 2026 23:01:00 +0000".to_owned(),
                references: String::new(),
                body: format!(
                    "Claiming onixresearch/kiln docs through the Kiln fleet.\npurpose: docs\nholder: omp on aspen3\nfence: {renewed_fence}\n"
                ),
            },
            Article {
                number: 2,
                message_id: "<orig-claim@x>".to_owned(),
                from: "kiln <kiln@swarm.local>".to_owned(),
                subject: "CLAIM onixresearch/kiln docs".to_owned(),
                date: "Wed, 18 Nov 2026 23:01:00 +0000".to_owned(),
                references: String::new(),
                body: format!(
                    "Claiming onixresearch/kiln docs through the Kiln fleet.\npurpose: docs\nholder: omp on aspen3\nfence: {original_fence}\n"
                ),
            },
        ];
        let learned = learned_message_ids(&registry, &articles, "kiln@swarm.local");
        assert_eq!(
            learned,
            vec![(claim.identity.clone(), false, "<orig-claim@x>".to_owned())]
        );
        let (registry, _, _) = decide(
            &registry,
            ClaimCommand::MirrorObserved {
                identity: claim.identity.clone(),
                release: false,
                status: MirrorStatus::Posted {
                    message_id: Some("<orig-claim@x>".to_owned()),
                },
            },
            &renew_ctx,
        )
        .unwrap_or_else(|_| unreachable!());
        let (_, _, effects) = decide(
            &registry,
            ClaimCommand::Release {
                identity: claim.identity,
                fence: renewed_fence,
                note: "done".to_owned(),
            },
            &ClaimContext {
                now_ms: NOW + 2_000,
                ..context
            },
        )
        .unwrap_or_else(|_| unreachable!());
        assert!(matches!(
            effects.as_slice(),
            [ClaimEffect::PostRelease { references: Some(reference), body, .. }]
                if reference == "<orig-claim@x>" && body.contains(&format!("fence: {renewed_fence}\n"))
        ));
    }
}
