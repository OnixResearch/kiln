//! Swarm news claim coordination over renews/iroh, pure rules.
//!
//! Agents coordinate exclusive work by posting `CLAIM <repository> <scope>` and
//! later `RELEASE <same scope>` with `References` set to the claim's message id.
//! Articles are untrusted data: only the subject verb, the two scope fields and
//! `References` are parsed. Bodies are never interpreted.
//!
//! This module formats and parses claims, enforces body redaction, and decides
//! overlap. It performs no I/O and holds no credentials.

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::fleet::scope::{FleetScope, overlaps, parse_scope};

pub const MAX_REPOSITORY_BYTES: usize = 128;
pub const MAX_SCOPE_BYTES: usize = 128;
pub const MAX_SUBJECT_BYTES: usize = 256;
pub const MAX_BODY_BYTES: usize = 2048;
pub const MAX_MESSAGE_ID_BYTES: usize = 256;
pub const MAX_REFERENCES: usize = 16;
pub const STALE_CLAIM_MS: u64 = 24 * 60 * 60 * 1000;
pub const MAX_TOKEN_RUN: usize = 32;
/// Characters of a holder session id rendered in a fleet signature line.
pub const SIGNATURE_SESSION_CHARS: usize = 12;

/// Parsed claim scope shared by `CLAIM` and `RELEASE` subjects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimScope {
    pub repository: String,
    pub scope: String,
}

/// Recognized claim article kinds. Everything else is untrusted noise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimArticle {
    Claim(ClaimScope),
    Release(ClaimScope),
    Other,
}

/// One observed article, reduced to the fields the overlap rule uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArticleView {
    pub message_id: String,
    pub from: String,
    pub subject: String,
    pub date_unix_ms: u64,
    pub references: Vec<String>,
}

/// One attempt as a swarm mirror reconcile pass sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptSnapshot {
    pub identity: String,
    pub fence: u64,
    pub terminal: bool,
    pub repository: String,
    pub scope: String,
    pub base_revision: String,
    pub lease_minutes: u32,
}

/// Overlap decision for a prospective claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlapDecision {
    Allowed,
    Deferred {
        message_id: String,
        author: String,
        age_ms: u64,
    },
}

// r[impl kiln.implementation.swarm.claims]
/// Parse a claim subject. Unknown verbs, malformed scopes and non-ASCII fields
/// return [`ClaimArticle::Other`]; they never authorize or defer anything.
#[must_use]
pub fn parse_subject(subject: &str) -> ClaimArticle {
    if subject.len() > MAX_SUBJECT_BYTES {
        return ClaimArticle::Other;
    }
    let (verb, rest) = match subject.split_once(' ') {
        Some(parts) => parts,
        None => return ClaimArticle::Other,
    };
    let (repository, scope) = match rest.split_once(' ') {
        Some((repository, scope)) => (repository.trim(), scope.trim()),
        None => (rest.trim(), ""),
    };
    if !admit_field(repository, MAX_REPOSITORY_BYTES) || !admit_field(scope, MAX_SCOPE_BYTES) {
        return ClaimArticle::Other;
    }
    let parsed = ClaimScope {
        repository: repository.to_string(),
        scope: scope.to_string(),
    };
    match verb {
        "CLAIM" => ClaimArticle::Claim(parsed),
        "RELEASE" => ClaimArticle::Release(parsed),
        _ => ClaimArticle::Other,
    }
}

/// Build one `CLAIM <repository> <scope>` subject.
///
/// # Errors
/// Returns a bounded diagnostic when either field is empty or invalid.
pub fn claim_subject(repository: &str, scope: &str) -> Result<String, String> {
    subject("CLAIM", repository, scope)
}

/// Build one `RELEASE <repository> <scope>` subject.
///
/// # Errors
/// Returns a bounded diagnostic when either field is empty or invalid.
pub fn release_subject(repository: &str, scope: &str) -> Result<String, String> {
    subject("RELEASE", repository, scope)
}

fn subject(verb: &str, repository: &str, scope: &str) -> Result<String, String> {
    if !admit_field(repository, MAX_REPOSITORY_BYTES) || !admit_field(scope, MAX_SCOPE_BYTES) {
        return Err("swarm claim scope is invalid".to_string());
    }
    let value = format!("{verb} {repository} {scope}");
    if value.len() > MAX_SUBJECT_BYTES {
        return Err("swarm claim subject exceeds its bound".to_string());
    }
    Ok(value)
}

/// Facts that may appear in a claim body. Nothing else is rendered.
#[derive(Debug, Clone)]
pub struct ClaimBodyFacts<'a> {
    pub scope: &'a ClaimScope,
    pub work: &'a str,
    pub attempt: &'a str,
    pub base_revision: &'a str,
    pub lease_minutes: u32,
    pub host: &'a str,
}

/// Render a redaction-checked claim body with the required signature line.
///
/// # Errors
/// Returns a diagnostic when a fact is invalid or the rendered body would carry
/// secret-shaped material.
pub fn format_claim_body(facts: &ClaimBodyFacts<'_>) -> Result<String, String> {
    if facts.lease_minutes == 0 {
        return Err("swarm claim lease must be positive".to_string());
    }
    for field in [facts.work, facts.attempt, facts.base_revision, facts.host] {
        if !admit_field(field, MAX_SCOPE_BYTES) {
            return Err("swarm claim fact is invalid".to_string());
        }
    }
    let body = format!(
        "Claiming {} {} for Kiln attempt {}.\nwork: {}\nbase revision: {}\nexpected lease: {} minutes\n\n— kiln on {}, attempt {}\n",
        facts.scope.repository,
        facts.scope.scope,
        facts.attempt,
        facts.work,
        facts.base_revision,
        facts.lease_minutes,
        facts.host,
        facts.attempt,
    );
    admit_body(&body)?;
    Ok(body)
}

/// Render a redaction-checked release body with the required signature line.
///
/// # Errors
/// Returns a diagnostic when a fact is invalid or the rendered body would carry
/// secret-shaped material.
pub fn format_release_body(
    scope: &ClaimScope,
    attempt: &str,
    reason: &str,
    host: &str,
) -> Result<String, String> {
    for field in [attempt, reason, host] {
        if !admit_field(field, MAX_SCOPE_BYTES) {
            return Err("swarm release fact is invalid".to_string());
        }
    }
    let body = format!(
        "Releasing {} {} after Kiln attempt {}: {}\n\n— kiln on {}, attempt {}\n",
        scope.repository, scope.scope, attempt, reason, host, attempt,
    );
    admit_body(&body)?;
    Ok(body)
}

/// Admit a body for publication. Rejects oversized bodies, control characters,
/// secret references and token-shaped runs.
///
/// # Errors
/// Returns a bounded diagnostic naming the refused class.
pub fn admit_body(body: &str) -> Result<(), String> {
    if body.is_empty() || body.len() > MAX_BODY_BYTES {
        return Err("swarm body exceeds its bound".to_string());
    }
    if body
        .bytes()
        .any(|byte| byte.is_ascii_control() && byte != b'\n' && byte != b'\t')
    {
        return Err("swarm body contains a control character".to_string());
    }
    for marker in [
        "SECRETSPEC_",
        "/run/secrets",
        "ghp_",
        "github_pat_",
        "rad_",
        "Bearer ",
    ] {
        if body.contains(marker) {
            return Err("swarm body contains a secret reference".to_string());
        }
    }
    for token in body.split(|character: char| character.is_whitespace()) {
        if token.len() >= MAX_TOKEN_RUN
            && token
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_+/=".contains(&byte))
        {
            return Err("swarm body contains a token-shaped value".to_string());
        }
    }
    Ok(())
}

// r[impl kiln.implementation.swarm.overlap]
/// Decide whether a prospective claim overlaps an open claim by another author.
///
/// A claim is open until a `RELEASE` article lists its message id in
/// `References`. Claims older than [`STALE_CLAIM_MS`] do not defer. Scopes
/// overlap under fleet scope containment ([`crate::fleet::overlaps`]); a scope
/// that does not parse as a fleet scope falls back to exact equality. Only the
/// subject scope and references are inspected; bodies are ignored.
#[must_use]
pub fn evaluate_overlap(
    articles: &[ArticleView],
    author_from: &str,
    scope: &ClaimScope,
    now_ms: u64,
) -> OverlapDecision {
    let prospective = parse_scope(&scope.repository, &scope.scope).ok();
    let author = from_address(author_from);
    let mut released: BTreeSet<&str> = BTreeSet::new();
    for article in articles {
        if matches!(parse_subject(&article.subject), ClaimArticle::Release(_)) {
            for reference in &article.references {
                released.insert(reference.as_str());
            }
        }
    }
    let mut deferred: Option<(&ArticleView, u64)> = None;
    for article in articles {
        let ClaimArticle::Claim(observed) = parse_subject(&article.subject) else {
            continue;
        };
        if !scopes_overlap(&observed, scope, prospective.as_ref())
            || from_address(&article.from) == author
            || released.contains(article.message_id.as_str())
        {
            continue;
        }
        let age = now_ms.saturating_sub(article.date_unix_ms);
        if age > STALE_CLAIM_MS {
            continue;
        }
        if deferred.is_none_or(|(current, _)| article.date_unix_ms > current.date_unix_ms) {
            deferred = Some((article, age));
        }
    }
    match deferred {
        Some((article, age)) => OverlapDecision::Deferred {
            message_id: article.message_id.clone(),
            author: article.from.clone(),
            age_ms: age,
        },
        None => OverlapDecision::Allowed,
    }
}

/// Observed and prospective scopes overlap under fleet containment when both
/// parse as fleet scopes, and by exact equality otherwise.
fn scopes_overlap(
    observed: &ClaimScope,
    scope: &ClaimScope,
    prospective: Option<&FleetScope>,
) -> bool {
    match (
        prospective,
        parse_scope(&observed.repository, &observed.scope).ok(),
    ) {
        (Some(prospective), Some(observed)) => overlaps(prospective, &observed),
        _ => observed == scope,
    }
}

/// Who signs a fleet article: the coordinating Kiln and the claim holder.
#[derive(Debug, Clone, Copy)]
pub struct FleetSignature<'a> {
    pub coordinator: &'a str,
    pub tool: &'a str,
    pub host: &'a str,
    pub session: &'a str,
}

/// Facts a fleet claim body may carry. Nothing else is rendered.
#[derive(Debug, Clone, Copy)]
pub struct FleetClaimBodyFacts<'a> {
    pub target: &'a str,
    pub path: &'a str,
    pub purpose: &'a str,
    pub fence: u64,
    pub lease_minutes: u64,
    pub signature: FleetSignature<'a>,
}

/// Facts a fleet release body may carry. Nothing else is rendered.
#[derive(Debug, Clone, Copy)]
pub struct FleetReleaseBodyFacts<'a> {
    pub target: &'a str,
    pub path: &'a str,
    pub reason: &'a str,
    pub fence: u64,
    pub signature: FleetSignature<'a>,
}

/// Render a redaction-checked fleet claim body with the signature line
/// `— kiln on <coordinator> for <tool> on <host>, session <session>`.
///
/// # Errors
/// Returns a diagnostic when a fact is invalid or the rendered body would carry
/// secret-shaped material.
pub fn format_fleet_claim_body(facts: &FleetClaimBodyFacts<'_>) -> Result<String, String> {
    if facts.lease_minutes == 0 {
        return Err("swarm claim lease must be positive".to_string());
    }
    admit_fleet_facts(facts.target, facts.path, facts.purpose, &facts.signature)?;
    let body = format!(
        "Claiming {} {} through the Kiln fleet.\npurpose: {}\nholder: {} on {}\nfence: {}\nexpected lease: {} minutes\n\n{}\n",
        facts.target,
        facts.path,
        facts.purpose,
        facts.signature.tool,
        facts.signature.host,
        facts.fence,
        facts.lease_minutes,
        fleet_signature_line(&facts.signature),
    );
    admit_body(&body)?;
    Ok(body)
}

/// Render a redaction-checked fleet release body with the fleet signature line.
///
/// # Errors
/// Returns a diagnostic when a fact is invalid or the rendered body would carry
/// secret-shaped material.
pub fn format_fleet_release_body(facts: &FleetReleaseBodyFacts<'_>) -> Result<String, String> {
    admit_fleet_facts(facts.target, facts.path, facts.reason, &facts.signature)?;
    let body = format!(
        "Releasing {} {} through the Kiln fleet: {}\nholder: {} on {}\nfence: {}\n\n{}\n",
        facts.target,
        facts.path,
        facts.reason,
        facts.signature.tool,
        facts.signature.host,
        facts.fence,
        fleet_signature_line(&facts.signature),
    );
    admit_body(&body)?;
    Ok(body)
}

fn admit_fleet_facts(
    target: &str,
    path: &str,
    text: &str,
    signature: &FleetSignature<'_>,
) -> Result<(), String> {
    if !admit_field(target, MAX_REPOSITORY_BYTES) || !admit_field(path, MAX_SCOPE_BYTES) {
        return Err("swarm claim scope is invalid".to_string());
    }
    for field in [
        signature.coordinator,
        signature.tool,
        signature.host,
        signature.session,
    ] {
        if !admit_field(field, MAX_SCOPE_BYTES)
            || !field.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err("swarm fleet signature fact is invalid".to_string());
        }
    }
    if text.trim().is_empty() || text.chars().any(char::is_control) {
        return Err("swarm fleet fact is empty or contains a control character".to_string());
    }
    Ok(())
}

// Deviation from the design's `session <session>`: agent session ids are often
// UUID-shaped, which `admit_body` refuses as token-shaped runs, so the
// signature names the session by its first `SIGNATURE_SESSION_CHARS`.
fn fleet_signature_line(signature: &FleetSignature<'_>) -> String {
    format!(
        "— kiln on {} for {} on {}, session {}",
        signature.coordinator,
        signature.tool,
        signature.host,
        signature
            .session
            .get(..SIGNATURE_SESSION_CHARS)
            .unwrap_or(signature.session),
    )
}

/// Extract the address inside angle brackets, or the trimmed value.
#[must_use]
pub fn from_address(from: &str) -> &str {
    match (from.find('<'), from.rfind('>')) {
        (Some(start), Some(end)) if end > start => {
            from.get(start + 1..end).unwrap_or_else(|| from.trim())
        }
        _ => from.trim(),
    }
}

/// Parse an RFC 2822 date to Unix milliseconds.
///
/// Supports the form emitted by renews: an optional weekday, `DD Mon YYYY
/// HH:MM:SS` and a numeric or symbolic zone.
#[must_use]
pub fn parse_rfc2822_ms(value: &str) -> Option<u64> {
    let mut tokens = value.split_whitespace();
    let mut first = tokens.next()?;
    if first.ends_with(',') {
        first = tokens.next()?;
    }
    let day: u64 = first.parse().ok()?;
    let month = month(tokens.next()?)?;
    let year: u64 = tokens.next()?.parse().ok()?;
    let time = tokens.next()?;
    let zone = tokens.next()?;
    if !(1970..=9999).contains(&year) || !(1..=31).contains(&day) {
        return None;
    }
    let mut parts = time.split(':');
    let hour: u64 = parts.next()?.parse().ok()?;
    let minute: u64 = parts.next()?.parse().ok()?;
    let second: u64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let days = days_since_epoch(year, month, day)?;
    let seconds = days
        .checked_mul(86_400)?
        .checked_add(hour * 3_600 + minute * 60 + second)?;
    let adjusted = i64::try_from(seconds)
        .ok()?
        .checked_sub(zone_offset_seconds(zone)?)?;
    u64::try_from(adjusted).ok()?.checked_mul(1_000)
}

fn zone_offset_seconds(zone: &str) -> Option<i64> {
    match zone {
        "GMT" | "UTC" | "UT" | "Z" => Some(0),
        _ => {
            let (sign, digits) = if let Some(digits) = zone.strip_prefix('+') {
                (1_i64, digits)
            } else if let Some(digits) = zone.strip_prefix('-') {
                (-1_i64, digits)
            } else {
                return None;
            };
            if digits.len() != 4 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            let (hours, minutes) = digits.split_at_checked(2)?;
            let hours: i64 = hours.parse().ok()?;
            let minutes: i64 = minutes.parse().ok()?;
            if hours > 23 || minutes > 59 {
                return None;
            }
            Some(sign * (hours * 3_600 + minutes * 60))
        }
    }
}

fn month(value: &str) -> Option<u64> {
    Some(match value {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return None,
    })
}

fn days_since_epoch(year: u64, month: u64, day: u64) -> Option<u64> {
    let month_days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap(year) => 29,
        2 => 28,
        _ => return None,
    };
    if day > month_days {
        return None;
    }
    let mut days = 0_u64;
    for year_index in 1970..year {
        days += if leap(year_index) { 366 } else { 365 };
    }
    for month_index in 1..month {
        days += match month_index {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            _ if leap(year) => 29,
            _ => 28,
        };
    }
    days.checked_add(day - 1)
}

const fn leap(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn admit_field(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.trim() == value
        && value.is_ascii()
        && !value.bytes().any(|byte| byte.is_ascii_control())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn scope() -> ClaimScope {
        ClaimScope {
            repository: "OnixResearch/kiln".to_string(),
            scope: "crates/kiln-core".to_string(),
        }
    }

    #[test]
    // r[verify kiln.implementation.swarm.claims.scenario.subject]
    fn claim_and_release_subjects_round_trip() {
        let claim = claim_subject("OnixResearch/kiln", "crates/kiln-core")
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(claim, "CLAIM OnixResearch/kiln crates/kiln-core");
        assert_eq!(parse_subject(&claim), ClaimArticle::Claim(scope()));
        let release = release_subject("OnixResearch/kiln", "crates/kiln-core")
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(parse_subject(&release), ClaimArticle::Release(scope()));
        assert_eq!(
            parse_subject("claim OnixResearch/kiln scope"),
            ClaimArticle::Other
        );
        assert_eq!(parse_subject("CLAIM"), ClaimArticle::Other);
        assert_eq!(parse_subject("CLAIM repo"), ClaimArticle::Other);
        assert_eq!(parse_subject("RENEW repo scope"), ClaimArticle::Other);
        assert_eq!(parse_subject("CLAIM  repo\t"), ClaimArticle::Other);
    }

    #[test]
    // r[verify kiln.implementation.swarm.claims.scenario.redaction]
    fn bodies_are_bounded_and_redacted() {
        let facts = ClaimBodyFacts {
            scope: &scope(),
            work: "kiln.implementation.work",
            attempt: "attempt-a",
            base_revision: "3a6047ef22ea",
            lease_minutes: 60,
            host: "aspen1",
        };
        let body = format_claim_body(&facts).unwrap_or_else(|_| unreachable!());
        assert!(body.contains("— kiln on aspen1, attempt attempt-a"));
        assert!(admit_body(&body).is_ok());
        assert!(admit_body("token SECRETSPEC_GITHUB").is_err());
        assert!(admit_body("path /run/secrets/github").is_err());
        assert!(admit_body("Authorization: Bearer abc").is_err());
        assert!(admit_body(&"a".repeat(MAX_BODY_BYTES + 1)).is_err());
        assert!(admit_body("run 0123456789abcdef0123456789abcdef").is_err());
        assert!(admit_body("ok short token").is_ok());
        assert!(
            format_release_body(&scope(), "attempt-a", "verified", "aspen1")
                .unwrap_or_else(|_| unreachable!())
                .contains("Releasing OnixResearch/kiln crates/kiln-core")
        );
        let mut invalid = facts.clone();
        invalid.lease_minutes = 0;
        assert!(format_claim_body(&invalid).is_err());
    }

    #[test]
    // r[verify kiln.implementation.swarm.overlap.scenario.defer]
    fn another_authors_open_claim_defers() {
        let articles = vec![ArticleView {
            message_id: "<claim@aspen1>".to_string(),
            from: "omp on aspen3 <aspen3@swarm.local>".to_string(),
            subject: "CLAIM OnixResearch/kiln crates/kiln-core".to_string(),
            date_unix_ms: 1_000_000,
            references: vec![],
        }];
        assert_eq!(
            evaluate_overlap(
                &articles,
                "kiln <kiln-aspen1@swarm.local>",
                &scope(),
                1_000_500
            ),
            OverlapDecision::Deferred {
                message_id: "<claim@aspen1>".to_string(),
                author: "omp on aspen3 <aspen3@swarm.local>".to_string(),
                age_ms: 500,
            }
        );
        assert_eq!(
            evaluate_overlap(&articles, "kiln <aspen3@swarm.local>", &scope(), 1_000_500),
            OverlapDecision::Allowed
        );
        let other_scope = ClaimScope {
            repository: "OnixResearch/kiln".to_string(),
            scope: "docs".to_string(),
        };
        assert_eq!(
            evaluate_overlap(
                &articles,
                "kiln <kiln-aspen1@swarm.local>",
                &other_scope,
                1_000_500
            ),
            OverlapDecision::Allowed
        );
    }

    fn claim_article(subject: &str) -> ArticleView {
        ArticleView {
            message_id: "<claim@aspen3>".to_string(),
            from: "omp on aspen3 <aspen3@swarm.local>".to_string(),
            subject: subject.to_string(),
            date_unix_ms: 1_000_000,
            references: vec![],
        }
    }

    #[test]
    // r[verify kiln.implementation.swarm.overlap] r[verify kiln.implementation.swarm.overlap.scenario.defer]
    fn containing_scopes_defer_and_disjoint_scopes_do_not() {
        let author = "kiln <kiln-aspen1@swarm.local>";
        for subject in [
            "CLAIM OnixResearch/kiln crates",
            "CLAIM OnixResearch/kiln .",
            "CLAIM onixresearch/KILN crates/kiln-core",
            "CLAIM OnixResearch/kiln crates/kiln-core/src",
        ] {
            assert_eq!(
                evaluate_overlap(&[claim_article(subject)], author, &scope(), 1_000_500),
                OverlapDecision::Deferred {
                    message_id: "<claim@aspen3>".to_string(),
                    author: "omp on aspen3 <aspen3@swarm.local>".to_string(),
                    age_ms: 500,
                },
                "{subject}"
            );
        }
        for subject in [
            "CLAIM OnixResearch/kiln crates/kiln-corex",
            "CLAIM OnixResearch/kiln docs",
            "CLAIM OnixResearch/other crates/kiln-core",
            "CLAIM aspen3 system",
        ] {
            assert_eq!(
                evaluate_overlap(&[claim_article(subject)], author, &scope(), 1_000_500),
                OverlapDecision::Allowed,
                "{subject}"
            );
        }
        // Scopes that are not fleet scopes fall back to exact equality.
        let legacy = ClaimScope {
            repository: "OnixResearch/kiln".to_string(),
            scope: "crates kiln-core".to_string(),
        };
        let legacy_claim = claim_article("CLAIM OnixResearch/kiln crates kiln-core");
        assert!(matches!(
            evaluate_overlap(&[legacy_claim.clone()], author, &legacy, 1_000_500),
            OverlapDecision::Deferred { .. }
        ));
        assert_eq!(
            evaluate_overlap(&[legacy_claim], author, &scope(), 1_000_500),
            OverlapDecision::Allowed
        );
        let parent_of_legacy = claim_article("CLAIM OnixResearch/kiln crates");
        assert_eq!(
            evaluate_overlap(&[parent_of_legacy], author, &legacy, 1_000_500),
            OverlapDecision::Allowed
        );
    }

    #[test]
    // r[verify kiln.implementation.swarm.overlap.scenario.release]
    fn released_and_stale_containing_claims_do_not_defer() {
        let author = "kiln <kiln-aspen1@swarm.local>";
        let claim = claim_article("CLAIM OnixResearch/kiln crates");
        let mut release = claim_article("RELEASE OnixResearch/kiln crates");
        release.message_id = "<release@aspen3>".to_string();
        release.references = vec!["<claim@aspen3>".to_string()];
        assert_eq!(
            evaluate_overlap(&[claim.clone(), release], author, &scope(), 1_000_500),
            OverlapDecision::Allowed
        );
        assert_eq!(
            evaluate_overlap(
                &[claim.clone()],
                author,
                &scope(),
                1_000_000 + STALE_CLAIM_MS + 1
            ),
            OverlapDecision::Allowed
        );
        assert_eq!(
            evaluate_overlap(&[claim], "kiln <aspen3@swarm.local>", &scope(), 1_000_500),
            OverlapDecision::Allowed
        );
    }

    #[test]
    // r[verify kiln.fleet.mirror]
    fn fleet_bodies_carry_the_signature_and_pass_redaction() {
        let signature = FleetSignature {
            coordinator: "britton-desktop",
            tool: "claude",
            host: "aspen3",
            session: "0b8d3f5e-6c1a-4c2e-9d7f-2f6e1b0a9c11",
        };
        let facts = FleetClaimBodyFacts {
            target: "onixresearch/kiln",
            path: "crates/kiln-core",
            purpose: "fleet core slice",
            fence: 7,
            lease_minutes: 240,
            signature,
        };
        let body = format_fleet_claim_body(&facts).unwrap_or_else(|_| unreachable!());
        assert!(body.contains("Claiming onixresearch/kiln crates/kiln-core"));
        assert!(
            body.contains("— kiln on britton-desktop for claude on aspen3, session 0b8d3f5e-6c1")
        );
        assert!(admit_body(&body).is_ok());
        let release = format_fleet_release_body(&FleetReleaseBodyFacts {
            target: "onixresearch/kiln",
            path: "crates/kiln-core",
            reason: "lease expired",
            fence: 7,
            signature,
        })
        .unwrap_or_else(|_| unreachable!());
        assert!(release.contains("Releasing onixresearch/kiln crates/kiln-core"));
        assert!(admit_body(&release).is_ok());

        let mut zero_lease = facts;
        zero_lease.lease_minutes = 0;
        assert!(format_fleet_claim_body(&zero_lease).is_err());
        let mut secret = facts;
        secret.purpose = "use ghp_ token";
        assert!(format_fleet_claim_body(&secret).is_err());
        let mut multiline = facts;
        multiline.purpose = "first\nsecond";
        assert!(format_fleet_claim_body(&multiline).is_err());
        let mut bad_signature = facts;
        bad_signature.signature.host = "aspen 3";
        assert!(format_fleet_claim_body(&bad_signature).is_err());
    }

    #[test]
    // r[verify kiln.implementation.swarm.overlap.scenario.release]
    fn released_and_stale_claims_do_not_defer() {
        let claim = ArticleView {
            message_id: "<claim@aspen1>".to_string(),
            from: "omp on aspen3 <aspen3@swarm.local>".to_string(),
            subject: "CLAIM OnixResearch/kiln crates/kiln-core".to_string(),
            date_unix_ms: 1_000_000,
            references: vec![],
        };
        let release = ArticleView {
            message_id: "<release@aspen1>".to_string(),
            from: "omp on aspen3 <aspen3@swarm.local>".to_string(),
            subject: "RELEASE OnixResearch/kiln crates/kiln-core".to_string(),
            date_unix_ms: 1_001_000,
            references: vec!["<claim@aspen1>".to_string()],
        };
        let author = "kiln <kiln-aspen1@swarm.local>";
        assert_eq!(
            evaluate_overlap(&[claim.clone(), release], author, &scope(), 1_002_000),
            OverlapDecision::Allowed
        );
        assert_eq!(
            evaluate_overlap(
                &[claim.clone()],
                author,
                &scope(),
                1_000_000 + STALE_CLAIM_MS + 1
            ),
            OverlapDecision::Allowed
        );
        let mut body_only = claim;
        body_only.subject = "LAUNCH something".to_string();
        assert_eq!(
            evaluate_overlap(&[body_only], author, &scope(), 1_000_500),
            OverlapDecision::Allowed
        );
    }

    #[test]
    // r[verify kiln.implementation.swarm.transport.scenario.date]
    fn rfc2822_dates_parse() {
        assert_eq!(
            parse_rfc2822_ms("Wed, 18 Nov 2026 23:01:00 +0000"),
            Some(1_795_042_860_000)
        );
        assert_eq!(
            parse_rfc2822_ms("18 Nov 2026 18:01:00 -0500"),
            Some(1_795_042_860_000)
        );
        assert_eq!(
            parse_rfc2822_ms("18 Nov 2026 23:01:00 GMT"),
            Some(1_795_042_860_000)
        );
        assert_eq!(
            parse_rfc2822_ms("18 Nov 2026 23:01:00 +0000").map(|_| ()),
            Some(())
        );
        assert!(parse_rfc2822_ms("31 Feb 2026 00:00:00 +0000").is_none());
        assert!(parse_rfc2822_ms("18 Foo 2026 00:00:00 +0000").is_none());
        assert!(parse_rfc2822_ms("18 Nov 2026 24:00:00 +0000").is_none());
        assert!(parse_rfc2822_ms("nonsense").is_none());
    }
}
