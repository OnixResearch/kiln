use std::collections::BTreeMap;

use kiln_core::domain::Outcome;
use serde::Deserialize;
use serde_json::json;

use super::{ApiResponse, Engine, Role, api::error, api::json_response};
use crate::access::Principal;
use crate::feeds::{
    AtomItem, FeedSubscription, ParsedFeed, admit_url, render_atom, subscription_id,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddFeed {
    url: String,
    #[serde(default)]
    label: Option<String>,
}

pub(crate) struct AdmittedFeed {
    pub id: String,
    pub url: String,
    pub label: String,
}

// r[impl kiln.web.feeds.subscriptions]
// r[impl kiln.web.feeds.publish]
impl Engine {
    pub(crate) fn feeds_view(&self, role: Role) -> Result<ApiResponse, String> {
        let Some(profile) = &self.profile.feeds else {
            return json_response(200, json!({"configured":false,"subscriptions":[]}));
        };
        let subscriptions: Vec<serde_json::Value> = self
            .state
            .feeds
            .subscriptions
            .values()
            .map(serde_json::to_value)
            .collect::<Result<Vec<serde_json::Value>, serde_json::Error>>()
            .map_err(|_| "feed state encoding failed")?
            .into_iter()
            .map(|mut subscription| {
                if role != Role::Operator {
                    if let Some(object) = subscription.as_object_mut() {
                        object.remove("url");
                    }
                }
                subscription
            })
            .collect();
        let mut repositories: BTreeMap<String, (String, usize, u64)> = BTreeMap::new();
        for project in &self.profile.projects {
            let key = project.repository_key();
            let count = self
                .state
                .runs
                .values()
                .filter(|run| run.project_id == project.id)
                .count();
            let newest = self
                .state
                .runs
                .values()
                .filter(|run| run.project_id == project.id)
                .map(run_updated)
                .max()
                .unwrap_or(0);
            repositories
                .entry(key)
                .and_modify(|entry| {
                    entry.2 = entry.2.max(newest);
                })
                .or_insert((project.label.clone(), count, newest));
        }
        let token = match (role, &self.feeds_token) {
            (Role::Operator, Some(token)) => Some(token.as_str()),
            _ => None,
        };
        let publication: Vec<serde_json::Value> = repositories
            .into_iter()
            .map(|(key, (label, runs, _))| {
                let path = format!("/feeds/{key}.xml");
                let url = format!("{}{}", self.profile.origin, path);
                let subscribe_url = token.map(|token| format!("{url}?token={token}"));
                json!({
                    "key": key,
                    "label": label,
                    "runs": runs,
                    "url": url,
                    "subscribe_url": subscribe_url,
                })
            })
            .collect();
        let index_token = token.map(|token| format!("token={token}"));
        json_response(
            200,
            json!({
                "configured": true,
                "bounds": {
                    "max_subscriptions": profile.max_subscriptions,
                    "max_entries_per_feed": profile.max_entries_per_feed,
                    "max_feed_bytes": profile.max_feed_bytes,
                    "refresh_interval_ms": profile.refresh_interval_ms,
                },
                "subscriptions": subscriptions,
                "publication": {
                    "index_url": format!("{}/feeds.xml", self.profile.origin),
                    "index_subscribe_url": index_token
                        .map(|query| format!("{}/feeds.xml?{query}", self.profile.origin)),
                    "repositories": publication,
                }
            }),
        )
    }

    /// Validate an add request without fetching or mutating state.
    pub(crate) fn prepare_feed_add(&self, body: &[u8]) -> Result<AdmittedFeed, String> {
        let profile = self
            .profile
            .feeds
            .as_ref()
            .ok_or("feeds are not configured")?;
        let request: AddFeed =
            serde_json::from_slice(body).map_err(|_| "feed request is malformed")?;
        let parsed = admit_url(&request.url, profile.allow_private_destinations)?;
        let id = subscription_id(parsed.as_str());
        if !self.state.feeds.subscriptions.contains_key(&id)
            && self.state.feeds.subscriptions.len() >= profile.max_subscriptions
        {
            return Err("feed subscription bound reached".to_string());
        }
        let label = request
            .label
            .filter(|label| !label.trim().is_empty())
            .unwrap_or_else(|| parsed.host_str().unwrap_or("repository feed").to_string());
        let label = label.trim().to_string();
        if label.len() > 128 || label.chars().any(char::is_control) {
            return Err("feed label is invalid".to_string());
        }
        Ok(AdmittedFeed {
            id,
            url: parsed.to_string(),
            label,
        })
    }

    pub(crate) fn finish_feed_add(
        &mut self,
        admitted: AdmittedFeed,
        fetched: Result<ParsedFeed, String>,
        now_ms: u64,
        method: &str,
        target: &str,
        principal: &Principal,
    ) -> Result<ApiResponse, String> {
        self.append_audit(method, target, Some(principal), "admitted", 0)?;
        let is_new = !self.state.feeds.subscriptions.contains_key(&admitted.id);
        if is_new
            && self.profile.feeds.as_ref().is_some_and(|profile| {
                self.state.feeds.subscriptions.len() >= profile.max_subscriptions
            })
        {
            self.append_audit(method, target, Some(principal), "denied", 409)?;
            return error(409, "feed_retention");
        }
        let parsed = match fetched {
            Ok(parsed) => parsed,
            Err(_) => {
                self.append_audit(method, target, Some(principal), "denied", 502)?;
                return error(502, "feed_unavailable");
            }
        };
        let subscription = FeedSubscription {
            id: admitted.id.clone(),
            url: admitted.url,
            label: admitted.label,
            title: parsed.title,
            fetched_at_ms: Some(now_ms),
            error: None,
            entries: parsed.entries,
        };
        if !self.feed_state_fits(&admitted.id, &subscription) {
            self.append_audit(method, target, Some(principal), "denied", 409)?;
            return error(409, "feed_retention");
        }
        self.state
            .feeds
            .subscriptions
            .insert(admitted.id.clone(), subscription.clone());
        self.persist()?;
        self.append_audit(method, target, Some(principal), "accepted", 201)?;
        json_response(201, json!({"subscription": subscription}))
    }

    pub(crate) fn feed_url(&self, id: &str) -> Option<String> {
        self.state
            .feeds
            .subscriptions
            .get(id)
            .map(|subscription| subscription.url.clone())
    }

    /// Subscriptions whose cadence elapsed, oldest attempt first.
    pub(crate) fn due_feeds(&self, now_ms: u64) -> Vec<(String, String)> {
        let Some(profile) = &self.profile.feeds else {
            return Vec::new();
        };
        if profile.refresh_interval_ms == 0 {
            return Vec::new();
        }
        crate::feeds::due_subscriptions(
            self.state.feeds.subscriptions.values(),
            now_ms,
            profile.refresh_interval_ms,
            crate::feeds::MAX_BACKGROUND_REFRESHES_PER_TICK,
        )
    }

    /// Record one background refresh. Missing subscriptions are ignored.
    pub(crate) fn record_feed_refresh(
        &mut self,
        id: &str,
        fetched: Result<ParsedFeed, String>,
        now_ms: u64,
    ) -> Result<(), String> {
        let Some(existing) = self.state.feeds.subscriptions.get(id).cloned() else {
            return Ok(());
        };
        let candidate = refreshed_candidate(existing, fetched, now_ms);
        if !self.feed_state_fits(id, &candidate) {
            return Ok(());
        }
        self.state
            .feeds
            .subscriptions
            .insert(id.to_string(), candidate);
        self.persist()
    }

    // r[impl kiln.web.feeds.subscriptions.scenario.refresh]
    pub(crate) fn finish_feed_refresh(
        &mut self,
        id: &str,
        fetched: Result<ParsedFeed, String>,
        now_ms: u64,
        method: &str,
        target: &str,
        principal: &Principal,
    ) -> Result<ApiResponse, String> {
        self.append_audit(method, target, Some(principal), "admitted", 0)?;
        let Some(existing) = self.state.feeds.subscriptions.get(id).cloned() else {
            self.append_audit(method, target, Some(principal), "denied", 404)?;
            return error(404, "feed_missing");
        };
        let candidate = refreshed_candidate(existing, fetched, now_ms);
        if !self.feed_state_fits(id, &candidate) {
            self.append_audit(method, target, Some(principal), "denied", 409)?;
            return error(409, "feed_retention");
        }
        self.state
            .feeds
            .subscriptions
            .insert(id.to_string(), candidate.clone());
        self.persist()?;
        if candidate.error.is_some() {
            self.append_audit(method, target, Some(principal), "denied", 502)?;
            return json_response(
                502,
                json!({"code":"feed_unavailable","message":"External feed refresh failed; retained entries were kept","subscription":candidate}),
            );
        }
        self.append_audit(method, target, Some(principal), "accepted", 200)?;
        json_response(200, json!({"subscription": candidate}))
    }

    pub(crate) fn remove_feed(
        &mut self,
        id: &str,
        method: &str,
        target: &str,
        principal: &Principal,
    ) -> Result<ApiResponse, String> {
        self.append_audit(method, target, Some(principal), "admitted", 0)?;
        if self.state.feeds.subscriptions.remove(id).is_none() {
            self.append_audit(method, target, Some(principal), "denied", 404)?;
            return error(404, "feed_missing");
        }
        self.persist()?;
        self.append_audit(method, target, Some(principal), "accepted", 200)?;
        json_response(200, json!({"removed": id}))
    }

    fn feed_state_fits(&self, id: &str, subscription: &FeedSubscription) -> bool {
        let mut candidate = self.state.clone();
        candidate
            .feeds
            .subscriptions
            .insert(id.to_string(), subscription.clone());
        serde_json::to_vec(&candidate)
            .is_ok_and(|bytes| bytes.len() <= self.profile.bounds.max_state_bytes)
    }

    /// Render a repository Atom feed. The caller has already verified the
    /// dedicated feed token and passes the path without its query string.
    // r[impl kiln.web.feeds.publish.scenario.rendered]
    // r[impl kiln.web.feeds.publish.scenario.missing]
    pub(crate) fn render_feed(&self, path: &str) -> Option<ApiResponse> {
        let profile = self.profile.feeds.as_ref()?;
        let token = self.feeds_token.as_ref()?;
        let origin = &self.profile.origin;
        if path == "/feeds.xml" {
            let mut repositories: BTreeMap<String, (String, usize, u64)> = BTreeMap::new();
            for project in &self.profile.projects {
                let key = project.repository_key();
                let runs: Vec<&super::model::RunRecord> = self
                    .state
                    .runs
                    .values()
                    .filter(|run| run.project_id == project.id)
                    .collect();
                let newest = runs.iter().map(|run| run_updated(run)).max().unwrap_or(0);
                repositories
                    .entry(key)
                    .and_modify(|entry| {
                        entry.1 += runs.len();
                        entry.2 = entry.2.max(newest);
                    })
                    .or_insert((project.label.clone(), runs.len(), newest));
            }
            let items: Vec<AtomItem> = repositories
                .into_iter()
                .map(|(key, (label, runs, newest))| AtomItem {
                    id: format!("urn:kiln:repository:{key}"),
                    title: format!("{label} CI runs"),
                    link: format!("{origin}/feeds/{key}.xml?token={}", token.as_str()),
                    updated_ms: newest,
                    summary: format!("{runs} retained runs for this repository"),
                })
                .collect();
            let newest = items.iter().map(|item| item.updated_ms).max().unwrap_or(0);
            let xml = render_atom(
                "urn:kiln:feeds:index",
                "Kiln repositories",
                &format!("{origin}/feeds.xml?token={}", token.as_str()),
                newest,
                &items,
            );
            return Some(ApiResponse {
                status: 200,
                content_type: "application/atom+xml; charset=utf-8",
                body: xml.into_bytes(),
                download_name: None,
            });
        }
        let key = path
            .strip_prefix("/feeds/")?
            .strip_suffix(".xml")
            .filter(|key| key.len() == 64 && key.bytes().all(|byte| byte.is_ascii_hexdigit()))?;
        let projects: BTreeMap<String, &crate::profile::ProjectProfile> = self
            .profile
            .projects
            .iter()
            .filter(|project| project.repository_key() == key)
            .map(|project| (project.id.clone(), project))
            .collect();
        if projects.is_empty() {
            return None;
        }
        let mut runs: Vec<crate::feeds::FeedRun> = self
            .state
            .runs
            .values()
            .filter(|run| projects.contains_key(&run.project_id))
            .map(|run| crate::feeds::FeedRun {
                id: run.id.clone(),
                project_id: run.project_id.clone(),
                project_label: projects
                    .get(&run.project_id)
                    .map(|project| project.label.clone())
                    .unwrap_or_else(|| "unknown project".to_string()),
                ordinal: run.ordinal,
                updated_ms: run_updated(run),
                outcome: outcome_label(run.outcome()).to_string(),
                revision: run.source.exact_revision.native_id.as_str().to_string(),
            })
            .collect();
        let label = projects
            .values()
            .next()
            .map(|project| project.label.as_str())
            .unwrap_or("Repository");
        let xml = crate::feeds::repository_atom(
            origin,
            key,
            &format!("{label} CI runs"),
            token.as_str(),
            &mut runs,
            profile.max_entries_per_feed,
        );
        Some(ApiResponse {
            status: 200,
            content_type: "application/atom+xml; charset=utf-8",
            body: xml.into_bytes(),
            download_name: None,
        })
    }
}

fn refreshed_candidate(
    existing: FeedSubscription,
    fetched: Result<ParsedFeed, String>,
    now_ms: u64,
) -> FeedSubscription {
    let mut candidate = existing;
    match fetched {
        Ok(parsed) => {
            candidate.title = parsed.title;
            candidate.entries = parsed.entries;
            candidate.error = None;
            candidate.fetched_at_ms = Some(now_ms);
        }
        Err(message) => {
            candidate.error = Some(message.chars().take(256).collect());
            candidate.fetched_at_ms = Some(now_ms);
        }
    }
    candidate
}

fn run_updated(run: &super::model::RunRecord) -> u64 {
    run.completed_at_ms
        .or(run.started_at_ms)
        .or(run.created_at_ms)
        .unwrap_or(0)
}

fn outcome_label(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Success => "Success",
        Outcome::DomainFailure => "Domain failure",
        Outcome::Denied => "Denied",
        Outcome::Cancelled => "Cancelled",
        Outcome::TimedOut => "Timed out",
        Outcome::ProviderError => "Provider error",
        Outcome::Pending => "Pending",
        Outcome::Unknown => "Unknown",
    }
}
