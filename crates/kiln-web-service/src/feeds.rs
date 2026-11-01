//! Bounded repository Atom publication and external RSS/Atom subscription support.

use std::collections::BTreeMap;
use std::io::Read as _;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs as _};
use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::header::{
    ACCEPT, CONTENT_LENGTH, ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED, LOCATION,
    USER_AGENT,
};
use reqwest::{Url, redirect::Policy};
use serde::{Deserialize, Serialize};

pub const ABSOLUTE_MAX_SUBSCRIPTIONS: usize = 128;
pub const ABSOLUTE_MAX_ENTRIES_PER_FEED: usize = 200;
pub const ABSOLUTE_MAX_FEED_BYTES: usize = 4_194_304;
pub const ABSOLUTE_MAX_REDIRECTS: usize = 5;
pub const ABSOLUTE_MAX_TIMEOUT_SECONDS: u64 = 30;
pub const DEFAULT_REFRESH_INTERVAL_MS: u64 = 900_000;
pub const MIN_REFRESH_INTERVAL_MS: u64 = 60_000;
pub const MAX_REFRESH_INTERVAL_MS: u64 = 86_400_000;
pub const MAX_BACKGROUND_REFRESHES_PER_TICK: usize = 2;
pub const MAX_OPML_OUTLINES: usize = 256;
pub const MAX_URL_BYTES: usize = 2_048;
pub const MAX_TITLE_BYTES: usize = 512;
pub const MAX_SUMMARY_BYTES: usize = 4_096;
pub const MAX_DATE_BYTES: usize = 128;
pub const FEED_ACCEPT: &str =
    "application/rss+xml, application/atom+xml, application/xml;q=0.9, text/xml;q=0.8";
const USER_AGENT_VALUE: &str = "kiln-web-service/repository-feeds";

/// One external feed subscription and its most recent bounded entries.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedSubscription {
    pub id: String,
    pub url: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_at_ms: Option<u64>,
    /// Last successful fetch or 304, distinct from a failed attempt time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_success_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// HTTP entity tag returned by the feed origin, when bounded and valid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    /// HTTP Last-Modified value returned by the feed origin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
    /// Consecutive failed refresh attempts, used for bounded backoff.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub failures: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<FeedEntry>,
}

/// One parsed external feed entry. Text is displayed as text, never HTML.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedEntry {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedsState {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub subscriptions: BTreeMap<String, FeedSubscription>,
}

impl FeedsState {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.subscriptions.is_empty()
    }
}

/// One parsed external feed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedFeed {
    pub title: Option<String>,
    pub entries: Vec<FeedEntry>,
}

/// Extract bounded subscription URLs from an OPML 2.0 document.
///
/// Only outlines with an `xmlUrl` are returned. Labels use `text` or `title`.
/// Duplicate URLs are collapsed. DTD and entity declarations are refused.
pub fn parse_opml(
    bytes: &[u8],
    max_outlines: usize,
) -> Result<Vec<(String, Option<String>)>, String> {
    if bytes.is_empty() || bytes.len() > ABSOLUTE_MAX_FEED_BYTES {
        return Err("OPML bytes are empty or exceed the absolute bound".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "OPML is not UTF-8")?;
    reject_unsafe_xml(text)?;
    let document = roxmltree::Document::parse(text).map_err(|_| "OPML XML is malformed")?;
    if document.root_element().tag_name().name() != "opml" {
        return Err("document is not OPML".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut outlines = Vec::new();
    for node in document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "outline")
    {
        let Some(url) = node.attribute("xmlUrl").map(str::trim) else {
            continue;
        };
        if url.is_empty() || url.len() > MAX_URL_BYTES || !seen.insert(url.to_string()) {
            continue;
        }
        let label = node
            .attribute("text")
            .or_else(|| node.attribute("title"))
            .map(clean_text)
            .filter(|label| !label.is_empty())
            .map(|label| truncate(&label, 128));
        outlines.push((url.to_string(), label));
        if outlines.len() >= max_outlines {
            break;
        }
    }
    if outlines.is_empty() {
        return Err("OPML contains no subscription outlines".into());
    }
    Ok(outlines)
}

/// One subscription selected for background refresh.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DueFeed {
    pub id: String,
    pub url: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

/// Bounds and validators for one external feed request.
#[derive(Clone, Copy, Debug)]
pub struct FetchOptions<'a> {
    pub max_bytes: usize,
    pub max_entries: usize,
    pub max_redirects: usize,
    pub timeout_seconds: u64,
    pub allow_private: bool,
    pub etag: Option<&'a str>,
    pub last_modified: Option<&'a str>,
}

/// Result of one bounded conditional feed request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FetchOutcome {
    /// The origin returned a feed body and optional validators.
    Modified {
        feed: ParsedFeed,
        etag: Option<String>,
        last_modified: Option<String>,
    },
    /// The origin confirmed the supplied validators without a body.
    NotModified {
        etag: Option<String>,
        last_modified: Option<String>,
    },
}

/// Stable identity for one subscription URL.
#[must_use]
pub fn subscription_id(url: &str) -> String {
    blake3::hash(url.as_bytes()).to_hex().to_string()
}

fn is_zero(value: &u32) -> bool {
    *value == 0
}

/// Effective retry interval after consecutive failures.
#[must_use]
pub fn failure_backoff_ms(interval_ms: u64, failures: u32) -> u64 {
    if interval_ms == 0 {
        return 0;
    }
    let exponent = failures.min(6);
    interval_ms
        .saturating_mul(1_u64 << exponent)
        .min(MAX_REFRESH_INTERVAL_MS)
}

/// Whether a subscription is due for a background refresh.
#[must_use]
pub fn is_due(now_ms: u64, fetched_at_ms: Option<u64>, interval_ms: u64) -> bool {
    interval_ms != 0
        && fetched_at_ms.is_none_or(|fetched| now_ms.saturating_sub(fetched) >= interval_ms)
}

/// Select due subscriptions, oldest attempt first, up to the per-tick bound.
#[must_use]
pub fn due_subscriptions<'a>(
    subscriptions: impl Iterator<Item = &'a FeedSubscription>,
    now_ms: u64,
    interval_ms: u64,
    limit: usize,
) -> Vec<DueFeed> {
    if interval_ms == 0 || limit == 0 {
        return Vec::new();
    }
    let mut due: Vec<(u64, DueFeed)> = subscriptions
        .filter(|subscription| {
            is_due(
                now_ms,
                subscription.fetched_at_ms,
                failure_backoff_ms(interval_ms, subscription.failures),
            )
        })
        .map(|subscription| {
            (
                subscription.fetched_at_ms.unwrap_or(0),
                DueFeed {
                    id: subscription.id.clone(),
                    url: subscription.url.clone(),
                    etag: subscription.etag.clone(),
                    last_modified: subscription.last_modified.clone(),
                },
            )
        })
        .collect();
    due.sort_by_key(|(attempt, _)| *attempt);
    due.truncate(limit);
    due.into_iter().map(|(_, feed)| feed).collect()
}

/// Validate a subscription URL before any DNS or network effect.
///
/// Public destinations must be HTTPS with no userinfo. Plain HTTP is accepted
/// only for an explicitly admitted private/test destination.
pub fn admit_url(url: &str, allow_private: bool) -> Result<Url, String> {
    if url.is_empty() || url.len() > MAX_URL_BYTES {
        return Err("feed URL is empty or exceeds its named bound".into());
    }
    if url
        .bytes()
        .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return Err("feed URL contains whitespace or control bytes".into());
    }
    let parsed = Url::parse(url).map_err(|_| "feed URL is not a valid absolute URL")?;
    let scheme_ok = parsed.scheme() == "https" || (allow_private && parsed.scheme() == "http");
    if !scheme_ok {
        return Err("feed URL must use HTTPS".into());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("feed URL must not embed credentials".into());
    }
    let host = parsed
        .host_str()
        .filter(|host| !host.is_empty())
        .ok_or("feed URL has no host")?;
    let bare = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    if let Ok(ip) = bare.parse::<IpAddr>() {
        if !allow_private && !globally_routable(ip) {
            return Err("feed URL host is not a globally routable address".into());
        }
    }
    if parsed.port().is_some() && parsed.port() != Some(443) && !allow_private {
        return Err("feed URL must use the default HTTPS port".into());
    }
    if parsed.fragment().is_some() {
        return Err("feed URL must not contain a fragment".into());
    }
    Ok(parsed)
}

/// Reject non-global destinations used for SSRF and local-service access.
#[must_use]
pub fn globally_routable(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => globally_routable_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return globally_routable_v4(v4);
            }
            !(v6.is_unspecified()
                || v6.is_loopback()
                || v6.is_multicast()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
                || (v6.segments()[0] & 0xffc0) == 0xfec0
                || is_documentation_v6(v6))
        }
    }
}

fn is_documentation_v6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    segments[0] == 0x2001 && segments[1] == 0x0db8
}

fn globally_routable_v4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_multicast()
        || octets[0] == 0
        || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        || (octets[0] == 198 && (18..=19).contains(&octets[1]))
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
        || (octets[0] == 192 && octets[1] == 88 && octets[2] == 99)
        || octets[0] >= 240)
}

// r[impl kiln.web.feeds.security]
// r[impl kiln.web.feeds.bounds]
/// Fetch and parse one external feed under the declared bounds.
///
/// DNS is resolved here, every address is admitted, and the connection is
/// pinned to the admitted address so a second resolution cannot rebind it.
/// Redirects are manual and re-admitted, and proxy environment variables are
/// ignored.
pub fn fetch(url: &str, options: FetchOptions<'_>) -> Result<FetchOutcome, String> {
    let FetchOptions {
        max_bytes,
        max_entries,
        max_redirects,
        timeout_seconds,
        allow_private,
        etag,
        last_modified,
    } = options;
    if max_bytes == 0 || max_bytes > ABSOLUTE_MAX_FEED_BYTES {
        return Err("feed byte bound is invalid".into());
    }
    if max_entries == 0 || max_entries > ABSOLUTE_MAX_ENTRIES_PER_FEED {
        return Err("feed entry bound is invalid".into());
    }
    if max_redirects > ABSOLUTE_MAX_REDIRECTS
        || timeout_seconds == 0
        || timeout_seconds > ABSOLUTE_MAX_TIMEOUT_SECONDS
    {
        return Err("feed fetch bounds are invalid".into());
    }
    let mut current = admit_url(url, allow_private)?;
    let mut etag = etag;
    let mut last_modified = last_modified;
    for hop in 0..=max_redirects {
        let client = pinned_client(&current, timeout_seconds, allow_private)?;
        let mut request = client
            .get(current.clone())
            .header(ACCEPT, FEED_ACCEPT)
            .header(USER_AGENT, USER_AGENT_VALUE);
        if let Some(etag) = etag.filter(|value| valid_validator(value)) {
            request = request.header(IF_NONE_MATCH, etag);
        }
        if let Some(last_modified) = last_modified.filter(|value| valid_validator(value)) {
            request = request.header(IF_MODIFIED_SINCE, last_modified);
        }
        let response = request
            .send()
            .map_err(|_| "external feed could not be reached")?;
        let status = response.status();
        if status == reqwest::StatusCode::NOT_MODIFIED {
            if etag.is_none() && last_modified.is_none() {
                return Err("external feed returned not-modified without a validator".into());
            }
            return Ok(FetchOutcome::NotModified {
                etag: response_validator(&response, ETAG).or_else(|| etag.map(ToString::to_string)),
                last_modified: response_validator(&response, LAST_MODIFIED)
                    .or_else(|| last_modified.map(ToString::to_string)),
            });
        }
        if status.is_redirection() {
            if hop == max_redirects {
                return Err("external feed exceeded its redirect bound".into());
            }
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or("external feed redirect has no valid location")?;
            if location.len() > MAX_URL_BYTES {
                return Err("external feed redirect location exceeds its bound".into());
            }
            current = current
                .join(location)
                .map_err(|_| "external feed redirect location is invalid")?;
            current = admit_url(current.as_str(), allow_private)?;
            // Validators belong to the origin that issued them; a redirect is a
            // different resource and must be fetched without conditional headers.
            etag = None;
            last_modified = None;
            continue;
        }
        if status != reqwest::StatusCode::OK {
            return Err("external feed returned a non-success status".into());
        }
        if let Some(length) = response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<usize>().ok())
        {
            if length > max_bytes {
                return Err("external feed content length exceeds its bound".into());
            }
        }
        let etag = response_validator(&response, ETAG);
        let last_modified = response_validator(&response, LAST_MODIFIED);
        let mut bytes = Vec::new();
        response
            .take((max_bytes as u64).saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|_| "external feed body could not be read")?;
        if bytes.len() > max_bytes {
            return Err("external feed body exceeds its bound".into());
        }
        return Ok(FetchOutcome::Modified {
            feed: parse(&bytes, max_entries)?,
            etag,
            last_modified,
        });
    }
    Err("external feed exceeded its redirect bound".into())
}

fn response_validator(
    response: &reqwest::blocking::Response,
    header: reqwest::header::HeaderName,
) -> Option<String> {
    response
        .headers()
        .get(header)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| valid_validator(value))
        .map(str::to_string)
}

fn valid_validator(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && !value
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte == b'\r' || byte == b'\n')
}

fn pinned_client(url: &Url, timeout_seconds: u64, allow_private: bool) -> Result<Client, String> {
    let host = url
        .host_str()
        .filter(|host| !host.is_empty())
        .ok_or("feed URL has no host")?
        .to_string();
    let bare = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host.as_str())
        .to_string();
    let port = url.port_or_known_default().ok_or("feed URL has no port")?;
    let addresses: Vec<SocketAddr> = if let Ok(ip) = bare.parse::<IpAddr>() {
        vec![SocketAddr::new(ip, port)]
    } else {
        (bare.as_str(), port)
            .to_socket_addrs()
            .map_err(|_| "feed host could not be resolved")?
            .collect()
    };
    if addresses.is_empty() {
        return Err("feed host resolved to no addresses".into());
    }
    if !allow_private
        && addresses
            .iter()
            .any(|address| !globally_routable(address.ip()))
    {
        return Err("feed host resolved to a non-global address".into());
    }
    let pinned = addresses
        .first()
        .copied()
        .ok_or("feed host resolved to no addresses")?;
    let mut builder = Client::builder()
        .redirect(Policy::none())
        .no_proxy()
        .timeout(Duration::from_secs(timeout_seconds))
        .connect_timeout(Duration::from_secs(timeout_seconds))
        .https_only(!allow_private);
    if bare.parse::<IpAddr>().is_err() {
        builder = builder.resolve(bare.as_str(), pinned);
    }
    let client = builder
        .build()
        .map_err(|_| "bounded feed client could not be created")?;
    Ok(client)
}

// r[impl kiln.web.feeds.security.scenario.malformed]
/// Parse RSS 2.0 or Atom into bounded text entries.
pub fn parse(bytes: &[u8], max_entries: usize) -> Result<ParsedFeed, String> {
    if bytes.is_empty() || bytes.len() > ABSOLUTE_MAX_FEED_BYTES {
        return Err("external feed bytes are empty or exceed the absolute bound".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "external feed is not UTF-8")?;
    reject_unsafe_xml(text)?;
    let document =
        roxmltree::Document::parse(text).map_err(|_| "external feed XML is malformed")?;
    let root = document.root_element();
    let root_name = root.tag_name().name();
    match root_name {
        "rss" => parse_rss(root, max_entries),
        "RDF" => parse_rdf(root, max_entries),
        "feed" => parse_atom(root, max_entries),
        _ => Err("external feed is not RSS, RSS 1.0/RDF or Atom".into()),
    }
}

fn reject_unsafe_xml(text: &str) -> Result<(), String> {
    let lower = text.to_ascii_lowercase();
    if lower.contains("<!doctype") || lower.contains("<!entity") {
        return Err("DTD and entity declarations are refused".into());
    }
    Ok(())
}

fn parse_rss(root: roxmltree::Node<'_, '_>, max_entries: usize) -> Result<ParsedFeed, String> {
    let channel = root
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "channel")
        .ok_or("RSS feed has no channel")?;
    let title = direct_text(channel, "title").map(|text| truncate(&text, MAX_TITLE_BYTES));
    let mut entries = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for item in channel
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == "item")
    {
        let entry = rss_entry(item);
        if seen.insert(entry.id.clone()) {
            entries.push(entry);
        }
        if entries.len() >= max_entries {
            break;
        }
    }
    Ok(ParsedFeed { title, entries })
}

/// RSS 1.0/RDF places items beside the channel, not inside it.
fn parse_rdf(root: roxmltree::Node<'_, '_>, max_entries: usize) -> Result<ParsedFeed, String> {
    let title = root
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "channel")
        .and_then(|channel| direct_text(channel, "title"))
        .map(|text| truncate(&text, MAX_TITLE_BYTES));
    let mut entries = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for item in root
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "item")
    {
        let entry = rss_entry(item);
        if seen.insert(entry.id.clone()) {
            entries.push(entry);
        }
        if entries.len() >= max_entries {
            break;
        }
    }
    Ok(ParsedFeed { title, entries })
}

fn rss_entry(item: roxmltree::Node<'_, '_>) -> FeedEntry {
    let title = direct_text(item, "title")
        .map(|text| truncate(&text, MAX_TITLE_BYTES))
        .unwrap_or_else(|| "(untitled)".to_string());
    let link = direct_text(item, "link").and_then(|link| safe_link(&link));
    let published = direct_text(item, "pubDate")
        .or_else(|| direct_text(item, "published"))
        .or_else(|| direct_text(item, "date"))
        .map(|text| truncate(&text, MAX_DATE_BYTES));
    let summary = direct_text(item, "description").map(|text| truncate(&text, MAX_SUMMARY_BYTES));
    let id = direct_text(item, "guid")
        .filter(|value| !value.is_empty())
        .or_else(|| link.clone())
        .unwrap_or_else(|| entry_fallback_id(&title, published.as_deref()));
    FeedEntry {
        id: truncate(&id, MAX_URL_BYTES),
        title,
        link,
        published,
        summary,
    }
}

fn parse_atom(root: roxmltree::Node<'_, '_>, max_entries: usize) -> Result<ParsedFeed, String> {
    let title = direct_text(root, "title").map(|text| truncate(&text, MAX_TITLE_BYTES));
    let mut entries = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for entry in root
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == "entry")
    {
        let title = direct_text(entry, "title")
            .map(|text| truncate(&text, MAX_TITLE_BYTES))
            .unwrap_or_else(|| "(untitled)".to_string());
        let link = atom_link(entry);
        let published = direct_text(entry, "published")
            .or_else(|| direct_text(entry, "updated"))
            .map(|text| truncate(&text, MAX_DATE_BYTES));
        let summary = direct_text(entry, "summary")
            .or_else(|| direct_text(entry, "content"))
            .map(|text| truncate(&text, MAX_SUMMARY_BYTES));
        let id = direct_text(entry, "id")
            .filter(|value| !value.is_empty())
            .or_else(|| link.clone())
            .unwrap_or_else(|| entry_fallback_id(&title, published.as_deref()));
        let entry = FeedEntry {
            id: truncate(&id, MAX_URL_BYTES),
            title,
            link,
            published,
            summary,
        };
        if seen.insert(entry.id.clone()) {
            entries.push(entry);
        }
        if entries.len() >= max_entries {
            break;
        }
    }
    Ok(ParsedFeed { title, entries })
}

fn atom_link(entry: roxmltree::Node<'_, '_>) -> Option<String> {
    let mut fallback = None;
    for link in entry
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() == "link")
    {
        let Some(href) = link.attribute("href") else {
            continue;
        };
        let rel = link.attribute("rel").unwrap_or("alternate");
        if rel == "alternate" {
            return safe_link(href);
        }
        if fallback.is_none() {
            fallback = safe_link(href);
        }
    }
    fallback
}

fn direct_text(node: roxmltree::Node<'_, '_>, name: &str) -> Option<String> {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name() == name)
        .map(|child| clean_text(child.text().unwrap_or_default()))
        .filter(|text| !text.is_empty())
}

fn clean_text(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_control() || *character == '\n' || *character == '\t')
        .collect::<String>()
        .trim()
        .to_string()
}

fn safe_link(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > MAX_URL_BYTES {
        return None;
    }
    let url = Url::parse(value).ok()?;
    matches!(url.scheme(), "http" | "https").then(|| truncate(value, MAX_URL_BYTES))
}

fn entry_fallback_id(title: &str, published: Option<&str>) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"kiln-feed-entry-v1\0");
    hasher.update(title.as_bytes());
    hasher.update(b"\0");
    hasher.update(published.unwrap_or_default().as_bytes());
    hasher.finalize().to_hex().to_string()
}

fn truncate(text: &str, maximum: usize) -> String {
    if text.len() <= maximum {
        return text.to_string();
    }
    let mut end = maximum;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

/// One retained run selected for a repository Atom feed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeedRun {
    pub id: String,
    pub project_id: String,
    pub project_label: String,
    pub ordinal: u64,
    pub updated_ms: u64,
    pub outcome: String,
    pub revision: String,
}

/// Select and render the newest retained runs for one repository.
#[must_use]
pub fn repository_items(origin: &str, runs: &mut Vec<FeedRun>, max_items: usize) -> Vec<AtomItem> {
    runs.sort_by(|left, right| right.ordinal.cmp(&left.ordinal));
    runs.truncate(max_items);
    runs.iter()
        .map(|run| AtomItem {
            id: format!("urn:kiln:run:{}", run.id),
            title: format!("{} · {}", run.project_label, run.outcome),
            link: format!("{origin}/?page=runs&run={}", run.id),
            updated_ms: run.updated_ms,
            summary: format!("Run {} on {} — {}", run.id, run.revision, run.outcome),
        })
        .collect()
}

pub fn repository_atom(
    origin: &str,
    repository_key: &str,
    title: &str,
    token: &str,
    runs: &mut Vec<FeedRun>,
    max_items: usize,
) -> String {
    let items = repository_items(origin, runs, max_items);
    let newest = items.iter().map(|item| item.updated_ms).max().unwrap_or(0);
    render_atom(
        &format!("urn:kiln:repository:{repository_key}"),
        title,
        &format!("{origin}/feeds/{repository_key}.xml?token={token}"),
        newest,
        &items,
    )
}

/// Render a bounded RSS 2.0 document.
#[must_use]
pub fn render_rss(title: &str, link: &str, description: &str, items: &[AtomItem]) -> String {
    let mut output = String::with_capacity(512 + items.len() * 512);
    output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    output.push_str("<rss version=\"2.0\"><channel><title>");
    output.push_str(&escape(title));
    output.push_str("</title><link>");
    output.push_str(&escape(link));
    output.push_str("</link><description>");
    output.push_str(&escape(description));
    output.push_str("</description>");
    for item in items {
        output.push_str("<item><title>");
        output.push_str(&escape(&item.title));
        output.push_str("</title><link>");
        output.push_str(&escape(&item.link));
        output.push_str("</link><guid isPermaLink=\"false\">");
        output.push_str(&escape(&item.id));
        output.push_str("</guid><pubDate>");
        output.push_str(&rfc2822(item.updated_ms));
        output.push_str("</pubDate><description>");
        output.push_str(&escape(&item.summary));
        output.push_str("</description></item>");
    }
    output.push_str("</channel></rss>\n");
    output
}

/// RFC 2822 UTC timestamp for an epoch millisecond value.
#[must_use]
pub fn rfc2822(ms: u64) -> String {
    const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let seconds = ms / 1_000;
    let days = (seconds / 86_400) as i64;
    let day_seconds = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let weekday = DAYS
        .get(usize::try_from((days + 4).rem_euclid(7)).unwrap_or(0))
        .copied()
        .unwrap_or("");
    let hour = day_seconds / 3_600;
    let minute = (day_seconds % 3_600) / 60;
    let second = day_seconds % 60;
    format!(
        "{weekday}, {day:02} {} {year:04} {hour:02}:{minute:02}:{second:02} +0000",
        MONTHS
            .get(usize::try_from(month.saturating_sub(1)).unwrap_or(0))
            .copied()
            .unwrap_or("")
    )
}

/// Render an OPML 2.0 subscription list for repository feeds.
#[must_use]
pub fn render_opml(title: &str, feeds: &[(&str, &str)]) -> String {
    let mut output = String::with_capacity(256 + feeds.len() * 256);
    output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    output.push_str("<opml version=\"2.0\"><head><title>");
    output.push_str(&escape(title));
    output.push_str("</title></head><body>\n");
    for (label, url) in feeds {
        output.push_str("<outline type=\"rss\" text=\"");
        output.push_str(&escape(label));
        output.push_str("\" title=\"");
        output.push_str(&escape(label));
        output.push_str("\" xmlUrl=\"");
        output.push_str(&escape(url));
        output.push_str("\"/>\n");
    }
    output.push_str("</body></opml>\n");
    output
}

/// One Atom entry for a repository or the feed index.
#[derive(Debug)]
pub struct AtomItem {
    pub id: String,
    pub title: String,
    pub link: String,
    pub updated_ms: u64,
    pub summary: String,
}

// r[impl kiln.web.feeds.publish]
/// Render a bounded Atom 1.0 document.
#[must_use]
pub fn render_atom(
    feed_id: &str,
    title: &str,
    self_link: &str,
    updated_ms: u64,
    items: &[AtomItem],
) -> String {
    let mut output = String::with_capacity(512 + items.len() * 512);
    output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    output.push_str("<feed xmlns=\"http://www.w3.org/2005/Atom\">\n");
    output.push_str("<id>");
    output.push_str(&escape(feed_id));
    output.push_str("</id>\n<title type=\"text\">");
    output.push_str(&escape(title));
    output.push_str("</title>\n<updated>");
    output.push_str(&rfc3339(updated_ms));
    output.push_str("</updated>\n<link rel=\"self\" href=\"");
    output.push_str(&escape(self_link));
    output.push_str("\"/>\n<author><name>Kiln</name></author>\n");
    for item in items {
        output.push_str("<entry><id>");
        output.push_str(&escape(&item.id));
        output.push_str("</id><title type=\"text\">");
        output.push_str(&escape(&item.title));
        output.push_str("</title><link rel=\"alternate\" href=\"");
        output.push_str(&escape(&item.link));
        output.push_str("\"/><updated>");
        output.push_str(&rfc3339(item.updated_ms));
        output.push_str("</updated><summary type=\"text\">");
        output.push_str(&escape(&item.summary));
        output.push_str("</summary></entry>\n");
    }
    output.push_str("</feed>\n");
    output
}

fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

/// RFC 3339 UTC timestamp for a non-negative epoch millisecond value.
#[must_use]
pub fn rfc3339(ms: u64) -> String {
    let seconds = ms / 1_000;
    let millis = ms % 1_000;
    let days = (seconds / 86_400) as i64;
    let day_seconds = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = day_seconds / 3_600;
    let minute = (day_seconds % 3_600) / 60;
    let second = day_seconds % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    if month <= 2 {
        year += 1;
    }
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RSS: &str = r#"<?xml version="1.0"?><rss version="2.0"><channel><title>Repo &amp; CI</title><item><title>Release 1.0</title><link>https://example.invalid/r1</link><guid>tag:r1</guid><pubDate>Wed, 02 Oct 2024 13:00:00 GMT</pubDate><description>First release</description></item><item><title>Second</title><link>https://example.invalid/r2</link><description>Second release</description></item></channel></rss>"#;
    const ATOM: &str = r#"<?xml version="1.0" encoding="utf-8"?><feed xmlns="http://www.w3.org/2005/Atom"><title>Repo feed</title><entry><title>Commit one</title><id>urn:commit:one</id><updated>2024-10-02T13:00:00Z</updated><link rel="alternate" href="https://example.invalid/c1"/><summary>Change one</summary></entry></feed>"#;

    #[test]
    fn rss_and_atom_parse_with_bounded_text() {
        let rss = parse(RSS.as_bytes(), 50).unwrap_or_else(|_| unreachable!());
        assert_eq!(rss.title.as_deref(), Some("Repo & CI"));
        assert_eq!(rss.entries.len(), 2);
        assert_eq!(
            rss.entries.first().unwrap_or_else(|| unreachable!()).title,
            "Release 1.0"
        );
        assert_eq!(
            rss.entries.first().unwrap_or_else(|| unreachable!()).id,
            "tag:r1"
        );
        assert_eq!(
            rss.entries
                .first()
                .unwrap_or_else(|| unreachable!())
                .link
                .as_deref(),
            Some("https://example.invalid/r1")
        );
        assert_eq!(
            rss.entries.get(1).unwrap_or_else(|| unreachable!()).id,
            "https://example.invalid/r2"
        );
        let atom = parse(ATOM.as_bytes(), 50).unwrap_or_else(|_| unreachable!());
        assert_eq!(atom.title.as_deref(), Some("Repo feed"));
        assert_eq!(
            atom.entries
                .first()
                .unwrap_or_else(|| unreachable!())
                .link
                .as_deref(),
            Some("https://example.invalid/c1")
        );
        assert_eq!(
            atom.entries
                .first()
                .unwrap_or_else(|| unreachable!())
                .published
                .as_deref(),
            Some("2024-10-02T13:00:00Z")
        );
    }

    #[test]
    fn rss_one_rdf_feeds_parse() {
        let rdf = r#"<?xml version="1.0"?><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns="http://purl.org/rss/1.0/" xmlns:dc="http://purl.org/dc/elements/1.1/"><channel><title>RDF feed</title></channel><item><title>RDF one</title><link>https://example.invalid/rdf</link><dc:date>2024-10-02T13:00:00Z</dc:date><description>RDF summary</description></item></rdf:RDF>"#;
        let parsed = parse(rdf.as_bytes(), 10).unwrap_or_else(|_| unreachable!());
        assert_eq!(parsed.title.as_deref(), Some("RDF feed"));
        assert_eq!(parsed.entries.len(), 1);
        let entry = parsed
            .entries
            .first()
            .unwrap_or_else(|| unreachable!("fixture entry"));
        assert_eq!(entry.title, "RDF one");
        assert_eq!(entry.published.as_deref(), Some("2024-10-02T13:00:00Z"));
    }

    #[test]
    // r[verify kiln.web.feeds.security.scenario.malformed]
    fn parser_rejects_doctype_malformed_xml_and_unknown_roots() {
        assert!(parse(b"<!DOCTYPE x [<!ENTITY y \"z\">]><rss/>", 10).is_err());
        assert!(parse(b"<rss>", 10).is_err());
        assert!(parse(b"<html/>", 10).is_err());
        assert!(parse(b"", 10).is_err());
    }

    #[test]
    fn parser_limits_entries_and_sanitizes_links() {
        let many = format!(
            "<rss><channel>{}</channel></rss>",
            (0..10)
                .map(|i| format!("<item><title>{i}</title><link>javascript:alert(1)</link></item>"))
                .collect::<String>()
        );
        let parsed = parse(many.as_bytes(), 3).unwrap_or_else(|_| unreachable!());
        assert_eq!(parsed.entries.len(), 3);
        assert!(parsed.entries.iter().all(|entry| entry.link.is_none()));
        let duplicates = br#"<rss><channel><item><title>one</title><guid>same</guid></item><item><title>two</title><guid>same</guid></item></channel></rss>"#;
        let parsed = parse(duplicates, 10).unwrap_or_else(|_| unreachable!());
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(
            parsed.entries.first().map(|entry| entry.title.as_str()),
            Some("one")
        );
    }

    #[test]
    // r[verify kiln.web.feeds.security.scenario.private]
    fn url_admission_rejects_public_http_and_private_addresses() {
        assert!(admit_url("http://example.invalid/feed", false).is_err());
        assert!(admit_url("https://user@example.invalid/feed", false).is_err());
        assert!(admit_url("https://127.0.0.1/feed", false).is_err());
        assert!(admit_url("https://[::1]/feed", false).is_err());
        assert!(admit_url("https://10.0.0.1/feed", false).is_err());
        assert!(admit_url("https://example.invalid:8443/feed", false).is_err());
        assert!(admit_url("https://example.invalid/feed#x", false).is_err());
        assert!(admit_url("http://127.0.0.1:8080/feed", true).is_ok());
        assert!(admit_url("https://example.invalid/feed", false).is_ok());
    }

    fn options<'a>(
        allow_private: bool,
        etag: Option<&'a str>,
        last_modified: Option<&'a str>,
    ) -> FetchOptions<'a> {
        FetchOptions {
            max_bytes: 65_536,
            max_entries: 10,
            max_redirects: 1,
            timeout_seconds: 5,
            allow_private,
            etag,
            last_modified,
        }
    }

    fn serve(
        responses: Vec<(&'static str, &'static str, &'static str)>,
    ) -> (u16, std::thread::JoinHandle<()>) {
        use std::io::{Read, Write};
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").unwrap_or_else(|_| unreachable!());
        let port = listener
            .local_addr()
            .unwrap_or_else(|_| unreachable!())
            .port();
        let handle = std::thread::spawn(move || {
            for (status, headers, body) in responses {
                let (mut stream, _) = listener.accept().unwrap_or_else(|_| unreachable!());
                let mut request = [0_u8; 4096];
                let _ = stream.read(&mut request);
                let response = format!(
                    "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        (port, handle)
    }

    #[test]
    // r[verify kiln.web.feeds.security.scenario.redirect]
    fn bounded_fetch_reads_local_feed_and_follows_one_redirect() {
        let (port, handle) = serve(vec![
            ("302 Found", "Location: /feed.xml\r\n", ""),
            ("200 OK", "", RSS),
        ]);
        let fetched = fetch(
            &format!("http://127.0.0.1:{port}/start"),
            options(true, None, None),
        )
        .unwrap_or_else(|_| unreachable!());
        let FetchOutcome::Modified { feed, .. } = fetched else {
            unreachable!("redirected feed must be modified")
        };
        assert_eq!(feed.entries.len(), 2);
        handle.join().unwrap_or_else(|_| unreachable!());
    }

    #[test]
    fn fetch_refuses_private_destinations_without_the_explicit_exception() {
        assert!(fetch("https://127.0.0.1:9/feed.xml", options(false, None, None)).is_err());
        assert!(fetch("http://127.0.0.1:9/feed.xml", options(false, None, None)).is_err());
        assert!(fetch("https://10.0.0.1/feed.xml", options(false, None, None)).is_err());
    }

    #[test]
    fn conditional_fetch_honours_not_modified_and_requires_a_validator() {
        let (port, handle) = serve(vec![("304 Not Modified", "ETag: \"v2\"\r\n", "")]);
        let fetched = fetch(
            &format!("http://127.0.0.1:{port}/feed.xml"),
            options(true, Some("\"v1\""), None),
        )
        .unwrap_or_else(|_| unreachable!());
        assert!(matches!(
            fetched,
            FetchOutcome::NotModified { etag: Some(ref value), .. } if value == "\"v2\""
        ));
        handle.join().unwrap_or_else(|_| unreachable!());

        let (port, handle) = serve(vec![("304 Not Modified", "", "")]);
        assert!(
            fetch(
                &format!("http://127.0.0.1:{port}/feed.xml"),
                FetchOptions {
                    max_redirects: 0,
                    ..options(true, None, None)
                },
            )
            .is_err()
        );
        handle.join().unwrap_or_else(|_| unreachable!());
    }

    #[test]
    // r[verify kiln.web.feeds.bounds]
    fn fetch_enforces_the_body_bound() {
        let (port, handle) = serve(vec![("200 OK", "", RSS)]);
        assert!(
            fetch(
                &format!("http://127.0.0.1:{port}/feed.xml"),
                FetchOptions {
                    max_bytes: 16,
                    ..options(true, None, None)
                },
            )
            .is_err()
        );
        handle.join().unwrap_or_else(|_| unreachable!());
    }

    #[test]
    fn due_selection_is_oldest_first_and_bounded() {
        let subscription = |id: &str, fetched| FeedSubscription {
            id: id.into(),
            url: format!("https://example.invalid/{id}.xml"),
            label: id.into(),
            title: None,
            fetched_at_ms: fetched,
            last_success_ms: fetched,
            error: None,
            etag: None,
            last_modified: None,
            failures: 0,
            entries: Vec::new(),
        };
        let subscriptions = vec![
            subscription("never", None),
            subscription("newer", Some(50)),
            subscription("oldest", Some(10)),
        ];
        let due = due_subscriptions(subscriptions.iter(), 100, 60, 2);
        assert_eq!(due.len(), 2);
        assert_eq!(due.first().map(|feed| feed.id.as_str()), Some("never"));
        assert_eq!(due.get(1).map(|feed| feed.id.as_str()), Some("oldest"));
        assert!(due_subscriptions(subscriptions.iter(), 100, 0, 2).is_empty());
    }

    #[test]
    fn failure_backoff_grows_and_caps() {
        assert_eq!(failure_backoff_ms(0, 10), 0);
        assert_eq!(failure_backoff_ms(60_000, 0), 60_000);
        assert_eq!(failure_backoff_ms(60_000, 1), 120_000);
        assert_eq!(failure_backoff_ms(60_000, 6), 3_840_000);
        assert_eq!(failure_backoff_ms(60_000, 100), 3_840_000);
        assert_eq!(failure_backoff_ms(86_400_000, 6), 86_400_000);
    }

    #[test]
    fn due_intervals_are_bounded_and_monotonic() {
        assert!(!is_due(1_000, Some(0), 0));
        assert!(is_due(60_000, None, 60_000));
        assert!(!is_due(59_999, Some(0), 60_000));
        assert!(is_due(60_000, Some(0), 60_000));
        assert!(!is_due(10, Some(100), 60_000));
    }

    #[test]
    fn opml_import_extracts_bounded_unique_outlines() {
        let xml = br#"<?xml version="1.0"?><opml version="2.0"><body>
            <outline text="One" title="One title" xmlUrl="https://example.invalid/one.xml"/>
            <outline text="Duplicate" xmlUrl="https://example.invalid/one.xml"/>
            <outline text="Folder"><outline text="Nested" xmlUrl="https://example.invalid/nested.xml"/></outline>
            <outline text="No URL"/>
        </body></opml>"#;
        let outlines = parse_opml(xml, 10).unwrap_or_else(|_| unreachable!());
        assert_eq!(outlines.len(), 2);
        assert_eq!(
            outlines.first().map(|outline| outline.0.as_str()),
            Some("https://example.invalid/one.xml")
        );
        assert_eq!(
            outlines.first().and_then(|outline| outline.1.as_deref()),
            Some("One")
        );
        assert_eq!(
            outlines.get(1).and_then(|outline| outline.1.as_deref()),
            Some("Nested")
        );
        assert!(parse_opml(xml, 1).unwrap_or_else(|_| unreachable!()).len() == 1);
        assert!(parse_opml(b"<!DOCTYPE x><opml/>", 10).is_err());
        assert!(parse_opml(b"<rss/>", 10).is_err());
        assert!(parse_opml(b"<opml><body/></opml>", 10).is_err());
    }

    #[test]
    fn rss_publication_renders_channel_and_dates() {
        let xml = render_rss(
            "Repo & CI",
            "https://localhost/feeds/x.rss",
            "Retained runs",
            &[AtomItem {
                id: "urn:run:1".into(),
                title: "Run <one>".into(),
                link: "https://localhost/?page=runs&run=1".into(),
                updated_ms: 1_709_385_000_000,
                summary: "A & B".into(),
            }],
        );
        assert!(xml.starts_with("<?xml"));
        assert!(xml.contains("<rss version=\"2.0\">"));
        assert!(xml.contains("<title>Repo &amp; CI</title>"));
        assert!(xml.contains("<pubDate>Sat, 02 Mar 2024 13:10:00 +0000</pubDate>"));
        assert!(xml.contains("https://localhost/?page=runs&amp;run=1"));
        assert!(xml.contains("A &amp; B"));
    }

    #[test]
    fn opml_export_escapes_labels_and_urls() {
        let xml = render_opml(
            "Kiln & repositories",
            &[("A & B", "https://localhost/feeds/abc.xml?token=def&x=1")],
        );
        assert!(xml.starts_with("<?xml"));
        assert!(xml.contains("<opml version=\"2.0\">"));
        assert!(xml.contains("text=\"A &amp; B\""));
        assert!(xml.contains("xmlUrl=\"https://localhost/feeds/abc.xml?token=def&amp;x=1\""));
    }

    // r[verify kiln.web.feeds.publish.scenario.rendered]
    #[test]
    fn repository_atom_filters_orders_limits_and_escapes_runs() {
        let mut runs = vec![
            FeedRun {
                id: "run-old".into(),
                project_id: "a".into(),
                project_label: "A & B".into(),
                ordinal: 1,
                updated_ms: 1_000,
                outcome: "Success".into(),
                revision: "a".repeat(40),
            },
            FeedRun {
                id: "run-new".into(),
                project_id: "b".into(),
                project_label: "Other".into(),
                ordinal: 2,
                updated_ms: 2_000,
                outcome: "Failed".into(),
                revision: "b".repeat(40),
            },
            FeedRun {
                id: "run-third".into(),
                project_id: "a".into(),
                project_label: "A & B".into(),
                ordinal: 3,
                updated_ms: 3_000,
                outcome: "Success".into(),
                revision: "c".repeat(40),
            },
        ];
        let xml = repository_atom(
            "https://localhost:9443",
            &"d".repeat(64),
            "Repository runs",
            "token",
            &mut runs,
            2,
        );
        assert!(!xml.contains("run-old"));
        assert!(xml.contains("run-third"));
        assert!(xml.contains("run-new"));
        assert!(
            xml.find("run-third").unwrap_or_default() < xml.find("run-new").unwrap_or_default()
        );
        assert!(xml.contains("A &amp; B"));
        assert!(xml.contains("https://localhost:9443/?page=runs&amp;run=run-new"));
    }

    #[test]
    fn timestamp_matches_known_utc_values() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(rfc3339(1_709_385_000_000), "2024-03-02T13:10:00.000Z");
    }

    #[test]
    fn atom_render_escapes_text_and_links() {
        let xml = render_atom(
            "urn:kiln:feed",
            "Repo & <CI>",
            "https://localhost/feeds/x.xml?token=abc",
            0,
            &[AtomItem {
                id: "urn:run:1".into(),
                title: "Run <one>".into(),
                link: "https://localhost/?page=runs&run=1".into(),
                updated_ms: 0,
                summary: "A & B".into(),
            }],
        );
        assert!(xml.contains("Repo &amp; &lt;CI&gt;"));
        assert!(xml.contains("https://localhost/?page=runs&amp;run=1"));
        assert!(xml.contains("A &amp; B"));
        assert!(xml.starts_with("<?xml"));
    }
}
