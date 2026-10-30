//! Opt-in structural (syntax-aware) comparison for display only. Grammars run
//! server-side through `kiln-highlight`; every file either carries exact-line
//! structural rows or an explicit fallback reason, and the line comparison
//! stays in the response for anchors and fallback rendering.
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

/// Structural view per displayed file, in comparison order. `note` forces an
/// explicit fallback for every file (inexact old side or whitespace filter).
// r[impl kiln.review.structural.scenario.fallback]
pub(super) fn files(
    provider: &impl ChangeReviewPort,
    diff: &CommitComparison,
    note: Option<&str>,
) -> Result<Value, Failure> {
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
    Ok(json!({"files":files,"deadline_ms":STRUCTURAL_DEADLINE.as_millis()}))
}
