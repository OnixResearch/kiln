use super::api::{error, json_response};
use super::{ApiResponse, Engine, Role, identity};
use crate::automation::{
    AutomationProfile, DeliveryRecord, HttpsRuntime, NotificationProfile, RegistrationState,
    SourceRecord, external, now_ms, secret,
};
use kiln_adapter_github::events::{DeliveryHeaders, EventSelector, admit_event};
use kiln_adapter_webhook::{WebhookAdapter, project_notification};
use kiln_core::domain::{NotificationIntent, Outcome};
use kiln_core::types::{EffectId, RunId};
use kiln_provider_api::{NotificationOutcome, NotificationPort, NotificationRequest};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Mutation {
    request_id: String,
    kind: String,
    id: String,
    enabled: bool,
    push: Option<bool>,
    pull_request: Option<bool>,
    interval_ms: Option<u64>,
    next_due_ms: Option<u64>,
    on_success: Option<bool>,
    on_failure: Option<bool>,
}
fn settings(enabled: bool, ordinal: u64) -> RegistrationState {
    RegistrationState {
        enabled,
        push: false,
        pull_request: false,
        interval_ms: None,
        next_due_ms: None,
        on_success: false,
        on_failure: false,
        notification_after_ordinal: ordinal,
        last_status: None,
    }
}
fn key(kind: &str, id: &str) -> String {
    format!("{kind}:{id}")
}
fn digest<T: Serialize>(value: &T) -> Result<String, String> {
    Ok(identity(value)?.as_str().to_string())
}
fn head_key(project: &crate::profile::ProjectProfile, reference: &str) -> String {
    format!("{}:{reference}", project.repository_key())
}

// r[impl kiln.monorepo.dispatch]
/// Pure selection of the other registrations one authenticated repository
/// event applies to: enabled for this event kind, same provider repository,
/// matching reference, a project on the same canonical repository, and at
/// most one registration (the first in policy order) per project.
fn sibling_triggers(
    profile: &AutomationProfile,
    settings: &std::collections::BTreeMap<String, RegistrationState>,
    projects: &[crate::profile::ProjectProfile],
    registration: &crate::automation::TriggerProfile,
    selector: &EventSelector,
) -> Result<Vec<crate::automation::TriggerProfile>, String> {
    let project_of = |id: &str| {
        projects
            .iter()
            .find(|p| p.id == id)
            .ok_or("automation project missing")
    };
    let repository = project_of(&registration.project_id)?.repository_key();
    let mut seen = std::collections::BTreeSet::from([registration.project_id.as_str()]);
    let mut siblings = Vec::new();
    for trigger in &profile.triggers {
        let Some(state) = settings.get(&key("trigger", &trigger.id)) else {
            continue;
        };
        let matches = match selector {
            EventSelector::Push { reference } => state.push && &trigger.reference == reference,
            EventSelector::PullRequest {
                head_reference,
                base_reference,
                ..
            } => {
                state.pull_request
                    && trigger.reference.strip_prefix("refs/heads/")
                        == Some(head_reference.as_str())
                    && trigger.pull_request_base.as_ref() == Some(base_reference)
            }
        };
        if state.enabled
            && matches
            && trigger.repository == registration.repository
            && project_of(&trigger.project_id)?.repository_key() == repository
            && seen.insert(trigger.project_id.as_str())
        {
            siblings.push(trigger.clone());
        }
    }
    Ok(siblings)
}

/// A pull request's exact base: the merge base of its head and the admitted
/// base branch. Unknown bases are `None` and fall back to every project.
fn pull_request_base(
    repository: &git2::Repository,
    head: &str,
    base_reference: &str,
) -> Option<String> {
    let head = git2::Oid::from_str(head).ok()?;
    let base = repository
        .find_reference(&format!("refs/heads/{base_reference}"))
        .and_then(|reference| reference.peel_to_commit())
        .ok()?;
    repository
        .merge_base(head, base.id())
        .ok()
        .map(|oid| oid.to_string())
}

/// A durable reservation, consumed exactly once outside the engine lock.
/// Dropping it leaves the reservation ambiguous on restart, never retryable.
#[derive(Debug)]
#[must_use]
pub struct NotificationWork {
    reservation: DeliveryRecord,
    policy_identity: String,
    registration: NotificationProfile,
    request: NotificationRequest,
}

/// Opaque, exact-reservation observation produced only by consuming work.
#[derive(Debug)]
pub struct NotificationCompletion {
    reservation: DeliveryRecord,
    policy_identity: String,
    completed_at_ms: u64,
    status: &'static str,
    http_status: Option<u16>,
    error_code: Option<&'static str>,
}

impl NotificationWork {
    pub fn deliver(self) -> Result<NotificationCompletion, String> {
        let webhook = self.registration.webhook()?;
        let mut adapter = WebhookAdapter::new(
            webhook,
            HttpsRuntime {
                registration: self.registration,
                addresses: Vec::new(),
                http_status: None,
            },
        );
        let result = adapter.deliver(&self.request);
        let (status, error_code) = match result {
            Ok(observation) => {
                if observation.effect_id != self.request.effect_id
                    || observation.endpoint_profile != self.request.endpoint_profile
                    || observation.body_identity != self.request.body_identity
                {
                    return Err("notification observation identity differs from reservation".into());
                }
                match observation.outcome {
                    NotificationOutcome::Accepted => ("delivered", None),
                    NotificationOutcome::Unknown => {
                        ("ambiguous", Some("notification_acceptance_unknown"))
                    }
                    NotificationOutcome::Rejected => {
                        ("rejected", Some("notification_receiver_rejected"))
                    }
                    NotificationOutcome::RateLimited => {
                        ("rejected", Some("notification_rate_limited"))
                    }
                    NotificationOutcome::TimedOut => {
                        ("rejected", Some("notification_timeout_before_send"))
                    }
                    NotificationOutcome::ProviderError => {
                        ("rejected", Some("notification_provider_error"))
                    }
                }
            }
            Err(_) => ("rejected", Some("notification_preflight_rejected")),
        };
        Ok(NotificationCompletion {
            reservation: self.reservation,
            policy_identity: self.policy_identity,
            completed_at_ms: now_ms()?,
            status,
            http_status: adapter.runtime().http_status,
            error_code,
        })
    }
}

enum SelectedEffect {
    Source(String),
    Schedule {
        index: usize,
        due: u64,
        interval: u64,
    },
    Notification {
        index: usize,
        run_id: String,
        outcome: Outcome,
        delivery_id: String,
    },
}

/// Pure selection: at most one ready effect from the next eligible registration.
/// The cursor advances only when its effect is durably reserved by the shell.
fn next_effect(
    profile: &AutomationProfile,
    state: &super::model::DurableState,
    now: u64,
) -> Result<Option<(usize, SelectedEffect)>, String> {
    enum Registration<'a> {
        Trigger(&'a str),
        Schedule(usize, &'a crate::automation::ScheduleProfile),
        Notification(usize, &'a NotificationProfile),
    }
    let count = profile
        .triggers
        .len()
        .checked_add(profile.schedules.len())
        .and_then(|count| count.checked_add(profile.notifications.len()))
        .ok_or("automation registration count overflow")?;
    let cursor = state.automation.next_registration;
    if cursor >= count.max(1) {
        return Err("automation cursor exceeds registered authority".into());
    }
    let registrations = profile
        .triggers
        .iter()
        .map(|p| Registration::Trigger(p.id.as_str()))
        .chain(
            profile
                .schedules
                .iter()
                .enumerate()
                .map(|(index, p)| Registration::Schedule(index, p)),
        )
        .chain(
            profile
                .notifications
                .iter()
                .enumerate()
                .map(|(index, p)| Registration::Notification(index, p)),
        )
        .enumerate();
    for (index, registration) in registrations
        .clone()
        .skip(cursor)
        .chain(registrations.take(cursor))
    {
        let next = if index + 1 == count { 0 } else { index + 1 };
        let source_registration = match &registration {
            Registration::Trigger(id) => Some(("trigger", *id)),
            Registration::Schedule(_, p) => Some(("schedule", p.id.as_str())),
            Registration::Notification(_, _) => None,
        };
        if let Some((kind, id)) = source_registration {
            if let Some(source) = state
                .automation
                .sources
                .values()
                .filter(|s| s.status == "reserved" && s.kind == kind && s.registration_id == id)
                .min_by_key(|s| (s.received_at_ms, &s.id))
            {
                return Ok(Some((next, SelectedEffect::Source(source.id.clone()))));
            }
        }
        if matches!(&registration, Registration::Trigger(_)) {
            continue;
        }
        if let Registration::Schedule(schedule_index, registration) = &registration {
            if state.automation.sources.len() >= profile.max_records {
                continue;
            }
            let setting = state
                .automation
                .settings
                .get(&key("schedule", &registration.id))
                .ok_or("schedule state missing")?;
            if let Some(due) = setting
                .next_due_ms
                .filter(|due| setting.enabled && *due <= now)
            {
                return Ok(Some((
                    next,
                    SelectedEffect::Schedule {
                        index: *schedule_index,
                        due,
                        interval: setting.interval_ms.ok_or("schedule interval missing")?,
                    },
                )));
            }
            continue;
        }
        if state.automation.deliveries.len() >= profile.max_records {
            continue;
        }
        let Registration::Notification(notification_index, registration) = registration else {
            return Err("automation selection registration invalid".into());
        };
        let setting = state
            .automation
            .settings
            .get(&key("notification", &registration.id))
            .ok_or("notification state missing")?;
        if !setting.enabled {
            continue;
        }
        let mut candidate = None;
        for run in state.runs.values().filter(|r| {
            r.project_id == registration.project_id
                && r.ordinal > setting.notification_after_ordinal
                && r.outcome().is_terminal()
                && ((r.outcome() == Outcome::Success && setting.on_success)
                    || (r.outcome() != Outcome::Success && setting.on_failure))
        }) {
            let delivery_id = digest(&("notification", &registration.id, &run.id))?;
            if !state.automation.deliveries.contains_key(&delivery_id)
                && candidate.as_ref().is_none_or(
                    |(previous, _): &(&super::model::RunRecord, String)| {
                        (run.ordinal, &run.id) < (previous.ordinal, &previous.id)
                    },
                )
            {
                candidate = Some((run, delivery_id));
            }
        }
        if let Some((run, delivery_id)) = candidate {
            return Ok(Some((
                next,
                SelectedEffect::Notification {
                    index: notification_index,
                    run_id: run.id.clone(),
                    outcome: run.outcome(),
                    delivery_id,
                },
            )));
        }
    }
    Ok(None)
}
impl Engine {
    pub fn configure_automation(&mut self, profile: AutomationProfile) -> Result<(), String> {
        profile.admit()?;
        for (project_id, reference) in profile
            .triggers
            .iter()
            .map(|p| (&p.project_id, Some(&p.reference)))
            .chain(
                profile
                    .schedules
                    .iter()
                    .map(|p| (&p.project_id, Some(&p.reference))),
            )
            .chain(profile.notifications.iter().map(|p| (&p.project_id, None)))
        {
            let project = self
                .profile
                .projects
                .iter()
                .find(|p| &p.id == project_id)
                .ok_or("automation project is not registered")?;
            if reference.is_some_and(|reference| {
                reference != &project.reference && !project.admitted_references.contains(reference)
            }) {
                return Err("automation reference is not admitted by service policy".into());
            }
        }
        let fingerprint = digest(&profile)?;
        if self
            .state
            .automation
            .policy_identity
            .as_ref()
            .is_some_and(|id| id != &fingerprint)
        {
            return Err("automation policy differs from retained authority; explicit state migration required".into());
        }
        if self.state.automation.policy_identity.is_none() {
            let ordinal = self
                .state
                .runs
                .values()
                .map(|r| r.ordinal)
                .max()
                .unwrap_or(0);
            for p in &profile.triggers {
                let mut value = settings(p.enabled, ordinal);
                value.push = p.push;
                value.pull_request = p.pull_request;
                self.state
                    .automation
                    .settings
                    .insert(key("trigger", &p.id), value);
            }
            for p in &profile.schedules {
                let mut value = settings(p.enabled, ordinal);
                value.interval_ms = Some(p.interval_ms);
                value.next_due_ms = p.next_due_ms;
                self.state
                    .automation
                    .settings
                    .insert(key("schedule", &p.id), value);
            }
            for p in &profile.notifications {
                let mut value = settings(p.enabled, ordinal);
                value.on_success = p.on_success;
                value.on_failure = p.on_failure;
                self.state
                    .automation
                    .settings
                    .insert(key("notification", &p.id), value);
            }
            self.state.automation.policy_identity = Some(fingerprint);
        }
        if self.state.automation.settings.len()
            != profile.triggers.len() + profile.schedules.len() + profile.notifications.len()
            || self.state.automation.sources.len() > profile.max_records
            || self.state.automation.deliveries.len() > profile.max_records
            || self.state.automation.requests.len() > profile.max_records
        {
            return Err("retained automation registration or ledger bounds invalid".into());
        }
        let registration_count =
            profile.triggers.len() + profile.schedules.len() + profile.notifications.len();
        if self.state.automation.next_registration >= registration_count.max(1) {
            return Err("retained automation cursor exceeds registered authority".into());
        }
        for p in &profile.triggers {
            let s = self
                .state
                .automation
                .settings
                .get(&key("trigger", &p.id))
                .ok_or("retained trigger missing")?;
            if (s.push && !p.push)
                || (s.pull_request && !p.pull_request)
                || s.interval_ms.is_some()
                || s.next_due_ms.is_some()
                || s.on_success
                || s.on_failure
            {
                return Err("retained trigger exceeds registered authority".into());
            }
        }
        for p in &profile.schedules {
            let s = self
                .state
                .automation
                .settings
                .get(&key("schedule", &p.id))
                .ok_or("retained schedule missing")?;
            if s.interval_ms
                .is_none_or(|v| !(p.min_interval_ms..=p.max_interval_ms).contains(&v))
                || (s.enabled && s.next_due_ms.is_none())
                || s.push
                || s.pull_request
                || s.on_success
                || s.on_failure
            {
                return Err("retained schedule exceeds registered authority".into());
            }
        }
        for p in &profile.notifications {
            let s = self
                .state
                .automation
                .settings
                .get(&key("notification", &p.id))
                .ok_or("retained notification missing")?;
            if (s.on_success && !p.on_success)
                || (s.on_failure && !p.on_failure)
                || s.push
                || s.pull_request
                || s.interval_ms.is_some()
                || s.next_due_ms.is_some()
            {
                return Err("retained notification exceeds registered authority".into());
            }
        }
        for source in self.state.automation.sources.values() {
            let admitted = match source.kind.as_str() {
                "trigger" => profile.triggers.iter().any(|p| {
                    p.id == source.registration_id
                        && p.project_id == source.project_id
                        && p.reference == source.reference
                }),
                "schedule" => profile.schedules.iter().any(|p| {
                    p.id == source.registration_id
                        && p.project_id == source.project_id
                        && p.reference == source.reference
                }),
                _ => false,
            };
            if !admitted
                || source.run_id.as_ref().is_some_and(|id| {
                    self.state
                        .runs
                        .get(id)
                        .is_none_or(|r| r.project_id != source.project_id)
                })
            {
                return Err("retained source reservation is not registration-bound".into());
            }
        }
        for delivery in self.state.automation.deliveries.values() {
            if !profile.notifications.iter().any(|p| {
                p.id == delivery.registration_id
                    && self
                        .state
                        .runs
                        .get(&delivery.run_id)
                        .is_some_and(|r| r.project_id == p.project_id)
            }) {
                return Err("retained notification is not registration-bound".into());
            }
        }
        // A crash after reservation may have delivered the request. Never resend.
        for delivery in self
            .state
            .automation
            .deliveries
            .values_mut()
            .filter(|d| d.status == "reserved")
        {
            delivery.status = "ambiguous".into();
            delivery.error_code = Some("restart_after_reservation".into());
        }
        self.automation_profile = Some(profile);
        self.persist()
    }
    fn automation_view(&self) -> Result<Value, String> {
        let Some(profile) = &self.automation_profile else {
            return Ok(
                json!({"configured":false,"policy_identity":null,"status":"unconfigured","triggers":[],"schedules":[],"notifications":[],"sources":[],"deliveries":[]}),
            );
        };
        let get = |kind, id: &str| {
            self.state
                .automation
                .settings
                .get(&key(kind, id))
                .ok_or_else(|| format!("{kind} state missing"))
        };
        let triggers: Vec<_> = profile.triggers.iter().map(|p| {
            let s = get("trigger", &p.id)?;
            let scope = self.profile.projects.iter().find(|project| project.id == p.project_id).map(|project| project.scope_view()).transpose()?.unwrap_or(Value::Null);
            Ok(json!({"id":p.id,"project_id":p.project_id,"reference":p.reference,"enabled":s.enabled,"push":s.push,"pull_request":s.pull_request,"allow_push":p.push,"allow_pull_request":p.pull_request,"last_status":s.last_status,"scope":scope}))
        }).collect::<Result<_, String>>()?;
        let schedules: Vec<_> = profile.schedules.iter().map(|p| {
            let s = get("schedule", &p.id)?;
            Ok(json!({"id":p.id,"project_id":p.project_id,"reference":p.reference,"enabled":s.enabled,"interval_ms":s.interval_ms,"min_interval_ms":p.min_interval_ms,"max_interval_ms":p.max_interval_ms,"next_due_ms":s.next_due_ms,"last_status":s.last_status}))
        }).collect::<Result<_, String>>()?;
        let notifications: Vec<_> = profile.notifications.iter().map(|p| {
            let s = get("notification", &p.id)?;
            Ok(json!({"id":p.id,"project_id":p.project_id,"enabled":s.enabled,"on_success":s.on_success,"on_failure":s.on_failure,"allow_success":p.on_success,"allow_failure":p.on_failure,"last_status":s.last_status}))
        }).collect::<Result<_, String>>()?;
        let mut sources: Vec<_> = self.state.automation.sources.values().collect();
        sources.sort_by_key(|r| (std::cmp::Reverse(r.received_at_ms), &r.id));
        let sources: Vec<_> = sources.into_iter().take(profile.status_limit).map(|r| json!({"id":r.id,"kind":r.kind,"registration_id":r.registration_id,"project_id":r.project_id,"received_at_ms":r.received_at_ms,"status":r.status,"run_id":r.run_id,"error_code":r.error_code,"affected":r.affected,"changed":r.changed})).collect();
        let mut deliveries: Vec<_> = self.state.automation.deliveries.values().collect();
        deliveries.sort_by_key(|r| (std::cmp::Reverse(r.reserved_at_ms), &r.id));
        let deliveries: Vec<_> = deliveries.into_iter().take(profile.status_limit).collect();
        let enabled = self.state.automation.settings.values().any(|r| r.enabled);
        Ok(
            json!({"configured":true,"policy_identity":self.state.automation.policy_identity,"status":if enabled {"enabled"} else {"disabled"},"triggers":triggers,"schedules":schedules,"notifications":notifications,"sources":sources,"deliveries":deliveries,
            "retention":{"sources":self.state.automation.sources.len(),"deliveries":self.state.automation.deliveries.len(),"requests":self.state.automation.requests.len(),"max_records":profile.max_records}}),
        )
    }
    pub(super) fn automation_route(
        &mut self,
        method: &str,
        target: &str,
        body: &[u8],
        role: Role,
    ) -> Result<ApiResponse, String> {
        if target != "/api/automation" {
            return error(404, "route_missing");
        }
        if method == "GET" {
            return json_response(200, self.automation_view()?);
        }
        if method != "POST" {
            return error(404, "route_missing");
        }
        if role != Role::Operator {
            return error(403, "automation_forbidden");
        }
        let Some(profile) = &self.automation_profile else {
            return error(409, "automation_unconfigured");
        };
        let request: Mutation = match serde_json::from_slice(body) {
            Ok(value) => value,
            Err(_) => return error(400, "automation_invalid"),
        };
        if !crate::profile::identifier(&request.request_id)
            || !crate::profile::identifier(&request.id)
        {
            return error(400, "automation_identity_invalid");
        }
        let fingerprint = digest(&request)?;
        if let Some(previous) = self.state.automation.requests.get(&request.request_id) {
            return if previous == &fingerprint {
                json_response(200, self.automation_view()?)
            } else {
                error(409, "automation_request_conflict")
            };
        }
        if self.state.automation.requests.len() >= profile.max_records {
            return error(409, "automation_retention");
        }
        let registration_key = key(&request.kind, &request.id);
        let Some(previous) = self.state.automation.settings.get(&registration_key) else {
            return error(404, "automation_registration_missing");
        };
        let mut updated = previous.clone();
        updated.enabled = request.enabled;
        let valid = match request.kind.as_str() {
            "trigger" => {
                let Some(registration) = profile.triggers.iter().find(|p| p.id == request.id)
                else {
                    return error(404, "automation_registration_missing");
                };
                updated.push = request.push.unwrap_or(updated.push);
                updated.pull_request = request.pull_request.unwrap_or(updated.pull_request);
                request.interval_ms.is_none()
                    && request.next_due_ms.is_none()
                    && request.on_success.is_none()
                    && request.on_failure.is_none()
                    && (!updated.push || registration.push)
                    && (!updated.pull_request || registration.pull_request)
            }
            "schedule" => {
                let Some(registration) = profile.schedules.iter().find(|p| p.id == request.id)
                else {
                    return error(404, "automation_registration_missing");
                };
                updated.interval_ms = request.interval_ms.or(updated.interval_ms);
                updated.next_due_ms = request.next_due_ms.or(updated.next_due_ms);
                let now = now_ms()?;
                request.push.is_none()
                    && request.pull_request.is_none()
                    && request.on_success.is_none()
                    && request.on_failure.is_none()
                    && updated.interval_ms.is_some_and(|v| {
                        (registration.min_interval_ms..=registration.max_interval_ms).contains(&v)
                    })
                    && (!updated.enabled || updated.next_due_ms.is_some())
                    && request
                        .next_due_ms
                        .is_none_or(|v| v >= now && v <= now.saturating_add(31_536_000_000))
            }
            "notification" => {
                let Some(registration) = profile.notifications.iter().find(|p| p.id == request.id)
                else {
                    return error(404, "automation_registration_missing");
                };
                updated.on_success = request.on_success.unwrap_or(updated.on_success);
                updated.on_failure = request.on_failure.unwrap_or(updated.on_failure);
                if !previous.enabled && updated.enabled {
                    updated.notification_after_ordinal = self
                        .state
                        .runs
                        .values()
                        .map(|r| r.ordinal)
                        .max()
                        .unwrap_or(0);
                }
                request.push.is_none()
                    && request.pull_request.is_none()
                    && request.interval_ms.is_none()
                    && request.next_due_ms.is_none()
                    && (!updated.on_success || registration.on_success)
                    && (!updated.on_failure || registration.on_failure)
            }
            _ => false,
        };
        if !valid {
            return error(400, "automation_options_not_admitted");
        }
        self.state
            .automation
            .settings
            .insert(registration_key, updated);
        self.state
            .automation
            .requests
            .insert(request.request_id, fingerprint);
        self.persist()?;
        json_response(200, self.automation_view()?)
    }
    pub fn automation_trigger(
        &mut self,
        id: &str,
        headers: &DeliveryHeaders,
        body: &[u8],
    ) -> Result<ApiResponse, String> {
        if self.storage_failed {
            return error(503, "durable_state_unavailable");
        }
        if !crate::profile::identifier(id) {
            return error(400, "trigger_identity_invalid");
        }
        let mut authenticated = false;
        let result = self.admit_automation_trigger(id, headers, body, &mut authenticated);
        let principal = authenticated.then(|| crate::access::Principal {
            id: format!("github:{id}"),
            label: format!("GitHub {id}"),
            role: Role::Operator,
        });
        let status = result.as_ref().map(|r| r.status).unwrap_or(503);
        self.append_audit(
            "POST",
            &format!("/api/triggers/{id}"),
            principal.as_ref(),
            if status < 300 {
                "trigger_admitted"
            } else {
                "trigger_denied"
            },
            status,
        )?;
        result
    }
    fn admit_automation_trigger(
        &mut self,
        id: &str,
        headers: &DeliveryHeaders,
        body: &[u8],
        authenticated: &mut bool,
    ) -> Result<ApiResponse, String> {
        let Some(profile) = &self.automation_profile else {
            return error(409, "automation_unconfigured");
        };
        let Some(registration) = profile.triggers.iter().find(|r| r.id == id).cloned() else {
            return error(404, "trigger_unregistered");
        };
        let state = self
            .state
            .automation
            .settings
            .get(&key("trigger", id))
            .ok_or("trigger state missing")?;
        if !state.enabled {
            return error(409, "trigger_disabled");
        }
        let mut credential = secret(&registration.secret_file)?;
        let result = admit_event(
            &registration.repository,
            headers,
            body,
            &credential,
            profile.max_event_bytes,
        );
        credential.fill(0);
        let event = match result {
            Ok(event) => event,
            Err(_) => return error(403, "trigger_event_rejected"),
        };
        *authenticated = true;
        let delivery = digest(&("github-delivery", &headers.delivery))?;
        // Persist every authenticated delivery ID, including ignored event actions.
        if self.state.automation.sources.contains_key(&delivery) {
            return error(409, "trigger_replayed");
        }
        let body_identity = blake3::hash(body).to_hex().to_string();
        let mut revision = None;
        let mut candidate = Some(body_identity);
        // The receiving registration first, then one sibling per other project.
        let mut group = vec![registration.clone()];
        let mut evaluation = None;
        let mut head = None;
        if let Some(event) = event {
            let selected = match &event.selector {
                EventSelector::Push { reference } => {
                    state.push && reference == &registration.reference
                }
                EventSelector::PullRequest {
                    head_reference,
                    base_reference,
                    ..
                } => {
                    state.pull_request
                        && registration.reference.strip_prefix("refs/heads/")
                            == Some(head_reference.as_str())
                        && registration.pull_request_base.as_ref() == Some(base_reference)
                }
            };
            if !selected {
                return error(403, "trigger_source_unregistered");
            }
            let project = self
                .profile
                .projects
                .iter()
                .find(|p| p.id == registration.project_id)
                .ok_or("automation project missing")?;
            let repository = git2::Repository::open(&project.repository_path)
                .map_err(|_| "automation source unavailable")?;
            let current = repository
                .revparse_single(&registration.reference)
                .and_then(|v| v.peel_to_commit())
                .map_err(|_| "automation reference unavailable")?;
            if current.id().to_string() != event.revision.to_ascii_lowercase() {
                return error(409, "trigger_source_stale");
            }
            let exact = current.id().to_string();
            group.extend(sibling_triggers(
                profile,
                &self.state.automation.settings,
                &self.profile.projects,
                &registration,
                &event.selector,
            )?);
            let projects = group
                .iter()
                .map(|trigger| {
                    self.profile
                        .projects
                        .iter()
                        .find(|p| p.id == trigger.project_id)
                        .ok_or("automation project missing")
                })
                .collect::<Result<Vec<_>, _>>()?;
            let base = match &event.selector {
                EventSelector::Push { reference } => {
                    let key = head_key(project, reference);
                    let base = self.state.automation.heads.get(&key).cloned();
                    head = Some((key, exact.clone()));
                    base
                }
                EventSelector::PullRequest { base_reference, .. } => {
                    pull_request_base(&repository, &exact, base_reference)
                }
            };
            // Repositories without scoped projects keep today's behaviour exactly.
            if projects.iter().any(|p| p.scope.is_some()) {
                evaluation = Some(crate::monorepo::evaluate(
                    &project.repository_path,
                    &projects,
                    base.as_deref(),
                    &exact,
                )?);
            }
            candidate = Some(event.candidate_identity.as_str().to_string());
            revision = Some(event.revision);
        }
        if self
            .state
            .automation
            .sources
            .values()
            .any(|r| r.kind == "trigger" && r.candidate_identity == candidate)
        {
            return error(409, "trigger_candidate_replayed");
        }
        if self.state.automation.sources.len() + group.len() > profile.max_records {
            return error(409, "automation_retention");
        }
        let received_at_ms = now_ms()?;
        let mut reserved = Vec::new();
        for (index, trigger) in group.into_iter().enumerate() {
            let id = if index == 0 {
                delivery.clone()
            } else {
                digest(&("github-delivery", &headers.delivery, &trigger.id))?
            };
            let affected = evaluation
                .as_ref()
                .and_then(|e: &crate::monorepo::Evaluation| e.set.decision(&trigger.project_id))
                .map(kiln_core::monorepo::ProjectDecision::is_affected);
            let status = if revision.is_none() {
                "ignored"
            } else if affected == Some(false) {
                "skipped"
            } else {
                reserved.push(id.clone());
                "reserved"
            };
            if index > 0 || status == "skipped" {
                if let Some(settings) = self
                    .state
                    .automation
                    .settings
                    .get_mut(&key("trigger", &trigger.id))
                {
                    settings.last_status = Some(status.into());
                }
            }
            self.state.automation.sources.insert(
                id.clone(),
                SourceRecord {
                    id,
                    kind: "trigger".into(),
                    registration_id: trigger.id,
                    received_at_ms,
                    status: status.into(),
                    run_id: None,
                    error_code: None,
                    project_id: trigger.project_id,
                    reference: trigger.reference,
                    source_revision: revision.clone(),
                    candidate_identity: candidate.clone(),
                    affected: evaluation.as_ref().map(|e| e.set.clone()),
                    changed: evaluation.as_ref().map(|e| e.changed),
                },
            );
        }
        if let Some((key, exact)) = head {
            self.state.automation.heads.insert(key, exact);
        }
        self.persist()?;
        for id in reserved {
            self.dispatch_automation_source(&id)?;
        }
        json_response(202, self.state.automation.sources.get(&delivery))
    }
    fn dispatch_automation_source(&mut self, id: &str) -> Result<(), String> {
        let reservation = self
            .state
            .automation
            .sources
            .get(id)
            .ok_or("automation source reservation missing")?
            .clone();
        if reservation.status != "reserved" {
            return Ok(());
        }
        let request_id = format!("automation-{}", blake3::hash(id.as_bytes()).to_hex());
        let body = match reservation.source_revision {
            Some(revision) => {
                json!({"project_id":reservation.project_id,"request_id":request_id,"reference":reservation.reference,"source_revision":revision})
            }
            None => {
                json!({"project_id":reservation.project_id,"request_id":request_id,"reference":reservation.reference})
            }
        };
        let principal = crate::access::Principal {
            id: format!(
                "automation:{}:{}",
                reservation.kind, reservation.registration_id
            ),
            label: format!("{} {}", reservation.kind, reservation.registration_id),
            role: Role::Operator,
        };
        // The run records why it ran; manual fallback applies when absent.
        self.automation_affected = match (&reservation.affected, reservation.changed) {
            (Some(set), Some(changed)) => crate::monorepo::Evaluation {
                set: set.clone(),
                changed,
            }
            .run_affected(&reservation.project_id),
            _ => None,
        };
        let response = self.api_as(
            "POST",
            "/api/runs",
            &serde_json::to_vec(&body).map_err(|_| "automation run encoding failed")?,
            &principal,
        );
        self.automation_affected = None;
        let response = response?;
        let value: Value = serde_json::from_slice(&response.body)
            .map_err(|_| "automation run response invalid")?;
        let record = self
            .state
            .automation
            .sources
            .get_mut(id)
            .ok_or("automation source reservation missing")?;
        record.run_id = value.get("id").and_then(Value::as_str).map(str::to_string);
        record.status = if record.run_id.is_some() {
            "queued"
        } else if response.status >= 500 {
            "ambiguous"
        } else {
            "rejected"
        }
        .into();
        record.error_code = value
            .get("code")
            .and_then(Value::as_str)
            .map(str::to_string);
        let registration_key = key(&record.kind, &record.registration_id);
        let status = record.status.clone();
        if let Some(settings) = self.state.automation.settings.get_mut(&registration_key) {
            settings.last_status = Some(status);
        }
        self.persist()
    }
    /// Reserve bounded notification work under the lock without notification I/O.
    pub fn tick_automation(&mut self) -> Result<Vec<NotificationWork>, String> {
        if self.storage_failed {
            return Err("durable state unavailable".into());
        }
        let Some(profile) = self.automation_profile.as_ref() else {
            return Ok(Vec::new());
        };
        let budget = profile.max_tick_effects;
        let now = now_ms()?;
        let mut notifications = Vec::new();
        for _ in 0..budget {
            let profile = self
                .automation_profile
                .as_ref()
                .ok_or("automation policy missing")?;
            let Some((next, effect)) = next_effect(profile, &self.state, now)? else {
                break;
            };
            match effect {
                SelectedEffect::Source(id) => {
                    self.state.automation.next_registration = next;
                    self.persist()?;
                    self.dispatch_automation_source(&id)?;
                }
                SelectedEffect::Schedule {
                    index,
                    due,
                    interval,
                } => {
                    let schedule = profile
                        .schedules
                        .get(index)
                        .ok_or("schedule registration missing")?;
                    let reservation = digest(&("schedule", &schedule.id, due))?;
                    // Skip missed intervals; persist advance and rotation before dispatch.
                    let next_due = due
                        .checked_add(
                            (now.saturating_sub(due) / interval + 1)
                                .checked_mul(interval)
                                .ok_or("schedule clock overflow")?,
                        )
                        .ok_or("schedule clock overflow")?;
                    self.state
                        .automation
                        .settings
                        .get_mut(&key("schedule", &schedule.id))
                        .ok_or("schedule state missing")?
                        .next_due_ms = Some(next_due);
                    self.state
                        .automation
                        .sources
                        .entry(reservation.clone())
                        .or_insert(SourceRecord {
                            id: reservation.clone(),
                            kind: "schedule".into(),
                            registration_id: schedule.id.clone(),
                            received_at_ms: now,
                            status: "reserved".into(),
                            run_id: None,
                            error_code: None,
                            project_id: schedule.project_id.clone(),
                            reference: schedule.reference.clone(),
                            source_revision: None,
                            candidate_identity: None,
                            affected: None,
                            changed: None,
                        });
                    self.state.automation.next_registration = next;
                    self.persist()?;
                    self.dispatch_automation_source(&reservation)?;
                }
                SelectedEffect::Notification {
                    index,
                    run_id,
                    outcome,
                    delivery_id,
                } => {
                    let registration = profile
                        .notifications
                        .get(index)
                        .ok_or("notification registration missing")?
                        .clone();
                    let request = project_notification(
                        &registration.webhook()?,
                        &NotificationIntent {
                            run_id: RunId::new(run_id.clone())
                                .map_err(|_| "notification run identity invalid")?,
                            outcome,
                        },
                        EffectId::new(delivery_id.clone())
                            .map_err(|_| "notification effect identity invalid")?,
                        external("notification-policy", &registration.id)?,
                        external("idempotency", &delivery_id)?,
                    )
                    .map_err(|_| "notification request rejected")?;
                    let reservation = DeliveryRecord {
                        id: delivery_id.clone(),
                        registration_id: registration.id.clone(),
                        run_id,
                        reserved_at_ms: now,
                        completed_at_ms: None,
                        status: "reserved".into(),
                        http_status: None,
                        error_code: None,
                    };
                    let policy_identity = self
                        .state
                        .automation
                        .policy_identity
                        .clone()
                        .ok_or("automation policy identity missing")?;
                    self.state
                        .automation
                        .deliveries
                        .insert(delivery_id, reservation.clone());
                    self.state.automation.next_registration = next;
                    self.persist()?;
                    notifications.push(NotificationWork {
                        reservation,
                        policy_identity,
                        registration,
                        request,
                    });
                }
            }
        }
        Ok(notifications)
    }

    pub fn record_notification(
        &mut self,
        completion: NotificationCompletion,
    ) -> Result<(), String> {
        if self.storage_failed {
            return Err("durable state unavailable".into());
        }
        if self.state.automation.policy_identity.as_ref() != Some(&completion.policy_identity)
            || self
                .state
                .automation
                .deliveries
                .get(&completion.reservation.id)
                != Some(&completion.reservation)
            || completion.reservation.status != "reserved"
        {
            return Err(
                "notification observation does not match exact retained reservation".into(),
            );
        }
        let record = self
            .state
            .automation
            .deliveries
            .get_mut(&completion.reservation.id)
            .ok_or("delivery reservation missing")?;
        record.http_status = completion.http_status;
        record.completed_at_ms = Some(completion.completed_at_ms);
        record.status = completion.status.into();
        record.error_code = completion.error_code.map(str::to_string);
        self.state
            .automation
            .settings
            .get_mut(&key("notification", &record.registration_id))
            .ok_or("notification state missing")?
            .last_status = Some(record.status.clone());
        self.persist()
    }
}
