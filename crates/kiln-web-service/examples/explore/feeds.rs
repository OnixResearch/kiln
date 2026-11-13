//! Explicit offline feed simulator using Kiln's real bounded feed parsers/renderers.
//! The live fetch boundary is intentionally never called by this example.
use std::collections::BTreeMap;

use kiln_web_service::engine::{ApiResponse, Engine, Role};
use kiln_web_service::feeds::{self, AtomItem, FeedRun, FeedSubscription};
use serde_json::{Value, json};

use super::{Example, Result, accepted, bytes, error, now_ms};
const MAX_SUBSCRIPTIONS: usize = 16;
const MAX_ENTRIES: usize = 8;
const FEED_TOKEN: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
const CATALOG: [(&str, &str); 5] = [
    ("releases", "Example releases"),
    ("paused", "Paused upstream"),
    ("outage", "Unavailable upstream, retained entries"),
    ("pending", "Pending first observation"),
    ("new", "New sample subscription"),
];

pub struct Feeds {
    origin: String,
    subscriptions: BTreeMap<String, FeedSubscription>,
}
fn label(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}
fn catalog_url(url: &str) -> Result<(String, &'static str)> {
    let parsed = feeds::admit_url(url, false)?;
    if parsed.host_str() != Some("examples.invalid")
        || parsed.query().is_some()
        || parsed.port().is_some()
    {
        return Err("Offline examples only admit catalog URLs under https://examples.invalid; no external destination is fetched".into());
    }
    for (name, title) in CATALOG {
        if parsed.path() == format!("/{name}.atom") {
            return Ok((parsed.to_string(), title));
        }
    }
    Err("Use https://examples.invalid/new.atom (or another named example feed); arbitrary feeds require the live service".into())
}
impl Feeds {
    // r[impl kiln.web.exploration.scenario.coverage]
    pub fn seed(origin: &str) -> Result<Self> {
        let mut feeds = Self {
            origin: origin.into(),
            subscriptions: BTreeMap::new(),
        };
        for (name, title) in CATALOG.into_iter().take(4) {
            let url = format!("https://examples.invalid/{name}.atom");
            let mut subscription = feeds.subscription(&url, title)?;
            if name == "paused" {
                subscription.enabled = false;
            }
            if name == "outage" {
                subscription.error=Some("Simulated upstream outage. Retained entries are preserved; no request was sent.".into());
                subscription.failures = 2;
            }
            if name == "pending" {
                subscription.fetched_at_ms = None;
                subscription.last_success_ms = None;
                subscription.entries.clear();
            }
            feeds
                .subscriptions
                .insert(subscription.id.clone(), subscription);
        }
        Ok(feeds)
    }
    fn subscription(&self, url: &str, label: &str) -> Result<FeedSubscription> {
        let items=vec![
            AtomItem { id:format!("{url}#one"),title:"Synthetic release: exact source retained".into(),link:format!("{}/?page=projects",self.origin),updated_ms:now_ms()?,summary:"An illustrative upstream entry. Follow the local CI projects without leaving the exploration workspace.".into() },
            AtomItem { id:format!("{url}#two"),title:"Synthetic maintenance note: investigate failures".into(),link:format!("{}/?page=runs&outcome=DomainFailure",self.origin),updated_ms:now_ms()?.saturating_sub(3_600_000),summary:"A second parsed Atom entry for filtering and safe text rendering. No upstream fetch occurred.".into() },
        ];
        let xml = feeds::render_atom(url, "Synthetic upstream feed", url, now_ms()?, &items);
        let parsed = feeds::parse(xml.as_bytes(), MAX_ENTRIES)?;
        Ok(FeedSubscription {
            id: feeds::subscription_id(url),
            url: url.into(),
            label: label.into(),
            title: parsed.title,
            fetched_at_ms: Some(now_ms()?),
            last_success_ms: Some(now_ms()?),
            error: None,
            etag: Some("synthetic-local-observation".into()),
            last_modified: None,
            failures: 0,
            enabled: true,
            entries: parsed.entries,
        })
    }
    pub fn examples(&self) -> Vec<Example> {
        vec![
            Example::new(
                "Feeds: repository publication and offline upstream entries",
                "/?page=feeds",
                "Inspect observed, paused, failing and pending subscriptions; filter entry text, pause/resume/rename, export OPML and open local Atom/RSS. No remote feed is fetched.",
            ),
            Example::new(
                "Feed subscription rehearsal",
                "/?page=feeds&feed_q=Example",
                "Subscribe to https://examples.invalid/new.atom, then rename or pause it. Only the named offline catalog is admitted; arbitrary URLs fail explicitly.",
            ),
            Example::new(
                "Feed outage preserves retained entries",
                "/?page=feeds&feed_q=Unavailable",
                "Refresh the outage example to inspect a failure that preserves earlier entries. Other catalog feeds refresh from locally parsed sample bytes.",
            ),
            Example::new(
                "Feed OPML import/export and local publication",
                "/?page=feeds",
                "Export subscriptions, remove a local sample and import the OPML again. Repository feed links project the current retained synthetic runs.",
            ),
        ]
    }
    fn retained_runs(engine: &mut Engine) -> Result<Vec<FeedRun>> {
        let mut runs = Vec::new();
        let mut cursor = String::new();
        loop {
            let target = if cursor.is_empty() {
                "/api/runs".into()
            } else {
                format!("/api/runs?cursor={cursor}")
            };
            let page = accepted(engine.api("GET", &target, &[], Role::Operator)?)?;
            for row in page
                .get("runs")
                .ok_or("Retained run list absent")?
                .as_array()
                .ok_or("Retained feed runs unavailable")?
            {
                let text = |key: &str| -> Result<String> {
                    Ok(row[key]
                        .as_str()
                        .ok_or_else(|| format!("Feed run lacks {key}"))?
                        .into())
                };
                runs.push(FeedRun {
                    id: text("id")?,
                    project_id: text("project_id")?,
                    project_label: format!("{} · simulated", text("project_id")?),
                    ordinal: row["ordinal"].as_u64().ok_or("Feed run ordinal absent")?,
                    updated_ms: row["completed_at_ms"]
                        .as_u64()
                        .or_else(|| row["started_at_ms"].as_u64())
                        .or_else(|| row["created_at_ms"].as_u64())
                        .ok_or("Feed run timestamp absent")?,
                    outcome: text("outcome")?,
                    revision: text("source_revision")?,
                });
            }
            match page
                .get("next_cursor")
                .ok_or("Run cursor field absent")?
                .as_str()
            {
                Some(next) if next != cursor => cursor = next.into(),
                Some(_) => return Err("Feed run cursor repeated".into()),
                None => break,
            }
        }
        Ok(runs)
    }
    // Catalog refreshes are simulations, not a fallback for failed live requests.
    pub fn response(
        &mut self,
        method: &str,
        target: &str,
        body: &[u8],
        engine: &mut Engine,
    ) -> Result<ApiResponse> {
        let url = reqwest::Url::parse(&format!("{}{target}", self.origin))?;
        let path = url.path();
        let key = blake3::hash(b"synthetic-retained-ci-feed")
            .to_hex()
            .to_string();
        let feed_path = format!("/feeds/{key}.xml");
        let atom_url = format!("{}{feed_path}?token={FEED_TOKEN}", self.origin);
        if method == "GET"
            && (path.starts_with("/feeds/")
                || matches!(path, "/feeds.xml" | "/feeds.rss" | "/feeds.opml"))
        {
            if !url
                .query_pairs()
                .any(|(name, value)| name == "token" && value == FEED_TOKEN)
            {
                return Ok(error(
                    403,
                    "The local example feed URL requires its synthetic read-only token",
                ));
            }
            let title = "Synthetic retained Kiln runs";
            if path == "/feeds.opml" {
                return Ok(bytes(
                    "text/x-opml; charset=utf-8",
                    feeds::render_opml(title, &[(title, atom_url.as_str())]).as_bytes(),
                ));
            }
            let mut runs = Self::retained_runs(engine)?;
            if path == format!("{feed_path}.rss") || path == "/feeds.rss" {
                let items = feeds::repository_items(&self.origin, &mut runs, 100);
                return Ok(bytes(
                    "application/rss+xml; charset=utf-8",
                    feeds::render_rss(
                        title,
                        &atom_url,
                        "Synthetic observations, not executed CI",
                        &items,
                    )
                    .as_bytes(),
                ));
            }
            if path != feed_path && path != "/feeds.xml" {
                return Ok(error(404, "Local repository feed not found"));
            }
            return Ok(bytes(
                "application/atom+xml; charset=utf-8",
                feeds::repository_atom(&self.origin, &key, title, FEED_TOKEN, &mut runs, 100)
                    .as_bytes(),
            ));
        }
        if method == "GET" && path == "/api/feeds/export" {
            let rows: Vec<_> = self
                .subscriptions
                .values()
                .map(|row| (row.label.as_str(), row.url.as_str()))
                .collect();
            let mut response = bytes(
                "text/x-opml; charset=utf-8",
                feeds::render_opml("Synthetic offline subscriptions", &rows).as_bytes(),
            );
            response.download_name = Some("kiln-example-feeds.opml".into());
            return Ok(response);
        }
        if method == "GET" && path == "/api/feeds" {
            let needle = url
                .query_pairs()
                .find(|(key, _)| key == "q")
                .map(|(_, value)| value.to_lowercase())
                .unwrap_or_default();
            let subscriptions: Vec<_> = self
                .subscriptions
                .values()
                .filter(|row| {
                    format!(
                        "{} {} {}",
                        row.label,
                        row.url,
                        row.entries
                            .iter()
                            .map(|entry| format!(
                                "{} {}",
                                entry.title,
                                entry.summary.as_deref().unwrap_or_default()
                            ))
                            .collect::<String>()
                    )
                    .to_lowercase()
                    .contains(&needle)
                })
                .collect();
            let runs = Self::retained_runs(engine)?;
            return Ok(super::json(
                200,
                json!({"configured":true,"synthetic":true,"bounds":{"max_subscriptions":MAX_SUBSCRIPTIONS,"max_entries_per_feed":MAX_ENTRIES,"max_feed_bytes":65536,"max_import_bytes":65536,"refresh_interval_ms":0},"subscriptions":subscriptions,"summary":{"total":self.subscriptions.len(),"paused":self.subscriptions.values().filter(|row|!row.enabled).count(),"failing":self.subscriptions.values().filter(|row|row.error.is_some()).count(),"pending":self.subscriptions.values().filter(|row|row.fetched_at_ms.is_none()).count()},"publication":{"repositories":[{"key":key,"label":"Synthetic CI workspace","runs":runs.len(),"url":atom_url,"subscribe_url":atom_url,"rss_subscribe_url":format!("{}{feed_path}.rss?token={FEED_TOKEN}",self.origin)}],"opml_subscribe_url":format!("{}/feeds.opml?token={FEED_TOKEN}",self.origin)}}),
            ));
        }
        if method != "POST" {
            return Ok(error(405, "Unsupported offline feed operation"));
        }
        let input: Value = match serde_json::from_slice(body) {
            Ok(value) => value,
            Err(_) => return Ok(error(400, "Expected a bounded JSON feed command")),
        };
        if path == "/api/feeds" || path == "/api/feeds/import" {
            let candidates = if path.ends_with("/import") {
                let Some(opml) = input
                    .get("opml")
                    .and_then(Value::as_str)
                    .filter(|value| value.len() <= 65536)
                else {
                    return Ok(error(400, "OPML exceeds its example bound"));
                };
                match feeds::parse_opml(opml.as_bytes(), MAX_SUBSCRIPTIONS) {
                    Ok(rows) => rows,
                    Err(cause) => return Ok(error(400, &cause)),
                }
            } else {
                let Some(url) = input.get("url").and_then(Value::as_str) else {
                    return Ok(error(400, "A catalog URL is required"));
                };
                vec![(
                    url.into(),
                    input
                        .get("label")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                )]
            };
            let mut prepared = Vec::new();
            let mut skipped = 0;
            for (url, title) in candidates {
                let (url, default) = match catalog_url(&url) {
                    Ok(value) => value,
                    Err(cause) => return Ok(error(422, &cause.to_string())),
                };
                let title = title.as_deref().unwrap_or(default);
                if !label(title) {
                    return Ok(error(400, "Feed label must be bounded nonempty text"));
                }
                let row = self.subscription(&url, title)?;
                if self.subscriptions.contains_key(&row.id)
                    || prepared
                        .iter()
                        .any(|item: &FeedSubscription| item.id == row.id)
                {
                    skipped += 1;
                } else {
                    prepared.push(row);
                }
            }
            if self.subscriptions.len() + prepared.len() > MAX_SUBSCRIPTIONS {
                return Ok(error(409, "Offline subscription bound reached"));
            }
            let added = prepared.len();
            for row in prepared {
                self.subscriptions.insert(row.id.clone(), row);
            }
            return Ok(super::json(
                201,
                json!({"added":added,"skipped":skipped,"synthetic":true}),
            ));
        }
        let Some(rest) = path.strip_prefix("/api/feeds/") else {
            return Ok(error(404, "Offline feed route not found"));
        };
        let Some((id, action)) = rest.split_once('/') else {
            return Ok(error(404, "Offline feed action not found"));
        };
        let Some(row) = self.subscriptions.get(id).cloned() else {
            return Ok(error(404, "Offline subscription not found"));
        };
        match action {
            "remove" => {
                self.subscriptions.remove(id);
                Ok(super::json(200, json!({"removed":true})))
            }
            "enabled" => {
                let Some(enabled) = input.get("enabled").and_then(Value::as_bool) else {
                    return Ok(error(400, "enabled must be a boolean"));
                };
                let mut next = row;
                next.enabled = enabled;
                self.subscriptions.insert(id.into(), next.clone());
                Ok(super::json(200, json!({"subscription":next})))
            }
            "label" => {
                let Some(title) = input
                    .get("label")
                    .and_then(Value::as_str)
                    .filter(|value| label(value))
                else {
                    return Ok(error(400, "Feed label must be bounded nonempty text"));
                };
                let mut next = row;
                next.label = title.into();
                self.subscriptions.insert(id.into(), next.clone());
                Ok(super::json(200, json!({"subscription":next})))
            }
            "refresh" if row.url.ends_with("/outage.atom") => {
                let mut next = row;
                next.fetched_at_ms = Some(now_ms()?);
                next.failures = next.failures.saturating_add(1);
                next.error=Some("Simulated upstream outage; retained entries kept. No network request was sent.".into());
                self.subscriptions.insert(id.into(), next.clone());
                Ok(super::json(
                    502,
                    json!({"code":"synthetic_feed_outage","message":next.error,"subscription":next}),
                ))
            }
            "refresh" => {
                let mut next = self.subscription(&row.url, &row.label)?;
                next.enabled = row.enabled;
                self.subscriptions.insert(id.into(), next.clone());
                Ok(super::json(200, json!({"subscription":next})))
            }
            _ => Ok(error(404, "Unsupported offline feed action")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_urls_are_exact_and_never_fall_back_to_live_fetching() -> Result {
        let (url, title) = catalog_url("https://examples.invalid/new.atom")?;
        assert_eq!(url, "https://examples.invalid/new.atom");
        assert_eq!(title, "New sample subscription");
        for url in [
            "https://upstream.invalid/new.atom",
            "https://examples.invalid.attacker.invalid/new.atom",
            "http://examples.invalid/new.atom",
            "https://user:secret@examples.invalid/new.atom",
            "https://examples.invalid/unknown.atom",
            "https://examples.invalid/new.atom?forward=https://upstream.invalid",
            "https://examples.invalid:8443/new.atom",
        ] {
            assert!(catalog_url(url).is_err(), "{url}");
        }
        Ok(())
    }
}
