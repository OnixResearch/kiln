//! Repository-relative path validity and glob matching shared by review ownership
//! and monorepo project scopes.
//!
//! Paths use `/` separators. Patterns support `*` and `?` within one segment and a
//! whole `**` segment spanning zero or more segments. Matching performs no
//! allocation and consults no filesystem state or locale rules.

/// A canonical repository-relative path: non-empty, `/`-separated, without empty,
/// `.` or `..` segments, backslashes or control characters (including NUL).
/// A leading `/` yields an empty first segment, so absolute paths are rejected.
#[must_use]
pub fn canonical(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        && !path.chars().any(char::is_control)
        && path.split('/').all(|part| !matches!(part, "" | "." | ".."))
}

/// A canonical path usable as a glob: no character classes or alternations, and
/// `**` only as a whole segment.
#[must_use]
pub fn valid_pattern(pattern: &str) -> bool {
    canonical(pattern)
        && !pattern.contains(['[', ']', '{', '}'])
        && pattern
            .split('/')
            .all(|part| !part.contains("**") || part == "**")
}

/// Greedy wildcard matching of one segment.
fn component_matches(pattern: &str, value: &str) -> bool {
    let mut pattern = pattern.chars();
    let mut value = value.chars();
    let mut retry = None;
    loop {
        match (pattern.clone().next(), value.clone().next()) {
            (Some('*'), _) => {
                pattern.next();
                retry = Some((pattern.clone(), value.clone()));
            }
            (Some(want), Some(got)) if want == '?' || want == got => {
                pattern.next();
                value.next();
            }
            (None, None) => return true,
            _ => {
                let Some((resume, rest)) = retry.as_mut() else {
                    return false;
                };
                if rest.next().is_none() {
                    return false;
                }
                pattern = resume.clone();
                value = rest.clone();
            }
        }
    }
}

/// Whether `path` matches `pattern` segment by segment.
#[must_use]
pub fn matches(pattern: &str, path: &str) -> bool {
    let mut pattern = pattern.split('/');
    let mut path = path.split('/');
    let mut retry = None;
    loop {
        match (pattern.clone().next(), path.clone().next()) {
            (Some("**"), _) => {
                pattern.next();
                retry = Some((pattern.clone(), path.clone()));
            }
            (Some(want), Some(got)) if component_matches(want, got) => {
                pattern.next();
                path.next();
            }
            (None, None) => return true,
            _ => {
                let Some((resume, rest)) = retry.as_mut() else {
                    return false;
                };
                if rest.next().is_none() {
                    return false;
                }
                pattern = resume.clone();
                path = rest.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one glob table: review ownership and monorepo scopes both match through
    /// [`matches`], so this table pins both.
    #[test]
    fn glob_table_shared_with_review_ownership() {
        for (pattern, path, matched) in [
            ("src/**", "src/lib.rs", true),
            ("src/*", "src/nested/lib.rs", false),
            ("**/lib.?s", "src/nested/lib.rs", true),
            ("**/lib.rs", "lib.rs", true),
            ("src/**/test/*.rs", "src/a/b/test/lib.rs", true),
            ("src/**/test/*.rs", "src/test/lib.rs", true),
            ("src/**/test/*.rs", "src/test/lib.py", false),
            ("src/**", "other/src/lib.rs", false),
            ("src/?.rs", "src/é.rs", true),
            ("**", "a", true),
            ("**", "a/b/c", true),
            ("*", "a/b", false),
            ("*.md", "README.md", true),
            ("*.md", "docs/README.md", false),
            ("a*c", "abbbc", true),
            ("a*c", "abbbd", false),
            ("?", "", false),
            ("crates/*/Cargo.toml", "crates/kiln-core/Cargo.toml", true),
            ("crates/*/Cargo.toml", "crates/a/b/Cargo.toml", false),
        ] {
            assert_eq!(matches(pattern, path), matched, "{pattern}: {path}");
        }
    }

    #[test]
    fn canonical_paths_and_patterns_reject_ambiguous_forms() {
        for good in ["a", "a/b.rs", "crates/api", "**/x", "é/ü"] {
            assert!(canonical(good), "{good}");
        }
        for bad in [
            "", "/a", "a/", "a//b", "./a", "a/./b", "../a", "a/..", "a\\b", "a\0b", "a\nb",
        ] {
            assert!(!canonical(bad), "{bad:?}");
            assert!(!valid_pattern(bad), "{bad:?}");
        }
        for good in ["**", "src/**/*.rs", "?.rs"] {
            assert!(valid_pattern(good), "{good}");
        }
        for bad in ["a**", "**b/c", "[ab]", "{a,b}"] {
            assert!(!valid_pattern(bad), "{bad}");
        }
    }
}
