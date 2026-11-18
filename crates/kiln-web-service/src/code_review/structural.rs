//! Opt-in structural (syntax-aware) comparison for display only. Grammars run
//! server-side through `kiln-highlight`; every file either carries exact-line
//! structural rows or an explicit fallback reason, and the line comparison
//! stays in the response for anchors and fallback rendering. The same per-file
//! function produces the opt-in structural evidence of a preview comparison.
// r[impl kiln.review.structural]
use std::time::{Duration, Instant};

use super::*;

/// Wall-clock bound for all structural matching of one diff response.
const STRUCTURAL_DEADLINE: Duration = Duration::from_millis(2_000);

fn fallback(path: &str, reason: &str) -> Value {
    json!({"path":path,"status":"fallback","reason":reason})
}

fn blob(
    provider: &impl ChangeReviewPort,
    id: Option<&String>,
) -> Result<Result<Option<Vec<u8>>, &'static str>, Failure> {
    let Some(id) = id else {
        return Ok(Ok(None));
    };
    let limit = u32::try_from(kiln_highlight::MAX_STRUCTURAL_BYTES).unwrap_or(u32::MAX);
    match provider.blob(id, limit) {
        Ok(Some(bytes)) => Ok(Ok(Some(bytes))),
        Ok(None) => Ok(Err("too_large")),
        Err(error) if error.kind == ReviewErrorKind::Unsupported => Ok(Err("unsupported_provider")),
        Err(error) => Err(error.into()),
    }
}

/// Structural outcome per displayed file, in comparison order. `note` forces an
/// explicit fallback for every file (inexact old side or whitespace filter).
// r[impl kiln.review.structural.scenario.fallback]
fn outcomes(
    provider: &impl ChangeReviewPort,
    diff: &CommitComparison,
    note: Option<&str>,
) -> Result<Vec<Value>, Failure> {
    let deadline = Instant::now() + STRUCTURAL_DEADLINE;
    let interrupted = || Instant::now() >= deadline;
    let mut files = Vec::with_capacity(diff.files.len());
    for file in &diff.files {
        let path = file
            .new_path
            .as_deref()
            .or(file.old_path.as_deref())
            .unwrap_or_default();
        let early = note.or(if file.binary {
            Some("binary")
        } else if file.truncated || diff.truncated {
            Some("truncated")
        } else {
            None
        });
        if let Some(reason) = early {
            files.push(fallback(path, reason));
            continue;
        }
        let old = match blob(
            provider,
            file.old_blob.as_ref().filter(|_| file.old_path.is_some()),
        )? {
            Ok(bytes) => bytes,
            Err(reason) => {
                files.push(fallback(path, reason));
                continue;
            }
        };
        let new = match blob(
            provider,
            file.new_blob.as_ref().filter(|_| file.new_path.is_some()),
        )? {
            Ok(bytes) => bytes,
            Err(reason) => {
                files.push(fallback(path, reason));
                continue;
            }
        };
        let outcome = kiln_highlight::structural_diff(
            kiln_highlight::language_for_path(path),
            old.as_deref(),
            new.as_deref(),
            &kiln_highlight::StructuralBounds::DEFAULT,
            &interrupted,
        );
        let mut value = serde_json::to_value(outcome).map_err(|_| Failure::Unavailable)?;
        if let Some(object) = value.as_object_mut() {
            object.insert("path".into(), json!(path));
        }
        files.push(value);
    }
    Ok(files)
}

/// Structural view per displayed file, in comparison order.
pub(super) fn files(
    provider: &impl ChangeReviewPort,
    diff: &CommitComparison,
    note: Option<&str>,
) -> Result<Value, Failure> {
    Ok(
        json!({"files":outcomes(provider, diff, note)?,"deadline_ms":STRUCTURAL_DEADLINE.as_millis()}),
    )
}

/// Rules version of structural preview evidence. Bump it with any change to
/// the record, its settings or how its files are chosen.
const PREVIEW_RULES: &str = "kiln.preview.structural-rules.v1";
const PREVIEW_SCHEMA: &str = "kiln.preview.structural.v1";
const PREVIEW_DOMAIN: &[u8] = b"kiln.preview.structural.v1\0";
const GRAMMAR_DOMAIN: &[u8] = b"kiln.highlight.grammar-set.v1\0";
const PREVIEW_PROVENANCE: &str = "structural comparison of two exact revisions for preview evidence; display only; not CI, review, merge or completion evidence";
/// Largest serialized record. A larger one keeps no files, so the comparison
/// response stays within its own bound.
const MAX_PREVIEW_EVIDENCE_BYTES: usize = 2 * 1024 * 1024;

/// Everything besides the two commits that decides a record's files.
#[derive(Serialize)]
struct PreviewSettings {
    rules: &'static str,
    engine_rules: &'static str,
    grammar_identity: String,
    max_bytes: usize,
    max_nodes: usize,
    max_steps: usize,
    context_lines: usize,
    deadline_ms: u64,
    whitespace: WhitespaceMode,
    diff_limits: DiffLimits,
}

/// A record's files, or why it has none.
enum PreviewLane {
    Structural(Vec<Value>),
    Incompatible(Vec<&'static str>),
    Unavailable(&'static str),
}

#[derive(Serialize)]
struct PreviewBody<'a> {
    schema: &'static str,
    comparison: &'a str,
    baseline_revision: &'a str,
    candidate_revision: &'a str,
    settings: &'a PreviewSettings,
    status: &'static str,
    reasons: &'a [&'static str],
    files: &'a [Value],
    provenance: &'static str,
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    hasher.finalize().to_hex().to_string()
}

fn preview_settings() -> Result<PreviewSettings, Failure> {
    let grammars =
        serde_json::to_vec(&kiln_highlight::grammar_set()).map_err(|_| Failure::Unavailable)?;
    let bounds = kiln_highlight::StructuralBounds::DEFAULT;
    Ok(PreviewSettings {
        rules: PREVIEW_RULES,
        engine_rules: kiln_highlight::STRUCTURAL_RULES,
        grammar_identity: digest(GRAMMAR_DOMAIN, &grammars),
        max_bytes: bounds.max_bytes,
        max_nodes: bounds.max_nodes,
        max_steps: bounds.max_steps,
        context_lines: kiln_highlight::STRUCTURAL_CONTEXT_LINES,
        deadline_ms: u64::try_from(STRUCTURAL_DEADLINE.as_millis()).unwrap_or(u64::MAX),
        whitespace: WhitespaceMode::Exact,
        diff_limits: DiffLimits::default(),
    })
}

/// The per-file outcomes structural mode shows for an exact comparison of the
/// two commits. Provider failures stay inside the record.
fn preview_files(
    provider: &impl ChangeReviewPort,
    settings: &PreviewSettings,
    baseline_revision: &str,
    candidate_revision: &str,
) -> Result<PreviewLane, Failure> {
    let request = CompareRequest {
        old_commit: baseline_revision.into(),
        new_commit: candidate_revision.into(),
        limits: settings.diff_limits,
        whitespace: settings.whitespace,
    };
    let diff = match provider.compare(&request) {
        Ok(diff) => diff,
        Err(error) if error.kind == ReviewErrorKind::Unsupported => {
            return Ok(PreviewLane::Unavailable("unsupported_provider"));
        }
        Err(_) => return Ok(PreviewLane::Unavailable("source_unavailable")),
    };
    match outcomes(provider, &diff, None) {
        Ok(files) => Ok(PreviewLane::Structural(files)),
        Err(Failure::Api(..)) => Ok(PreviewLane::Unavailable("source_unavailable")),
        Err(failure) => Err(failure),
    }
}

/// The identified record: its identity is BLAKE3 over a domain tag and the
/// serialized body that the response carries.
fn preview_record(
    comparison: &str,
    baseline_revision: &str,
    candidate_revision: &str,
    settings: &PreviewSettings,
    lane: PreviewLane,
) -> Result<Value, Failure> {
    let (status, reasons, files) = match lane {
        PreviewLane::Structural(files) => ("structural", Vec::new(), files),
        PreviewLane::Incompatible(reasons) => ("incompatible", reasons, Vec::new()),
        PreviewLane::Unavailable(reason) => ("unavailable", vec![reason], Vec::new()),
    };
    let encode = |status, reasons: &[&'static str], files: &[Value]| {
        let body = serde_json::to_value(PreviewBody {
            schema: PREVIEW_SCHEMA,
            comparison,
            baseline_revision,
            candidate_revision,
            settings,
            status,
            reasons,
            files,
            provenance: PREVIEW_PROVENANCE,
        })
        .map_err(|_| Failure::Unavailable)?;
        let bytes = serde_json::to_vec(&body).map_err(|_| Failure::Unavailable)?;
        Ok::<_, Failure>((body, bytes))
    };
    let (mut body, mut bytes) = encode(status, &reasons, &files)?;
    // r[impl kiln.preview.structural.scenario.fallback] No partial file list is kept.
    if bytes.len() > MAX_PREVIEW_EVIDENCE_BYTES {
        (body, bytes) = encode("unavailable", &["evidence_exhausted"], &[])?;
    }
    Ok(json!({"identity":digest(PREVIEW_DOMAIN, &bytes),"evidence":body}))
}

/// Opt-in structural evidence for one exact preview comparison: the outcomes
/// the review's structural mode shows for the two revision commits, or the
/// reason the record has none.
// r[impl kiln.preview.structural]
pub(super) fn preview_evidence(
    provider: &impl ChangeReviewPort,
    comparison: &crate::preview::observation::Comparison,
    baseline_revision: &str,
    candidate_revision: &str,
) -> Result<Value, Failure> {
    let settings = preview_settings()?;
    // r[impl kiln.preview.structural.scenario.fallback] Unlike runtime cohorts
    // are never compared as if only the source changed.
    let lane = if comparison.compatible {
        preview_files(provider, &settings, baseline_revision, candidate_revision)?
    } else {
        PreviewLane::Incompatible(comparison.reasons.clone())
    };
    preview_record(
        &comparison.identity,
        baseline_revision,
        candidate_revision,
        &settings,
        lane,
    )
}

#[cfg(test)]
#[path = "structural_preview_tests.rs"]
mod preview_tests;
