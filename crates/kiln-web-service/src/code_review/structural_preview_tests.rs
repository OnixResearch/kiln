//! Structural preview evidence over a fake provider: statuses, named
//! fallbacks, identity binding and the size bound.
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

use kiln_provider_api::change_review::{
    ChangeReviewPort, CommitComparison, CommitInspection, CompareRequest, DiffFile, ReviewEffect,
    ReviewError, ReviewErrorKind, ReviewOperationRequest, ReviewProviderInfo, ReviewResult,
    WhitespaceMode,
};
use serde_json::{Value, json};

use super::{
    PREVIEW_DOMAIN, PreviewLane, PreviewSettings, digest, preview_evidence, preview_record,
    preview_settings,
};
use crate::preview::observation::Comparison;

const OLD: &str = "1111111111111111111111111111111111111111";
const NEW: &str = "2222222222222222222222222222222222222222";
const VALID: &[u8] = b"fn a() {}\n";
const EDITED: &[u8] = b"fn a() { 1 }\n";

fn error(kind: ReviewErrorKind) -> ReviewError {
    ReviewError {
        kind,
        message: "fixture".into(),
    }
}

struct Provider {
    comparison: ReviewResult<CommitComparison>,
    blobs: BTreeMap<&'static str, ReviewResult<Option<Vec<u8>>>>,
    compares: Cell<usize>,
}

impl Provider {
    fn new(comparison: ReviewResult<CommitComparison>) -> Self {
        Self {
            comparison,
            blobs: BTreeMap::new(),
            compares: Cell::new(0),
        }
    }

    fn with_blob(mut self, id: &'static str, result: ReviewResult<Option<&[u8]>>) -> Self {
        self.blobs
            .insert(id, result.map(|bytes| bytes.map(<[u8]>::to_vec)));
        self
    }
}

impl ChangeReviewPort for Provider {
    fn info(&self) -> ReviewProviderInfo {
        ReviewProviderInfo {
            version: 1,
            repository_id: "repository".into(),
            capabilities: Vec::new(),
            target_refs: Vec::new(),
        }
    }

    fn inspect(&self, _commit: &str, _target_ref: &str) -> ReviewResult<CommitInspection> {
        Err(error(ReviewErrorKind::Unsupported))
    }

    fn compare(&self, request: &CompareRequest) -> ReviewResult<CommitComparison> {
        self.compares.set(self.compares.get() + 1);
        assert_eq!(
            (
                request.old_commit.as_str(),
                request.new_commit.as_str(),
                request.whitespace
            ),
            (OLD, NEW, WhitespaceMode::Exact)
        );
        self.comparison.clone()
    }

    fn blob(&self, id: &str, _max_bytes: u32) -> ReviewResult<Option<Vec<u8>>> {
        self.blobs
            .get(id)
            .cloned()
            .unwrap_or_else(|| Err(error(ReviewErrorKind::NotFound)))
    }

    fn execute(&self, _request: &ReviewOperationRequest) -> ReviewResult<ReviewEffect> {
        Err(error(ReviewErrorKind::Unsupported))
    }

    fn reconcile(&self, _request: &ReviewOperationRequest) -> ReviewResult<ReviewEffect> {
        Err(error(ReviewErrorKind::Unsupported))
    }
}

fn file(path: &str, old: &str, new: &str) -> DiffFile {
    DiffFile {
        old_path: Some(path.into()),
        new_path: Some(path.into()),
        old_blob: Some(old.into()),
        new_blob: Some(new.into()),
        old_line_count: None,
        new_line_count: None,
        renamed: false,
        binary: false,
        truncated: false,
        hunks: Vec::new(),
    }
}

fn diff(files: Vec<DiffFile>, truncated: bool) -> ReviewResult<CommitComparison> {
    Ok(CommitComparison {
        repository_id: "repository".into(),
        old_commit: OLD.into(),
        new_commit: NEW.into(),
        files,
        truncated,
    })
}

fn comparison(reasons: Vec<&'static str>) -> Comparison {
    Comparison {
        identity: "c".repeat(64),
        rules: "kiln.preview.comparison-rules.v1",
        baseline: "a".repeat(64),
        candidate: "b".repeat(64),
        compatible: reasons.is_empty(),
        reasons,
        exit_status_equal: true,
        output_equal: true,
    }
}

fn evidence(provider: &Provider, comparison: &Comparison) -> Value {
    preview_evidence(provider, comparison, OLD, NEW)
        .unwrap_or_else(|failure| unreachable!("fixture evidence: {failure:?}"))
}

fn settings() -> PreviewSettings {
    preview_settings().unwrap_or_else(|failure| unreachable!("fixture settings: {failure:?}"))
}

fn record(
    comparison: &str,
    old: &str,
    new: &str,
    settings: &PreviewSettings,
    lane: PreviewLane,
) -> Value {
    preview_record(comparison, old, new, settings, lane)
        .unwrap_or_else(|failure| unreachable!("fixture record: {failure:?}"))
}

fn text(value: &Value, pointer: &str) -> String {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// `(status, reasons, file count)` of a record.
fn shape(record: &Value) -> (String, Value, usize) {
    (
        text(record, "/evidence/status"),
        record
            .pointer("/evidence/reasons")
            .cloned()
            .unwrap_or_default(),
        record
            .pointer("/evidence/files")
            .and_then(Value::as_array)
            .map_or(usize::MAX, Vec::len),
    )
}

/// `path status reason` of every file.
fn outcomes(record: &Value) -> Vec<String> {
    record
        .pointer("/evidence/files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|file| {
            format!(
                "{} {} {}",
                text(file, "/path"),
                text(file, "/status"),
                text(file, "/reason")
            )
            .trim_end()
            .to_owned()
        })
        .collect()
}

// r[verify kiln.preview.structural.scenario.fallback]
#[test]
fn incompatible_subjects_keep_no_files_and_read_no_source() {
    let provider = Provider::new(diff(vec![file("src/lib.rs", "old", "new")], false))
        .with_blob("old", Ok(Some(VALID)))
        .with_blob("new", Ok(Some(EDITED)));
    let withheld = evidence(
        &provider,
        &comparison(vec!["arguments_differ", "engine_differs"]),
    );
    assert_eq!(provider.compares.get(), 0);
    assert_eq!(
        shape(&withheld),
        (
            "incompatible".to_owned(),
            json!(["arguments_differ", "engine_differs"]),
            0
        )
    );
    // Negative control: a compatible pair reads the exact comparison once.
    let compared = evidence(&provider, &comparison(Vec::new()));
    assert_eq!(provider.compares.get(), 1);
    assert_eq!(shape(&compared), ("structural".to_owned(), json!([]), 1));
    assert_eq!(outcomes(&compared), ["src/lib.rs structural"]);
}

// r[verify kiln.preview.structural.scenario.fallback]
#[test]
fn an_unreadable_source_stays_inside_the_record() {
    let blob_failure = Provider::new(diff(vec![file("src/lib.rs", "old", "lost")], false))
        .with_blob("old", Ok(Some(VALID)))
        .with_blob("lost", Err(error(ReviewErrorKind::Storage)));
    for (provider, reason) in [
        (
            Provider::new(Err(error(ReviewErrorKind::Unsupported))),
            "unsupported_provider",
        ),
        (
            Provider::new(Err(error(ReviewErrorKind::NotFound))),
            "source_unavailable",
        ),
        (
            Provider::new(Err(error(ReviewErrorKind::Uncertain))),
            "source_unavailable",
        ),
        (blob_failure, "source_unavailable"),
    ] {
        assert_eq!(
            shape(&evidence(&provider, &comparison(Vec::new()))),
            ("unavailable".to_owned(), json!([reason]), 0)
        );
    }
}

// r[verify kiln.preview.structural.scenario.fallback]
#[test]
fn every_file_fallback_is_named_and_keeps_no_rows() {
    let provider = Provider::new(diff(
        vec![
            DiffFile {
                binary: true,
                ..file("logo.png", "png-old", "png-new")
            },
            DiffFile {
                truncated: true,
                ..file("src/cut.rs", "old", "edited")
            },
            file("src/large.rs", "old", "large"),
            file("src/remote.rs", "remote", "edited"),
            file("notes.txt", "old", "edited"),
            file("src/broken.rs", "old", "broken"),
            file("src/lib.rs", "old", "edited"),
        ],
        false,
    ))
    .with_blob("old", Ok(Some(VALID)))
    .with_blob("edited", Ok(Some(EDITED)))
    .with_blob("large", Ok(None))
    .with_blob("remote", Err(error(ReviewErrorKind::Unsupported)))
    .with_blob("broken", Ok(Some(b"fn a( {\n")));
    let record = evidence(&provider, &comparison(Vec::new()));
    assert_eq!(
        outcomes(&record),
        [
            "logo.png fallback binary",
            "src/cut.rs fallback truncated",
            "src/large.rs fallback too_large",
            "src/remote.rs fallback unsupported_provider",
            "notes.txt fallback unsupported_language",
            "src/broken.rs fallback parse_error",
            "src/lib.rs structural",
        ]
    );
    let files = record
        .pointer("/evidence/files")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert!(
        files
            .iter()
            .filter(|file| file["status"] == "fallback")
            .all(|file| file.get("hunks").is_none() && file.get("moved").is_none())
    );
    // A truncated comparison truncates every file, including parseable ones.
    let truncated = Provider::new(diff(vec![file("src/lib.rs", "old", "edited")], true))
        .with_blob("old", Ok(Some(VALID)))
        .with_blob("edited", Ok(Some(EDITED)));
    assert_eq!(
        outcomes(&evidence(&truncated, &comparison(Vec::new()))),
        ["src/lib.rs fallback truncated"]
    );
}

// r[verify kiln.preview.structural]
#[test]
fn the_identity_binds_comparison_revisions_settings_and_outcomes() {
    let settings = settings();
    let comparison = "c".repeat(64);
    let files = || vec![json!({"path":"src/lib.rs","status":"structural"})];
    let base = record(
        &comparison,
        OLD,
        NEW,
        &settings,
        PreviewLane::Structural(files()),
    );
    assert_eq!(
        base,
        record(
            &comparison,
            OLD,
            NEW,
            &settings,
            PreviewLane::Structural(files())
        )
    );
    let identity = |value: &Value| text(value, "/identity");
    // The identity is recomputable from the body the response carries.
    let body = serde_json::to_vec(&base.get("evidence")).unwrap_or_default();
    assert_eq!(identity(&base), digest(PREVIEW_DOMAIN, &body));
    let narrower = PreviewSettings {
        max_steps: 1,
        ..preview_settings().unwrap_or_else(|failure| unreachable!("{failure:?}"))
    };
    let mut seen = BTreeSet::from([identity(&base)]);
    for changed in [
        record(
            &"d".repeat(64),
            OLD,
            NEW,
            &settings,
            PreviewLane::Structural(files()),
        ),
        record(
            &comparison,
            NEW,
            OLD,
            &settings,
            PreviewLane::Structural(files()),
        ),
        record(
            &comparison,
            OLD,
            NEW,
            &narrower,
            PreviewLane::Structural(files()),
        ),
        record(
            &comparison,
            OLD,
            NEW,
            &settings,
            PreviewLane::Structural(vec![
                json!({"path":"src/lib.rs","status":"fallback","reason":"interrupted"}),
            ]),
        ),
        record(
            &comparison,
            OLD,
            NEW,
            &settings,
            PreviewLane::Incompatible(vec!["engine_differs"]),
        ),
    ] {
        assert!(seen.insert(identity(&changed)), "{changed}");
    }
}

// r[verify kiln.preview.structural.scenario.fallback]
#[test]
fn an_oversized_record_keeps_no_files() {
    let settings = settings();
    let line = "x".repeat(1024);
    let files = |count: usize| {
        (0..count)
            .map(|n| json!({"path":format!("src/f{n}.rs"),"status":"structural","text":line}))
            .collect::<Vec<_>>()
    };
    let within = record(
        &"c".repeat(64),
        OLD,
        NEW,
        &settings,
        PreviewLane::Structural(files(1_024)),
    );
    assert_eq!(shape(&within), ("structural".to_owned(), json!([]), 1_024));
    let over = record(
        &"c".repeat(64),
        OLD,
        NEW,
        &settings,
        PreviewLane::Structural(files(2_100)),
    );
    assert_eq!(
        shape(&over),
        ("unavailable".to_owned(), json!(["evidence_exhausted"]), 0)
    );
}
