//! Shell-owned hunk identities for private viewed markers: BLAKE3 over the
//! normalized hunk (path, removed and added lines, no line numbers or context).
//! The pure core only stores and compares these digests.
// r[impl kiln.review.hunks]
use super::*;

const HUNK_DOMAIN: &str = "kiln.review.hunk.v1";
const FILE_DOMAIN: &str = "kiln.review.file.v1";

fn digest(value: &Value) -> String {
    blake3::hash(value.to_string().as_bytes())
        .to_hex()
        .to_string()
}

fn path_of(file: &DiffFile) -> &str {
    file.new_path
        .as_deref()
        .or(file.old_path.as_deref())
        .unwrap_or_default()
}

/// Digest of one hunk: only removed, added and end-of-file marker rows count.
pub(super) fn hunk_digest(file: &DiffFile, hunk: &DiffHunk) -> String {
    let rows: Vec<Value> = hunk
        .lines
        .iter()
        .filter_map(|line| {
            let kind = match line.kind {
                DiffLineKind::Context => return None,
                DiffLineKind::Deletion => "-",
                DiffLineKind::Addition => "+",
                DiffLineKind::NoNewline => "\\",
            };
            Some(json!([kind, line.text]))
        })
        .collect();
    digest(&json!([HUNK_DOMAIN, path_of(file), rows]))
}

/// Normalized hunk digests of one file, in display order. A file without
/// textual hunks (rename, mode change, empty file) is one whole-file hunk.
/// Binary and truncated files have none: they cannot be marked as viewed.
pub(super) fn file_digests(file: &DiffFile) -> Vec<String> {
    if file.binary || file.truncated {
        return Vec::new();
    }
    if file.hunks.is_empty() {
        return vec![digest(&json!([
            FILE_DOMAIN,
            path_of(file),
            file.old_path,
            file.old_blob,
            file.new_blob
        ]))];
    }
    file.hunks
        .iter()
        .map(|hunk| hunk_digest(file, hunk))
        .collect()
}

/// Trusted hunk facts of a revision's exact base comparison.
pub(super) fn facts(comparison: &CommitComparison) -> Vec<domain::HunkFact> {
    comparison
        .files
        .iter()
        .flat_map(|file| {
            file_digests(file)
                .into_iter()
                .map(|digest| domain::HunkFact {
                    file: path_of(file).to_owned(),
                    digest,
                })
        })
        .collect()
}

fn base_comparison(
    provider: &impl ChangeReviewPort,
    revision: &domain::Revision,
) -> Result<CommitComparison, Failure> {
    provider
        .compare(&CompareRequest {
            old_commit: revision.base.clone(),
            new_commit: revision.commit.clone(),
            limits: DiffLimits::default(),
            whitespace: WhitespaceMode::Exact,
        })
        .map_err(Failure::from)
}

/// Per-principal viewed progress for `revision`: per file the hunk digests,
/// which are viewed, and which are new since the principal's last viewed
/// revision. `exact_base` is the displayed comparison when it is the
/// revision's exact base comparison, so displayed hunks align with digests.
pub(super) fn review(
    provider: &impl ChangeReviewPort,
    change: &domain::Change,
    revision: &domain::Revision,
    principal: &str,
    exact_base: Option<&CommitComparison>,
) -> Result<Value, Failure> {
    let owned;
    let base = match exact_base {
        Some(comparison) => comparison,
        None => {
            owned = base_comparison(provider, revision)?;
            &owned
        }
    };
    let last = change
        .last_viewed
        .get(principal)
        .and_then(|id| change.revisions.iter().find(|r| &r.id == id));
    let earlier: Option<BTreeSet<(String, String)>> = match last {
        Some(last) if last.id != revision.id => Some(
            facts(&base_comparison(provider, last)?)
                .into_iter()
                .map(|fact| (fact.file, fact.digest))
                .collect(),
        ),
        _ => None,
    };
    let files: Vec<Value> = base
        .files
        .iter()
        .map(|file| {
            let path = path_of(file);
            let hunks: Vec<Value> = file_digests(file)
                .into_iter()
                .map(|digest| {
                    let viewed =
                        domain::hunk_viewed(change, principal, &revision.id, path, &digest);
                    let fresh = earlier
                        .as_ref()
                        .is_some_and(|set| !set.contains(&(path.to_owned(), digest.clone())));
                    json!({"digest":digest,"viewed":viewed,"new_since_viewed":fresh})
                })
                .collect();
            let count = |key: &str| hunks.iter().filter(|h| h[key] == json!(true)).count();
            json!({"path":path,"total":hunks.len(),"viewed":count("viewed"),
                "new_since_viewed":count("new_since_viewed"),"hunks":hunks})
        })
        .collect();
    Ok(
        json!({"revision":revision.id,"exact_base":exact_base.is_some(),
        "last_viewed":last.map(|r| json!({"id":r.id,"number":r.number})),"files":files}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(kind: DiffLineKind, old: Option<u32>, new: Option<u32>, text: &str) -> DiffLine {
        DiffLine {
            kind,
            old_line: old,
            new_line: new,
            text: text.into(),
        }
    }
    fn file(path: &str, hunks: Vec<DiffHunk>) -> DiffFile {
        DiffFile {
            old_path: Some(path.into()),
            new_path: Some(path.into()),
            old_blob: Some("a".repeat(40)),
            new_blob: Some("b".repeat(40)),
            old_line_count: Some(100),
            new_line_count: Some(100),
            renamed: false,
            binary: false,
            truncated: false,
            hunks,
        }
    }
    fn hunk(start: u32, context: &str, removed: &str, added: &str) -> DiffHunk {
        DiffHunk {
            old_start: start,
            old_lines: 2,
            new_start: start,
            new_lines: 2,
            header: format!("@@ -{start},2 +{start},2 @@"),
            lines: vec![
                line(DiffLineKind::Context, Some(start), Some(start), context),
                line(DiffLineKind::Deletion, Some(start + 1), None, removed),
                line(DiffLineKind::Addition, None, Some(start + 1), added),
            ],
        }
    }

    // r[verify kiln.review.hunks]
    #[test]
    fn digest_ignores_line_numbers_and_context_but_not_content_or_path() {
        let a = hunk(10, "fn a() {", "old\n", "new\n");
        let moved = hunk(90, "different context", "old\n", "new\n");
        let edited = hunk(10, "fn a() {", "old\n", "newer\n");
        let f = file("src/lib.rs", vec![a.clone()]);
        assert_eq!(hunk_digest(&f, &a), hunk_digest(&f, &moved));
        assert_ne!(hunk_digest(&f, &a), hunk_digest(&f, &edited));
        assert_ne!(
            hunk_digest(&f, &a),
            hunk_digest(&file("src/other.rs", vec![]), &a)
        );
        assert_eq!(hunk_digest(&f, &a).len(), 64);
        // Removed and added sides are distinguished.
        let swapped = hunk(10, "fn a() {", "new\n", "old\n");
        assert_ne!(hunk_digest(&f, &a), hunk_digest(&f, &swapped));
    }

    // r[verify kiln.review.hunks]
    #[test]
    fn rename_only_files_have_one_digest_and_binary_or_truncated_have_none() {
        let mut renamed = file("new.rs", Vec::new());
        renamed.old_path = Some("old.rs".into());
        renamed.renamed = true;
        assert_eq!(file_digests(&renamed).len(), 1);
        let mut binary = file("image.png", Vec::new());
        binary.binary = true;
        assert!(file_digests(&binary).is_empty());
        let mut truncated = file("big.rs", vec![hunk(1, "x", "y\n", "z\n")]);
        truncated.truncated = true;
        assert!(file_digests(&truncated).is_empty());
        let comparison = CommitComparison {
            repository_id: "r".into(),
            old_commit: "o".into(),
            new_commit: "n".into(),
            files: vec![
                renamed,
                binary,
                truncated,
                file(
                    "a.rs",
                    vec![hunk(1, "x", "y\n", "z\n"), hunk(20, "x", "p\n", "q\n")],
                ),
            ],
            truncated: false,
        };
        let observed = facts(&comparison);
        assert_eq!(observed.len(), 3);
        assert_eq!(observed.iter().filter(|f| f.file == "a.rs").count(), 2);
    }
}
