//! Published review delivery: deterministic routing plus a durable, bounded outbox.
//! The HTTPS receiver implements `kiln.review-notification.v1`; `email_adapter`
//! means that receiver, not Kiln, owns email submission. No draft or comment text
//! is sent. A successful HTTP response is receiver acceptance, not email receipt.
use std::collections::{BTreeMap, BTreeSet};

use kiln_adapter_webhook::{AuthMaterial, TransportProgress, TransportResult, WebhookRuntime};
use kiln_core::change_review as domain;
use kiln_core::domain::Outcome;
use kiln_core::types::{EffectId, RunId};
use kiln_provider_api::NotificationRequest;
use serde::{Deserialize, Serialize};

use crate::automation::{AutomationProfile, HttpsRuntime, NotificationProfile, external};

const MAX_RECORDS: usize = 100_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema: String,
    pub destinations: Vec<NotificationProfile>,
    pub recipients: Vec<Recipient>,
    pub max_records: usize,
    pub max_attempts: usize,
    pub retry_delay_ms: i64,
    pub max_tick_effects: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipient {
    pub principal: String,
    pub destination: String,
    pub address: String,
    pub channel: Channel,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Webhook,
    EmailAdapter,
}

impl Profile {
    pub fn admit(&self, principals: &[String]) -> Result<(), String> {
        if self.schema != "kiln.review-delivery-profile.v1"
            || self.max_records == 0
            || self.max_records > MAX_RECORDS
            || self.max_attempts == 0
            || self.max_attempts > 4
            || !(1000..=3_600_000).contains(&self.retry_delay_ms)
            || self.max_tick_effects == 0
            || self.max_tick_effects > 16
            || self.recipients.len() > super::MAX_PRINCIPALS
            || self.destinations.len() > super::MAX_PRINCIPALS
        {
            return Err("review delivery bounds invalid".into());
        }
        // Reuse the existing exact-authority, mapped-IP, path, CA and secret
        // admission rather than create a second, less restrictive HTTP policy.
        AutomationProfile {
            schema: crate::automation::AUTOMATION_SCHEMA.into(),
            triggers: Vec::new(),
            schedules: Vec::new(),
            notifications: self.destinations.clone(),
            max_records: self.max_records,
            status_limit: 1,
            max_event_bytes: 1,
            max_tick_effects: self.max_tick_effects,
        }
        .admit()?;
        let mut seen = BTreeSet::new();
        for recipient in &self.recipients {
            if !principals.contains(&recipient.principal)
                || !seen.insert(&recipient.principal)
                || recipient.address.is_empty()
                || recipient.address.len() > 256
                || recipient.address.chars().any(char::is_control)
                || !self
                    .destinations
                    .iter()
                    .any(|d| d.id == recipient.destination)
            {
                return Err("review delivery recipient rejected".into());
            }
        }
        if self
            .destinations
            .iter()
            .any(|d| !d.enabled || d.on_success || d.on_failure)
        {
            return Err("review delivery destination must enable only review transport".into());
        }
        Ok(())
    }

    fn route(&self, principal: &str) -> Option<(&Recipient, &NotificationProfile)> {
        let recipient = self.recipients.iter().find(|r| r.principal == principal)?;
        Some((
            recipient,
            self.destinations
                .iter()
                .find(|d| d.id == recipient.destination)?,
        ))
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct State {
    pub subscriptions: BTreeMap<String, BTreeSet<String>>,
    pub inbox: BTreeMap<String, Vec<domain::Notification>>,
    pub outbox: BTreeMap<String, Record>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Queued,
    Reserved,
    Delivered,
    Retryable,
    Rejected,
    Uncertain,
    DestinationDenied,
    Exhausted,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attempt {
    pub id: String,
    pub reserved_ms: i64,
    pub completed_ms: Option<i64>,
    pub status: Status,
    pub http_status: Option<u16>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub id: String,
    pub principal: String,
    pub notification: domain::Notification,
    pub destination_identity: String,
    pub body: Vec<u8>,
    pub status: Status,
    pub next_attempt_ms: i64,
    pub attempts: Vec<Attempt>,
}

fn identity<T: Serialize>(value: &T) -> Result<String, String> {
    let bytes = serde_json::to_vec(value).map_err(|_| "review delivery encoding failed")?;
    kiln_codec::identity_from_bytes(&bytes)
        .map(|id| id.as_str().to_owned())
        .map_err(|_| "review delivery identity failed".into())
}

pub fn subscribed(state: &State, actor: &str, change: &str) -> bool {
    state
        .subscriptions
        .get(actor)
        .is_some_and(|rows| rows.contains(change))
}

/// The authenticated actor is the only preference owner; no recipient argument.
pub fn subscribe(
    state: &mut State,
    review: &domain::ReviewState,
    actor: &str,
    change: &str,
    enabled: bool,
    principals: &[String],
) -> Result<(), String> {
    if !principals.iter().any(|p| p == actor) || !review.changes.contains_key(change) {
        return Err("review subscription principal or change denied".into());
    }
    if enabled {
        state
            .subscriptions
            .entry(actor.into())
            .or_default()
            .insert(change.into());
    } else if let Some(rows) = state.subscriptions.get_mut(actor) {
        rows.remove(change);
        if rows.is_empty() {
            state.subscriptions.remove(actor);
        }
    }
    Ok(())
}

pub fn inbox<'a>(state: &'a State, actor: &str) -> &'a [domain::Notification] {
    state
        .inbox
        .get(actor)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

/// Bounded recipient-owned projection; never expose transport bodies, addresses,
/// configured routes, destination fingerprints or another principal's outcomes.
pub fn outcomes(state: &State, actor: &str, change: &str) -> Vec<serde_json::Value> {
    state
        .outbox
        .values()
        .filter(|row| row.principal == actor && row.notification.change == change)
        .take(super::NOTIFICATION_PAGE_SIZE)
        .map(|row| {
            serde_json::json!({ "id":row.id, "status":row.status,
            "notification":row.notification, "attempts":row.attempts })
        })
        .collect()
}

pub fn set_read(state: &mut State, actor: &str, id: &str, read: bool) -> bool {
    if let Some(row) = state
        .inbox
        .get_mut(actor)
        .and_then(|rows| rows.iter_mut().find(|n| n.id == id))
    {
        row.read = read;
        true
    } else {
        false
    }
}

/// Exact admitted principal mentions, delimited from adjacent name characters.
/// `@{principal}` additionally supports admitted identities containing `@` or
/// spaces; ordinary `@principal` supports the same exact identity at boundaries.
fn mentions(body: &str, principal: &str) -> bool {
    let needle = format!("@{principal}");
    body.match_indices(&needle).any(|(offset, _)| {
        let before = body
            .get(..offset)
            .and_then(|prefix| prefix.chars().next_back());
        let after = body
            .get(offset + needle.len()..)
            .and_then(|suffix| suffix.chars().next());
        let name = |c: char| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '@' | ':' | '/');
        before.is_none_or(|c| !name(c)) && after.is_none_or(|c| !name(c))
    }) || body.contains(&format!("@{{{principal}}}"))
}

fn public_changed(before: &domain::Change, after: &domain::Change) -> bool {
    // Deliberately omit actor-owned file-read preferences.
    before.title != after.title
        || before.description != after.description
        || before.state != after.state
        || before.wip != after.wip
        || before.revisions != after.revisions
        || before.reviewers != after.reviewers
        || before.votes != after.votes
        || before.threads != after.threads
        || before.attention != after.attention
        || before.dependencies != after.dependencies
        || before.candidate != after.candidate
        || before.submission != after.submission
        || before.maintenance != after.maintenance
}

/// Pure planning over published changes and admitted principal inboxes only.
/// Call before the parent atomically commits the transition and this state.
pub fn after_transition(
    before: &domain::ReviewState,
    after: &domain::ReviewState,
    state: &mut State,
    profile: Option<&Profile>,
    principals: &[String],
    actor: &str,
    facts: &domain::Facts,
) -> Result<(), String> {
    let event_id = facts.event_id.as_str();
    let now_ms = facts.now_ms;
    if let Some(profile) = profile {
        profile.admit(principals)?;
    }
    let mut routes: BTreeMap<(String, String), (domain::Notification, bool)> = BTreeMap::new();
    for principal in principals {
        let old = domain::project(before, principal);
        for row in domain::project(after, principal).inbox {
            if !old.inbox.iter().any(|n| n.id == row.id) {
                routes.insert(
                    (principal.clone(), row.change.clone()),
                    (row.clone(), false),
                );
            }
        }
    }
    for change in after.changes.values() {
        let previous = before.changes.get(&change.id);
        if previous.is_some_and(|old| !public_changed(old, change)) {
            continue;
        }
        let revision = change
            .current()
            .map_err(|_| "review delivery revision missing")?;
        for principal in principals.iter().filter(|p| p.as_str() != actor) {
            let mentioned = change.threads.iter().any(|thread| {
                thread.comments.iter().any(|comment| {
                    let is_new = previous.is_none_or(|old| {
                        !old.threads.iter().any(|t| {
                            t.id == thread.id && t.comments.iter().any(|c| c.id == comment.id)
                        })
                    });
                    is_new && mentions(&comment.body, principal)
                })
            });
            if !mentioned && !subscribed(state, principal, &change.id) {
                continue;
            }
            routes
                .entry((principal.clone(), change.id.clone()))
                .or_insert_with(|| {
                    (
                        domain::Notification {
                            id: event_id.into(),
                            change: change.id.clone(),
                            revision: revision.id.clone(),
                            actor: actor.into(),
                            kind: if mentioned {
                                "mentioned"
                            } else {
                                "subscribed_update"
                            }
                            .into(),
                            created_ms: now_ms,
                            read: false,
                        },
                        true,
                    )
                });
        }
    }
    let mut inbox_rows = Vec::new();
    let mut outbox_rows = Vec::new();
    for ((principal, _), (notification, supplemental)) in routes {
        if supplemental
            && !inbox(state, &principal)
                .iter()
                .any(|n| n.id == notification.id && n.change == notification.change)
        {
            inbox_rows.push((principal.clone(), notification.clone()));
        }
        let Some((recipient, destination)) = profile.and_then(|p| p.route(&principal)) else {
            continue;
        };
        let id = identity(&(
            "kiln.review-delivery.v1",
            &principal,
            &notification.id,
            &notification.change,
        ))?;
        if state.outbox.contains_key(&id) {
            continue;
        }
        let body = serde_json::to_vec(&serde_json::json!({
            "schema":"kiln.review-notification.v1", "id":id,
            "principal":principal, "channel":recipient.channel, "address":recipient.address,
            "notification":notification,
        }))
        .map_err(|_| "review notification encoding failed")?;
        if body.len() > destination.max_request_bytes {
            return Err("review delivery request bound exceeded".into());
        }
        outbox_rows.push(Record {
            id,
            principal,
            notification,
            destination_identity: identity(&(recipient, destination))?,
            body,
            status: Status::Queued,
            next_attempt_ms: now_ms,
            attempts: Vec::new(),
        });
    }
    let limit = profile.map_or(MAX_RECORDS, |p| p.max_records);
    if state.outbox.len().saturating_add(outbox_rows.len()) > limit
        || state
            .inbox
            .values()
            .map(Vec::len)
            .sum::<usize>()
            .saturating_add(inbox_rows.len())
            > MAX_RECORDS
    {
        return Err("review delivery retained record bound exceeded".into());
    }
    for (principal, notification) in inbox_rows {
        state.inbox.entry(principal).or_default().push(notification);
    }
    for row in outbox_rows {
        state.outbox.insert(row.id.clone(), row);
    }
    Ok(())
}

/// Opaque reservation: persist State before consuming any Work. A dropped
/// reservation is uncertain on the next tick/restart and is never resent.
#[derive(Debug)]
#[must_use]
pub struct Work {
    id: String,
    attempt: String,
    destination_identity: String,
    registration: NotificationProfile,
    request: NotificationRequest,
}

#[derive(Debug)]
pub struct Completion {
    id: String,
    attempt: String,
    destination_identity: String,
    status: Status,
    http_status: Option<u16>,
    completed_ms: i64,
}

pub fn reserve(profile: &Profile, state: &mut State, now_ms: i64) -> Result<Vec<Work>, String> {
    // Validate exact destinations even when invoked independently of Profile::admit.
    let principals: Vec<_> = profile
        .recipients
        .iter()
        .map(|r| r.principal.clone())
        .collect();
    profile.admit(&principals)?;
    let mut work = Vec::new();
    for row in state.outbox.values_mut() {
        if row.status == Status::Reserved {
            row.status = Status::Uncertain;
            if let Some(attempt) = row.attempts.last_mut() {
                attempt.status = Status::Uncertain;
                attempt.completed_ms = Some(now_ms);
            }
        }
        if !matches!(row.status, Status::Queued | Status::Retryable) || row.next_attempt_ms > now_ms
        {
            continue;
        }
        if work.len() >= profile.max_tick_effects {
            continue;
        }
        let Some((recipient, destination)) = profile.route(&row.principal) else {
            row.status = Status::DestinationDenied;
            continue;
        };
        if identity(&(recipient, destination))? != row.destination_identity
            || row.body.len() > destination.max_request_bytes
        {
            row.status = Status::DestinationDenied;
            continue;
        }
        if row.attempts.len() >= profile.max_attempts {
            row.status = Status::Exhausted;
            continue;
        }
        let attempt = identity(&(&row.id, row.attempts.len() + 1))?;
        // NotificationRequest is only the existing transport's byte carrier;
        // its CI-only fields are not serialized or interpreted by HttpsRuntime.
        let request = NotificationRequest {
            effect_id: EffectId::new(attempt.clone())
                .map_err(|_| "review delivery effect identity invalid")?,
            run_id: RunId::new(row.id.clone())
                .map_err(|_| "review delivery carrier identity invalid")?,
            outcome: Outcome::Success,
            notification_policy: external("review-policy", &row.destination_identity)?,
            endpoint_profile: destination.webhook()?.profile_id,
            body_identity: kiln_codec::identity_from_bytes(&row.body)
                .map_err(|_| "review delivery body identity invalid")?,
            idempotency_key: external("review-delivery", &row.id)?,
            body: row.body.clone(),
        };
        row.status = Status::Reserved;
        row.next_attempt_ms = now_ms.saturating_add(profile.retry_delay_ms);
        row.attempts.push(Attempt {
            id: attempt.clone(),
            reserved_ms: now_ms,
            completed_ms: None,
            status: Status::Reserved,
            http_status: None,
        });
        work.push(Work {
            id: row.id.clone(),
            attempt,
            destination_identity: row.destination_identity.clone(),
            registration: destination.clone(),
            request,
        });
    }
    Ok(work)
}

fn transport_status(result: &TransportResult) -> Status {
    match result {
        TransportResult::Response {
            status,
            tls_verified: true,
            ..
        } if (200..=299).contains(status) => Status::Delivered,
        TransportResult::Response { .. } | TransportResult::Redirect { .. } => Status::Rejected,
        TransportResult::TimedOut {
            progress: TransportProgress::BeforeConnect,
        }
        | TransportResult::Disconnected {
            progress: TransportProgress::BeforeConnect,
        }
        | TransportResult::ProviderError {
            progress: TransportProgress::BeforeConnect,
        } => Status::Retryable,
        _ => Status::Uncertain,
    }
}

impl Work {
    pub fn deliver(self) -> Result<Completion, String> {
        let profile = self.registration.webhook()?;
        let mut runtime = HttpsRuntime {
            registration: self.registration,
            addresses: Vec::new(),
            http_status: None,
        };
        let preflight = runtime
            .resolve(&profile.host, profile.port, profile.max_resolver_candidates)
            .map_err(|_| "review notification address preflight failed".to_string())
            .and_then(|addresses| {
                kiln_adapter_webhook::admit_addresses(&profile, &addresses)
                    .map_err(|_| "review notification network denied".to_string())
            });
        let auth = if preflight.is_err() {
            None
        } else {
            match &profile.auth {
                kiln_adapter_webhook::AuthPolicy::None => Some(AuthMaterial::None),
                kiln_adapter_webhook::AuthPolicy::Bearer { secret_reference } => runtime
                    .load_secret(secret_reference)
                    .ok()
                    .map(AuthMaterial::Bearer),
                _ => None,
            }
        };
        let status = if let Some(mut auth) = auth {
            let result = runtime.send(&profile, &self.request, &auth);
            if let AuthMaterial::Bearer(bytes) = &mut auth {
                bytes.fill(0);
            }
            transport_status(&result)
        } else {
            Status::Retryable
        };
        let completed_ms = i64::try_from(crate::automation::now_ms()?)
            .map_err(|_| "review delivery clock overflow")?;
        Ok(Completion {
            id: self.id,
            attempt: self.attempt,
            destination_identity: self.destination_identity,
            status,
            http_status: runtime.http_status,
            completed_ms,
        })
    }
}

pub fn complete(state: &mut State, completion: Completion) -> Result<(), String> {
    let row = state
        .outbox
        .get_mut(&completion.id)
        .ok_or("review delivery reservation missing")?;
    let attempt = row
        .attempts
        .last_mut()
        .ok_or("review delivery attempt missing")?;
    if row.status != Status::Reserved
        || attempt.status != Status::Reserved
        || attempt.id != completion.attempt
        || row.destination_identity != completion.destination_identity
    {
        return Err("review delivery completion does not match exact reservation".into());
    }
    row.status = completion.status.clone();
    attempt.status = completion.status;
    attempt.completed_ms = Some(completion.completed_ms);
    attempt.http_status = completion.http_status;
    Ok(())
}

#[cfg(test)]
#[path = "delivery_tests.rs"]
mod tests;
