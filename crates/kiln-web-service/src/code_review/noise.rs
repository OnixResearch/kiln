//! Display-only review noise rules (`review.noise` in the review profile).
//!
//! Rules classify displayed files by declared glob only. Classification is a
//! pure function of the profile and an exact comparison; it never reaches the
//! review domain, so ownership, approvals, blocking threads, CI and queue
//! admission are unaffected. "Pre-viewed" is derived from the annotation at
//! display time and is never stored as a viewed marker.
use super::*;

/// Named bound on declared noise rules.
pub const MAX_NOISE_RULES: usize = 64;
/// Named byte bound on one noise pattern.
pub const MAX_NOISE_PATTERN_BYTES: usize = 256;
/// Named byte bound on one noise rule id.
pub const MAX_NOISE_ID_BYTES: usize = 128;
/// Named byte bound on one noise rule reason.
pub const MAX_NOISE_REASON_BYTES: usize = 256;
const NOISE_SCHEMA: &str = "kiln.review-noise.v1";

/// Display preferences of the review profile.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewDisplay {
    #[serde(default)]
    pub noise: Vec<NoiseRule>,
}

/// One declared low-signal path glob.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NoiseRule {
    pub id: String,
    pub pattern: String,
    pub reason: String,
}

impl ReviewDisplay {
    /// Admit rules with the ownership glob validator and named bounds.
    // r[impl kiln.review.noise]
    // r[impl kiln.review.noise.scenario.invalid]
    pub fn admit(&self) -> Result<(), String> {
        if self.noise.len() > MAX_NOISE_RULES {
            return Err("review noise rule count exceeds its bound".into());
        }
        let mut ids = BTreeSet::new();
        for rule in &self.noise {
            if !valid_principal(&rule.id)
                || rule.id.len() > MAX_NOISE_ID_BYTES
                || !ids.insert(rule.id.as_str())
                || rule.pattern.len() > MAX_NOISE_PATTERN_BYTES
                || !kiln_core::paths::valid_pattern(&rule.pattern)
                || !valid_text(&rule.reason, MAX_NOISE_REASON_BYTES)
            {
                return Err(
                    "review noise rule needs a unique id, a bounded valid path glob and a bounded reason"
                        .into(),
                );
            }
        }
        Ok(())
    }

    /// BLAKE3 display identity of the declared rules; `None` without rules.
    // r[impl kiln.review.noise.identity]
    pub fn identity(&self) -> Option<String> {
        if self.noise.is_empty() {
            return None;
        }
        serde_json::to_vec(&json!({"schema":NOISE_SCHEMA,"rules":self.noise}))
            .ok()
            .map(|bytes| blake3::hash(&bytes).to_hex().to_string())
    }
}

/// First rule, in profile order, matching the new or the old path.
pub fn rule_for<'a>(
    rules: &'a [NoiseRule],
    new_path: Option<&str>,
    old_path: Option<&str>,
) -> Option<&'a NoiseRule> {
    rules.iter().find(|rule| {
        [new_path, old_path]
            .into_iter()
            .flatten()
            .any(|path| kiln_core::paths::matches(&rule.pattern, path))
    })
}

/// Published rules and identity; `null` when the profile declares none.
pub fn published(display: Option<&ReviewDisplay>) -> Value {
    match display.filter(|display| !display.noise.is_empty()) {
        Some(display) => json!({"identity":display.identity(),"rules":display.noise}),
        None => Value::Null,
    }
}

/// Annotation of one exact comparison: every displayed file matching a rule,
/// keyed by its displayed path, with the first matching rule id.
// r[impl kiln.review.noise.display]
pub fn annotate(display: Option<&ReviewDisplay>, comparison: &CommitComparison) -> Value {
    let Some(display) = display.filter(|display| !display.noise.is_empty()) else {
        return Value::Null;
    };
    let files: BTreeMap<&str, &str> = comparison
        .files
        .iter()
        .filter_map(|file| {
            let shown = file.new_path.as_deref().or(file.old_path.as_deref())?;
            let rule = rule_for(
                &display.noise,
                file.new_path.as_deref(),
                file.old_path.as_deref(),
            )?;
            Some((shown, rule.id.as_str()))
        })
        .collect();
    json!({"identity":display.identity(),"rules":display.noise,"files":files})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(id: &str, pattern: &str) -> NoiseRule {
        NoiseRule {
            id: id.into(),
            pattern: pattern.into(),
            reason: "Generated or vendored content".into(),
        }
    }

    fn display(rules: Vec<NoiseRule>) -> ReviewDisplay {
        ReviewDisplay { noise: rules }
    }

    fn file(new_path: Option<&str>, old_path: Option<&str>) -> DiffFile {
        DiffFile {
            old_path: old_path.map(String::from),
            new_path: new_path.map(String::from),
            old_blob: None,
            new_blob: None,
            old_line_count: None,
            new_line_count: None,
            renamed: new_path.is_some() && old_path.is_some() && new_path != old_path,
            binary: false,
            truncated: false,
            hunks: Vec::new(),
        }
    }

    fn comparison(files: Vec<DiffFile>) -> CommitComparison {
        CommitComparison {
            repository_id: "r".into(),
            old_commit: "a".into(),
            new_commit: "b".into(),
            files,
            truncated: false,
        }
    }

    fn kiln_rules() -> ReviewDisplay {
        display(vec![
            rule("lockfiles", "**/Cargo.lock"),
            rule("vendor", "vendor/**"),
            rule("generated", "cairn-policy/generated/**"),
        ])
    }

    // r[verify kiln.review.noise.scenario.fixture]
    // r[verify kiln.review.noise.display]
    #[test]
    fn lockfile_and_vendored_files_are_annotated_and_source_is_not() {
        let rules = kiln_rules();
        let annotated = annotate(
            Some(&rules),
            &comparison(vec![
                file(Some("Cargo.lock"), Some("Cargo.lock")),
                file(Some("vendor/dep/src/lib.rs"), None),
                file(Some("src/lib.rs"), Some("src/lib.rs")),
                // A rename out of a noise path is classified by its old path.
                file(Some("third_party/dep.rs"), Some("vendor/dep.rs")),
                // A deletion keeps its old path.
                file(None, Some("crates/x/Cargo.lock")),
            ]),
        );
        assert_eq!(
            annotated.get("files"),
            Some(
                &json!({"Cargo.lock":"lockfiles","vendor/dep/src/lib.rs":"vendor","third_party/dep.rs":"vendor","crates/x/Cargo.lock":"lockfiles"})
            )
        );
        assert!(annotated.pointer("/files/src~1lib.rs").is_none());
        assert_eq!(annotated.get("identity"), Some(&json!(rules.identity())));
        assert_eq!(
            annotated
                .get("rules")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(3)
        );
    }

    #[test]
    fn first_rule_in_profile_order_wins_and_no_rules_annotate_nothing() {
        let rules = display(vec![
            rule("all-vendor", "vendor/**"),
            rule("deep", "vendor/a/**"),
        ]);
        assert_eq!(
            rule_for(&rules.noise, Some("vendor/a/b.rs"), None).map(|r| r.id.as_str()),
            Some("all-vendor")
        );
        assert!(rule_for(&rules.noise, Some("src/vendor/a.rs"), None).is_none());
        let empty = display(Vec::new());
        assert_eq!(empty.identity(), None);
        assert!(
            annotate(
                Some(&empty),
                &comparison(vec![file(Some("vendor/a"), None)])
            )
            .is_null()
        );
        assert!(annotate(None, &comparison(vec![file(Some("vendor/a"), None)])).is_null());
        assert!(published(None).is_null());
    }

    // r[verify kiln.review.noise.identity]
    #[test]
    fn identity_tracks_every_rule_field() {
        let base = kiln_rules();
        let id = base.identity();
        assert!(id.as_ref().is_some_and(|id| id.len() == 64));
        assert_eq!(kiln_rules().identity(), id);
        for change in [
            |r: &mut NoiseRule| r.id.push('x'),
            |r: &mut NoiseRule| r.pattern = "vendor/*".into(),
            |r: &mut NoiseRule| r.reason.push('!'),
        ] {
            let mut edited = kiln_rules();
            change(
                edited
                    .noise
                    .get_mut(1)
                    .unwrap_or_else(|| unreachable!("fixture has three rules")),
            );
            assert_ne!(edited.identity(), id);
        }
    }

    // r[verify kiln.review.noise.scenario.invalid]
    #[test]
    fn invalid_or_unbounded_rules_are_rejected() {
        assert!(kiln_rules().admit().is_ok());
        assert!(display(Vec::new()).admit().is_ok());
        let max = display(
            (0..MAX_NOISE_RULES)
                .map(|index| rule(&format!("rule-{index}"), "vendor/**"))
                .collect(),
        );
        assert!(max.admit().is_ok());
        let mut over = max.clone();
        over.noise.push(rule("one-more", "vendor/**"));
        assert!(over.admit().is_err());
        for pattern in [
            "/etc/passwd",
            "../outside",
            "vendor/../src",
            "vendor//x",
            "./vendor",
            "vendor/",
            "vendor\\x",
            "vendor/[ab]",
            "vendor/{a,b}",
            "vendor/a**",
            "**b/x",
            "",
            "vendor/\u{7}",
        ] {
            assert!(
                display(vec![rule("bad", pattern)]).admit().is_err(),
                "{pattern:?} must be rejected"
            );
        }
        let long = "a/".repeat(MAX_NOISE_PATTERN_BYTES / 2) + "b";
        assert!(display(vec![rule("long", &long)]).admit().is_err());
        let bounded = "a".repeat(MAX_NOISE_PATTERN_BYTES);
        assert!(display(vec![rule("bounded", &bounded)]).admit().is_ok());
        assert!(
            display(vec![rule("dup", "a/**"), rule("dup", "b/**")])
                .admit()
                .is_err()
        );
        for id in [
            "",
            "has space",
            "star*",
            &"i".repeat(MAX_NOISE_ID_BYTES + 1),
        ] {
            assert!(display(vec![rule(id, "a/**")]).admit().is_err(), "{id:?}");
        }
        for reason in [
            String::new(),
            "  ".into(),
            "line\nbreak".into(),
            "r".repeat(MAX_NOISE_REASON_BYTES + 1),
        ] {
            let mut bad = rule("ok", "a/**");
            bad.reason = reason;
            assert!(display(vec![bad]).admit().is_err());
        }
        assert!(
            serde_json::from_value::<ReviewDisplay>(
                json!({"noise":[{"id":"a","pattern":"a","reason":"r","extra":1}]})
            )
            .is_err()
        );
    }
}
