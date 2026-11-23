//! Fleet claim scopes and containment overlap.
//!
//! A scope is `<owner>/<repo> <path>` or `<machine> <path>`, the forms the
//! swarm skill defines. `.` names a whole repository and `system` a whole
//! machine. Repository names compare ASCII-case-insensitively and are stored
//! lowercase.

use alloc::format;
use alloc::string::{String, ToString};

use serde::{Deserialize, Serialize};

use super::refusal;
use crate::implementation::swarm::{MAX_REPOSITORY_BYTES, MAX_SCOPE_BYTES, MAX_SUBJECT_BYTES};
use crate::types::CoreResult;

/// Path text naming a whole repository.
pub const WHOLE_REPOSITORY_PATH: &str = ".";
/// Path text naming a whole machine.
pub const WHOLE_MACHINE_PATH: &str = "system";

/// What a claim scope is about.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FleetTarget {
    /// `owner/name`, stored lowercase.
    Repository(String),
    /// A machine label, `[a-z0-9][a-z0-9-]*`.
    Machine(String),
}

/// The part of a target a scope covers.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopePath {
    /// `.` for a repository or `system` for a machine.
    Whole,
    /// A normalized repository-relative path, a unit name or an absolute
    /// machine path.
    Path(String),
}

/// One parsed, normalized claim scope.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FleetScope {
    pub target: FleetTarget,
    pub path: ScopePath,
}

impl FleetScope {
    /// Canonical target text: the lowercase repository or the machine label.
    #[must_use]
    pub fn target_text(&self) -> &str {
        match &self.target {
            FleetTarget::Repository(value) | FleetTarget::Machine(value) => value,
        }
    }

    /// Canonical path text; `.` or `system` for a whole target.
    #[must_use]
    pub fn path_text(&self) -> &str {
        match (&self.path, &self.target) {
            (ScopePath::Path(path), _) => path,
            (ScopePath::Whole, FleetTarget::Repository(_)) => WHOLE_REPOSITORY_PATH,
            (ScopePath::Whole, FleetTarget::Machine(_)) => WHOLE_MACHINE_PATH,
        }
    }

    /// Canonical `<target> <path>` text.
    #[must_use]
    pub fn text(&self) -> String {
        format!("{} {}", self.target_text(), self.path_text())
    }

    /// Revalidate a scope that may have arrived through serde: it must be the
    /// exact normalized form [`parse_scope`] produces.
    ///
    /// # Errors
    /// Returns a `fleet_scope` diagnostic when the scope is not canonical.
    pub fn validate(&self) -> CoreResult<()> {
        if parse_scope(self.target_text(), self.path_text())? != *self {
            return Err(refusal(
                "fleet_scope",
                "scope",
                "fleet scope is not in its canonical form",
            ));
        }
        Ok(())
    }
}

/// Swarm claim verbs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimVerb {
    Claim,
    Release,
}

impl ClaimVerb {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Claim => "CLAIM",
            Self::Release => "RELEASE",
        }
    }
}

// r[impl kiln.fleet.scope]
/// Parse and normalize one claim scope.
///
/// # Errors
/// Refuses empty, oversized, non-ASCII or control-bearing fields, malformed
/// repositories and machine labels, absolute or `..` repository paths, and
/// scopes whose subject would exceed the swarm subject bound.
pub fn parse_scope(target: &str, path: &str) -> CoreResult<FleetScope> {
    if !scope_field(target, MAX_REPOSITORY_BYTES) {
        return Err(refusal(
            "fleet_scope_bound",
            "scope.target",
            "scope target is empty, oversized, non-ASCII or contains whitespace",
        ));
    }
    if !scope_field(path, MAX_SCOPE_BYTES) {
        return Err(refusal(
            "fleet_scope_bound",
            "scope.path",
            "scope path is empty, oversized, non-ASCII or contains whitespace",
        ));
    }
    let scope = if target.contains('/') {
        FleetScope {
            target: FleetTarget::Repository(repository(target)?),
            path: repository_path(path)?,
        }
    } else {
        FleetScope {
            target: FleetTarget::Machine(machine(target)?),
            path: machine_path(path)?,
        }
    };
    if scope_subject(ClaimVerb::Release, &scope).len() > MAX_SUBJECT_BYTES {
        return Err(refusal(
            "fleet_scope_bound",
            "scope",
            "scope subject exceeds the swarm subject bound",
        ));
    }
    Ok(scope)
}

/// Parse `<target> <path>` text, separated by exactly one space.
///
/// # Errors
/// Refuses text without exactly two fields, and every [`parse_scope`] refusal.
pub fn parse_scope_text(text: &str) -> CoreResult<FleetScope> {
    let Some((target, path)) = text.split_once(' ') else {
        return Err(refusal(
            "fleet_scope",
            "scope",
            "scope must be `<target> <path>`",
        ));
    };
    parse_scope(target, path)
}

/// Canonical `CLAIM|RELEASE <target> <path>` subject text.
#[must_use]
pub fn scope_subject(verb: ClaimVerb, scope: &FleetScope) -> String {
    format!(
        "{} {} {}",
        verb.as_str(),
        scope.target_text(),
        scope.path_text()
    )
}

/// Two scopes overlap when they name the same target and either is whole,
/// the paths are equal, or one contains the other at a `/` boundary. Units
/// and absolute machine paths never contain each other.
#[must_use]
pub fn overlaps(a: &FleetScope, b: &FleetScope) -> bool {
    contains_scope(a, b) || contains_scope(b, a)
}

/// Directional containment: `outer` covers `inner` when they name the same
/// target and either `outer` is whole, the paths are equal, or `outer` is a
/// `/`-boundary prefix of `inner`.
#[must_use]
pub fn contains_scope(outer: &FleetScope, inner: &FleetScope) -> bool {
    if outer.target != inner.target {
        return false;
    }
    match (&outer.path, &inner.path) {
        (ScopePath::Whole, _) => true,
        (ScopePath::Path(_), ScopePath::Whole) => false,
        (ScopePath::Path(left), ScopePath::Path(right)) => contains(left, right),
    }
}

/// `outer` equals `inner` or is a `/`-boundary prefix of it.
fn contains(outer: &str, inner: &str) -> bool {
    match inner.strip_prefix(outer) {
        Some("") => true,
        Some(rest) => outer.ends_with('/') || rest.starts_with('/'),
        None => false,
    }
}

fn scope_field(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum && value.bytes().all(|byte| byte.is_ascii_graphic())
}

fn repository(target: &str) -> CoreResult<String> {
    let valid = target.split_once('/').is_some_and(|(owner, name)| {
        repository_part(owner) && repository_part(name) && !name.contains('/')
    });
    if !valid {
        return Err(refusal(
            "fleet_scope_target",
            "scope.target",
            "repository must be `owner/name`",
        ));
    }
    Ok(target.to_ascii_lowercase())
}

fn repository_part(part: &str) -> bool {
    !part.is_empty()
        && part != "."
        && part != ".."
        && part
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
}

fn machine(target: &str) -> CoreResult<String> {
    let mut bytes = target.bytes();
    let valid = bytes
        .next()
        .is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    if !valid {
        return Err(refusal(
            "fleet_scope_target",
            "scope.target",
            "machine label must match [a-z0-9][a-z0-9-]*",
        ));
    }
    Ok(target.to_string())
}

// `system` is only the whole-machine word; inside a repository it is an
// ordinary path.
fn repository_path(path: &str) -> CoreResult<ScopePath> {
    if path == WHOLE_REPOSITORY_PATH {
        return Ok(ScopePath::Whole);
    }
    if path.starts_with('/') {
        return Err(refusal(
            "fleet_scope_path",
            "scope.path",
            "repository path must be relative",
        ));
    }
    let normalized = path
        .strip_prefix("./")
        .unwrap_or(path)
        .trim_end_matches('/');
    if normalized.is_empty() {
        return Ok(ScopePath::Whole);
    }
    if !segments_admitted(normalized) {
        return Err(refusal(
            "fleet_scope_path",
            "scope.path",
            "repository path has an empty, `.` or `..` segment",
        ));
    }
    Ok(ScopePath::Path(normalized.to_string()))
}

fn machine_path(path: &str) -> CoreResult<ScopePath> {
    if path == WHOLE_MACHINE_PATH {
        return Ok(ScopePath::Whole);
    }
    if let Some(relative) = path.strip_prefix('/') {
        let trimmed = relative.trim_end_matches('/');
        if trimmed.is_empty() {
            return Ok(ScopePath::Path("/".to_string()));
        }
        if !segments_admitted(trimmed) {
            return Err(refusal(
                "fleet_scope_path",
                "scope.path",
                "machine path has an empty, `.` or `..` segment",
            ));
        }
        return Ok(ScopePath::Path(format!("/{trimmed}")));
    }
    if path.contains('/') || path == "." || path == ".." {
        return Err(refusal(
            "fleet_scope_path",
            "scope.path",
            "machine path must be `system`, a unit name or an absolute path",
        ));
    }
    Ok(ScopePath::Path(path.to_string()))
}

fn segments_admitted(path: &str) -> bool {
    !path.contains('\\')
        && path
            .split('/')
            .all(|segment| !matches!(segment, "" | "." | ".."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(text: &str) -> FleetScope {
        parse_scope_text(text).unwrap_or_else(|_| unreachable!())
    }

    #[test]
    // r[verify kiln.fleet.scope] r[verify kiln.fleet.scope.scenario.containment]
    fn containment_and_case_overlap() {
        let claimed = scope("OnixResearch/kiln crates/kiln-core");
        for other in [
            "onixresearch/kiln crates",
            "OnixResearch/kiln .",
            "OnixResearch/kiln crates/kiln-core/src/fleet",
            "OnixResearch/kiln crates/kiln-core",
            "OnixResearch/kiln ./crates/kiln-core/",
        ] {
            assert!(overlaps(&claimed, &scope(other)), "{other}");
            assert!(overlaps(&scope(other), &claimed), "{other}");
        }
        for other in [
            "OnixResearch/kiln crates/kiln-corex",
            "OnixResearch/other crates/kiln-core",
            "aspen3 system",
            "OnixResearch/kiln docs",
        ] {
            assert!(!overlaps(&claimed, &scope(other)), "{other}");
            assert!(!overlaps(&scope(other), &claimed), "{other}");
        }
        // Directional containment: inner paths are contained, parent/whole scopes are not.
        assert!(contains_scope(
            &claimed,
            &scope("OnixResearch/kiln crates/kiln-core")
        ));
        assert!(contains_scope(
            &claimed,
            &scope("OnixResearch/kiln crates/kiln-core/src/fleet")
        ));
        assert!(contains_scope(&scope("OnixResearch/kiln ."), &claimed));
        assert!(!contains_scope(&claimed, &scope("OnixResearch/kiln .")));
        assert!(!contains_scope(
            &claimed,
            &scope("OnixResearch/kiln crates")
        ));
        assert!(!contains_scope(
            &claimed,
            &scope("OnixResearch/kiln crates/kiln-corex")
        ));
    }

    #[test]
    // r[verify kiln.fleet.scope]
    fn machine_scopes_contain_paths_but_not_units() {
        let whole = scope("aspen3 system");
        assert_eq!(whole.path, ScopePath::Whole);
        assert!(overlaps(&whole, &scope("aspen3 kiln.service")));
        assert!(overlaps(&whole, &scope("aspen3 /var/lib")));
        assert!(overlaps(
            &scope("aspen3 /var"),
            &scope("aspen3 /var/lib/kiln")
        ));
        assert!(overlaps(&scope("aspen3 /"), &scope("aspen3 /etc")));
        assert!(!overlaps(&scope("aspen3 /var"), &scope("aspen3 /variable")));
        assert!(!overlaps(
            &scope("aspen3 kiln"),
            &scope("aspen3 kiln.service")
        ));
        assert!(!overlaps(
            &scope("aspen3 /etc"),
            &scope("aspen3 kiln.service")
        ));
        assert!(!overlaps(&scope("aspen3 system"), &scope("aspen1 system")));
        assert_eq!(scope("aspen3 /var/lib/").path_text(), "/var/lib");
    }

    #[test]
    // r[verify kiln.fleet.scope]
    fn canonical_text_and_subjects() {
        let parsed = scope("OnixResearch/Kiln ./crates/");
        assert_eq!(
            parsed.target,
            FleetTarget::Repository("onixresearch/kiln".to_string())
        );
        assert_eq!(parsed.text(), "onixresearch/kiln crates");
        assert_eq!(
            scope_subject(ClaimVerb::Claim, &parsed),
            "CLAIM onixresearch/kiln crates"
        );
        assert_eq!(
            scope_subject(ClaimVerb::Release, &scope("OnixResearch/kiln .")),
            "RELEASE onixresearch/kiln ."
        );
        assert_eq!(scope("OnixResearch/kiln ./").path, ScopePath::Whole);
        assert_eq!(
            scope("OnixResearch/kiln system").path,
            ScopePath::Path("system".to_string())
        );
        assert!(parsed.validate().is_ok());
        let uncanonical = FleetScope {
            target: FleetTarget::Repository("OnixResearch/kiln".to_string()),
            path: ScopePath::Path("crates/".to_string()),
        };
        assert!(uncanonical.validate().is_err());
    }

    #[test]
    // r[verify kiln.fleet.scope.scenario.malformed]
    fn malformed_scopes_are_refused() {
        let long = "a".repeat(MAX_SCOPE_BYTES + 1);
        let long_repository = format!("o/{}", "r".repeat(MAX_REPOSITORY_BYTES));
        let cases: [(&str, &str); 17] = [
            ("OnixResearch/kiln", "crates/../secrets"),
            ("OnixResearch/kiln", ".."),
            ("OnixResearch/kiln", "/etc"),
            ("OnixResearch/kiln", "crates\u{7}"),
            ("OnixResearch/kiln", ""),
            ("OnixResearch/kiln", &long),
            (&long_repository, "crates"),
            ("OnixResearch/kiln", "crates//core"),
            ("OnixResearch/kiln", "crates/./core"),
            ("OnixResearch/kiln", "crates core"),
            ("OnixResearch/kiln/extra", "crates"),
            ("/kiln", "crates"),
            ("Aspen3", "system"),
            ("-aspen", "system"),
            ("aspen3", "var/lib"),
            ("aspen3", "/var/../etc"),
            ("aspen3", ".."),
        ];
        for (target, path) in cases {
            let refused = parse_scope(target, path);
            assert!(refused.is_err(), "{target} {path}");
            assert!(
                refused
                    .err()
                    .is_some_and(|diagnostic| diagnostic.code.starts_with("fleet_scope")),
                "{target} {path}"
            );
        }
        assert!(parse_scope_text("OnixResearch/kiln").is_err());
        assert!(parse_scope("", "crates").is_err());
        assert!(parse_scope("onix/kiln", "crates/kiln\u{e9}").is_err());
    }
}
