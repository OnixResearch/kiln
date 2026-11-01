use std::collections::{BTreeMap, BTreeSet};

use kiln_core::domain::Outcome;
use serde::Deserialize;
use serde_json::json;

use super::{ApiResponse, Engine, Role, api::error, api::json_response};
use crate::access::Principal;
use crate::feeds::{
    AtomItem, FeedSubscription, FetchOutcome, admit_url, render_atom, render_opml, subscription_id,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddFeed {
    url: String,
    #[serde(default)]
    label: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportFeeds {
    opml: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RenameFeed {
    label: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ToggleFeed {
    enabled: bool,
}

pub(crate) struct AdmittedFeed {
    pub id: String,
    pub url: String,
    pub label: String,
}

pub(crate) struct AdmittedImport {
    pub feeds: Vec<AdmittedFeed>,
    pub skipped: usize,
}

// r[impl kiln.web.feeds.subscriptions]
// r[impl kiln.web.feeds.publish]
impl Engine {
    pub(crate) fn feeds_view(
        &self,
        role: Role,
        query: Option<&str>,
    ) -> Result<ApiResponse, String> {
        let Some(profile) = &self.profile.feeds else {
            return json_response(200, json!({"configured":false,"subscriptions":[]}));
        };
        let needle = query
            .map(str::to_lowercase)
            .filter(|query| !query.is_empty());
        let mut subscriptions: Vec<serde_json::Value> = Vec::new();
        for subscription in self.state.feeds.subscriptions.values() {
            let label_match = needle.as_ref().is_none_or(|query| {
                subscription.label.to_lowercase().contains(query)
                    || subscription
                        .title
                        .as_deref()
                        .is_some_and(|title| title.to_lowercase().contains(query))
                    || subscription.url.to_lowercase().contains(query)
            });
            let mut entries = subscription.entries.clone();
            if let Some(query) = &needle {
                if !label_match {
                    entries.retain(|entry| {
                        entry.title.to_lowercase().contains(query)
                            || entry
                                .summary
                                .as_deref()
                                .is_some_and(|summary| summary.to_lowercase().contains(query))
                    });
                    if entries.is_empty() {
                        continue;
                    }
                }
            }
            let mut value =
                serde_json::to_value(subscription).map_err(|_| "feed state encoding failed")?;
            if let Some(object) = value.as_object_mut() {
                object.insert(
                    "entries".into(),
                    serde_json::to_value(entries).map_err(|_| "feed state encoding failed")?,
                );
                if role != Role::Operator {
                    object.remove("url");
                }
            }
            subscriptions.push(value);
        }
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
            .filter(|(key, (label, _, _))| {
                needle
                    .as_ref()
                    .is_none_or(|query| label.to_lowercase().contains(query) || key.contains(query))
            })
            .map(|(key, (label, runs, _))| {
                let path = format!("/feeds/{key}.xml");
                let url = format!("{}{}", self.profile.origin, path);
                let mut row = json!({
                    "key": key,
                    "label": label,
                    "runs": runs,
                    "url": url,
                });
                if let Some(token) = token {
                    if let Some(object) = row.as_object_mut() {
                        object.insert(
                            "subscribe_url".into(),
                            json!(format!("{url}?token={token}")),
                        );
                        object.insert(
                            "rss_subscribe_url".into(),
                            json!(format!("{url}.rss?token={token}")),
                        );
                    }
                }
                row
            })
            .collect();
        json_response(
            200,
            json!({
                "configured": true,
                "bounds": {
                    "max_subscriptions": profile.max_subscriptions,
                    "max_entries_per_feed": profile.max_entries_per_feed,
                    "max_feed_bytes": profile.max_feed_bytes,
                    "max_import_bytes": self.profile.bounds.max_request_bytes.saturating_sub(512) / 2,
                    "refresh_interval_ms": profile.refresh_interval_ms,
                },
                "subscriptions": subscriptions,
                "publication": publication_view(
                    &self.profile.origin,
                    token,
                    &publication,
                ),
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
        let admitted = self.admit_feed(&request.url, request.label.as_deref())?;
        if !self.state.feeds.subscriptions.contains_key(&admitted.id)
            && self.state.feeds.subscriptions.len() >= profile.max_subscriptions
        {
            return Err("feed subscription bound reached".to_string());
        }
        Ok(admitted)
    }

    /// Validate an OPML import without fetching or mutating state.
    pub(crate) fn prepare_feed_import(&self, body: &[u8]) -> Result<AdmittedImport, String> {
        let profile = self
            .profile
            .feeds
            .as_ref()
            .ok_or("feeds are not configured")?;
        let request: ImportFeeds =
            serde_json::from_slice(body).map_err(|_| "feed import is malformed")?;
        let outlines =
            crate::feeds::parse_opml(request.opml.as_bytes(), crate::feeds::MAX_OPML_OUTLINES)?;
        let remaining = profile
            .max_subscriptions
            .saturating_sub(self.state.feeds.subscriptions.len());
        if remaining == 0 {
            return Err("feed subscription bound reached".to_string());
        }
        let mut admitted = Vec::new();
        let mut ids = std::collections::BTreeSet::new();
        let outlines_len = outlines.len();
        for (url, label) in outlines {
            if admitted.len() >= remaining {
                break;
            }
            let Ok(feed) = self.admit_feed(&url, label.as_deref()) else {
                continue;
            };
            if self.state.feeds.subscriptions.contains_key(&feed.id) {
                continue;
            }
            if ids.insert(feed.id.clone()) {
                admitted.push(feed);
            }
        }
        if admitted.is_empty() {
            return Err("feed import has no new admitted URLs".to_string());
        }
        let skipped = outlines_len.saturating_sub(admitted.len());
        Ok(AdmittedImport {
            feeds: admitted,
            skipped,
        })
    }

    fn admit_feed(&self, url: &str, label: Option<&str>) -> Result<AdmittedFeed, String> {
        let profile = self
            .profile
            .feeds
            .as_ref()
            .ok_or("feeds are not configured")?;
        let parsed = admit_url(url, profile.allow_private_destinations)?;
        let label = label
            .filter(|label| !label.trim().is_empty())
            .unwrap_or_else(|| parsed.host_str().unwrap_or("repository feed"))
            .trim()
            .to_string();
        if label.len() > 128 || label.chars().any(char::is_control) {
            return Err("feed label is invalid".to_string());
        }
        Ok(AdmittedFeed {
            id: subscription_id(parsed.as_str()),
            url: parsed.to_string(),
            label,
        })
    }

    pub(crate) fn finish_feed_import(
        &mut self,
        import: AdmittedImport,
        method: &str,
        target: &str,
        principal: &Principal,
    ) -> Result<ApiResponse, String> {
        self.append_audit(method, target, Some(principal), "admitted", 0)?;
        let mut candidate = self.state.clone();
        let mut added = 0;
        for feed in import.feeds {
            if candidate.feeds.subscriptions.contains_key(&feed.id) {
                continue;
            }
            let id = feed.id;
            candidate.feeds.subscriptions.insert(
                id.clone(),
                FeedSubscription {
                    id,
                    url: feed.url,
                    label: feed.label,
                    title: None,
                    fetched_at_ms: None,
                    last_success_ms: None,
                    error: None,
                    etag: None,
                    last_modified: None,
                    failures: 0,
                    enabled: true,
                    entries: Vec::new(),
                },
            );
            added += 1;
        }
        if serde_json::to_vec(&candidate).map_or(true, |bytes| {
            bytes.len() > self.profile.bounds.max_state_bytes
        }) {
            self.append_audit(method, target, Some(principal), "denied", 409)?;
            return error(409, "feed_retention");
        }
        self.state = candidate;
        self.persist()?;
        self.append_audit(method, target, Some(principal), "accepted", 201)?;
        json_response(
            201,
            json!({
                "added": added,
                "skipped": import.skipped,
                "subscriptions": self.state.feeds.subscriptions.len()
            }),
        )
    }

    pub(crate) fn finish_feed_add(
        &mut self,
        admitted: AdmittedFeed,
        fetched: Result<FetchOutcome, String>,
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
        let (feed, etag, last_modified) = match fetched {
            Ok(FetchOutcome::Modified {
                feed,
                etag,
                last_modified,
            }) => (feed, etag, last_modified),
            Ok(FetchOutcome::NotModified { .. }) => {
                self.append_audit(method, target, Some(principal), "denied", 502)?;
                return error(502, "feed_unavailable");
            }
            Err(message) => {
                self.append_audit(method, target, Some(principal), "denied", 502)?;
                return json_response(
                    502,
                    json!({
                        "code": "feed_unavailable",
                        "message": message.chars().take(256).collect::<String>()
                    }),
                );
            }
        };
        let subscription = FeedSubscription {
            id: admitted.id.clone(),
            url: admitted.url,
            label: admitted.label,
            title: feed.title,
            fetched_at_ms: Some(now_ms),
            last_success_ms: Some(now_ms),
            error: None,
            etag,
            last_modified,
            failures: 0,
            enabled: true,
            entries: feed.entries,
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

    pub(crate) fn feed_refresh_target(
        &self,
        id: &str,
    ) -> Option<(String, Option<String>, Option<String>)> {
        self.state.feeds.subscriptions.get(id).map(|subscription| {
            (
                subscription.url.clone(),
                subscription.etag.clone(),
                subscription.last_modified.clone(),
            )
        })
    }

    /// Subscriptions whose cadence elapsed, oldest attempt first.
    pub(crate) fn due_feeds(&self, now_ms: u64) -> Vec<crate::feeds::DueFeed> {
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
        fetched: Result<FetchOutcome, String>,
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
        fetched: Result<FetchOutcome, String>,
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

    pub(crate) fn set_feed_enabled(
        &mut self,
        id: &str,
        body: &[u8],
        method: &str,
        target: &str,
        principal: &Principal,
    ) -> Result<ApiResponse, String> {
        self.append_audit(method, target, Some(principal), "admitted", 0)?;
        let request: ToggleFeed =
            serde_json::from_slice(body).map_err(|_| "feed toggle request is malformed")?;
        let Some(existing) = self.state.feeds.subscriptions.get(id).cloned() else {
            self.append_audit(method, target, Some(principal), "denied", 404)?;
            return error(404, "feed_missing");
        };
        let mut candidate = existing;
        candidate.enabled = request.enabled;
        if !self.feed_state_fits(id, &candidate) {
            self.append_audit(method, target, Some(principal), "denied", 409)?;
            return error(409, "feed_retention");
        }
        self.state
            .feeds
            .subscriptions
            .insert(id.to_string(), candidate.clone());
        self.persist()?;
        self.append_audit(method, target, Some(principal), "accepted", 200)?;
        json_response(200, json!({"subscription": candidate}))
    }

    pub(crate) fn rename_feed(
        &mut self,
        id: &str,
        body: &[u8],
        method: &str,
        target: &str,
        principal: &Principal,
    ) -> Result<ApiResponse, String> {
        self.append_audit(method, target, Some(principal), "admitted", 0)?;
        let request: RenameFeed =
            serde_json::from_slice(body).map_err(|_| "feed label request is malformed")?;
        let label = request.label.trim().to_string();
        if label.is_empty() || label.len() > 128 || label.chars().any(char::is_control) {
            self.append_audit(method, target, Some(principal), "denied", 400)?;
            return error(400, "feed_label_invalid");
        }
        let Some(existing) = self.state.feeds.subscriptions.get(id).cloned() else {
            self.append_audit(method, target, Some(principal), "denied", 404)?;
            return error(404, "feed_missing");
        };
        let mut candidate = existing;
        candidate.label = label;
        if !self.feed_state_fits(id, &candidate) {
            self.append_audit(method, target, Some(principal), "denied", 409)?;
            return error(409, "feed_retention");
        }
        self.state
            .feeds
            .subscriptions
            .insert(id.to_string(), candidate.clone());
        self.persist()?;
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
    fn repository_feed(&self, key: &str) -> Option<(String, Vec<crate::feeds::FeedRun>)> {
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
        let runs = self
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
            .map(|project| project.label.clone())
            .unwrap_or_else(|| "Repository".to_string());
        Some((label, runs))
    }

    // r[impl kiln.web.feeds.publish.scenario.rendered]
    // r[impl kiln.web.feeds.publish.scenario.missing]
    pub(crate) fn render_feed(&self, path: &str) -> Option<ApiResponse> {
        let profile = self.profile.feeds.as_ref()?;
        let token = self.feeds_token.as_ref()?;
        let origin = &self.profile.origin;
        if path == "/feeds.opml" {
            let mut seen = BTreeSet::new();
            let mut feeds = Vec::new();
            for project in &self.profile.projects {
                let key = project.repository_key();
                if seen.insert(key.clone()) {
                    feeds.push((
                        project.label.clone(),
                        format!("{origin}/feeds/{key}.xml?token={}", token.as_str()),
                    ));
                }
            }
            let borrowed: Vec<(&str, &str)> = feeds
                .iter()
                .map(|(label, url)| (label.as_str(), url.as_str()))
                .collect();
            return Some(ApiResponse {
                status: 200,
                content_type: "text/x-opml; charset=utf-8",
                body: render_opml("Kiln repositories", &borrowed).into_bytes(),
                download_name: None,
                headers: Vec::new(),
            });
        }
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
                headers: Vec::new(),
            });
        }
        if path == "/feeds.rss" {
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
                    link: format!("{origin}/feeds/{key}.rss?token={}", token.as_str()),
                    updated_ms: newest,
                    summary: format!("{runs} retained runs for this repository"),
                })
                .collect();
            let xml = crate::feeds::render_rss(
                "Kiln repositories",
                &format!("{origin}/feeds.rss?token={}", token.as_str()),
                "Retained CI runs by repository",
                &items,
            );
            return Some(ApiResponse {
                status: 200,
                content_type: "application/rss+xml; charset=utf-8",
                body: xml.into_bytes(),
                download_name: None,
                headers: Vec::new(),
            });
        }
        let rest = path.strip_prefix("/feeds/")?;
        let (key, rss) = if let Some(key) = rest.strip_suffix(".xml") {
            (key, false)
        } else if let Some(key) = rest.strip_suffix(".rss") {
            (key, true)
        } else {
            return None;
        };
        if key.len() != 64 || !key.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let (label, mut runs) = self.repository_feed(key)?;
        let xml = if rss {
            let items =
                crate::feeds::repository_items(origin, &mut runs, profile.max_entries_per_feed);
            crate::feeds::render_rss(
                &format!("{label} CI runs"),
                &format!("{origin}/feeds/{key}.rss?token={}", token.as_str()),
                &format!("Retained CI runs for {label}"),
                &items,
            )
        } else {
            crate::feeds::repository_atom(
                origin,
                key,
                &format!("{label} CI runs"),
                token.as_str(),
                &mut runs,
                profile.max_entries_per_feed,
            )
        };
        Some(ApiResponse {
            status: 200,
            content_type: if rss {
                "application/rss+xml; charset=utf-8"
            } else {
                "application/atom+xml; charset=utf-8"
            },
            body: xml.into_bytes(),
            download_name: None,
            headers: Vec::new(),
        })
    }
}

fn publication_view(
    origin: &str,
    token: Option<&str>,
    repositories: &[serde_json::Value],
) -> serde_json::Value {
    let mut publication = json!({
        "index_url": format!("{origin}/feeds.xml"),
        "opml_url": format!("{origin}/feeds.opml"),
        "repositories": repositories,
    });
    if let (Some(token), Some(object)) = (token, publication.as_object_mut()) {
        object.insert(
            "index_subscribe_url".into(),
            json!(format!("{origin}/feeds.xml?token={token}")),
        );
        object.insert(
            "opml_subscribe_url".into(),
            json!(format!("{origin}/feeds.opml?token={token}")),
        );
    }
    publication
}

fn refreshed_candidate(
    existing: FeedSubscription,
    fetched: Result<FetchOutcome, String>,
    now_ms: u64,
) -> FeedSubscription {
    let mut candidate = existing;
    match fetched {
        Ok(FetchOutcome::Modified {
            feed,
            etag,
            last_modified,
        }) => {
            candidate.title = feed.title;
            candidate.entries = feed.entries;
            candidate.error = None;
            candidate.etag = etag;
            candidate.last_modified = last_modified;
            candidate.failures = 0;
            candidate.fetched_at_ms = Some(now_ms);
            candidate.last_success_ms = Some(now_ms);
        }
        Ok(FetchOutcome::NotModified {
            etag,
            last_modified,
        }) => {
            candidate.error = None;
            candidate.etag = etag.or(candidate.etag);
            candidate.last_modified = last_modified.or(candidate.last_modified);
            candidate.failures = 0;
            candidate.fetched_at_ms = Some(now_ms);
            candidate.last_success_ms = Some(now_ms);
        }
        Err(message) => {
            candidate.error = Some(message.chars().take(256).collect());
            candidate.failures = candidate.failures.saturating_add(1);
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
