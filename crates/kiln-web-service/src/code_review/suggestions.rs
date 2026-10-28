//! Exact published suggestion extraction. Provider revalidates the same blob bytes.
//! r[impl kiln.review.parity.ergonomics]
use super::*;

const MAX_SUGGESTIONS: usize = 32;
const MAX_SUGGESTION_BLOB_BYTES: usize = 2 * 1024 * 1024;

pub(super) fn edits(
    profile: &Profile,
    change: &domain::Change,
    thread_ids: &[String],
) -> Result<Vec<SuggestionEdit>, Failure> {
    if thread_ids.is_empty()
        || thread_ids.len() > MAX_SUGGESTIONS
        || thread_ids.iter().collect::<BTreeSet<_>>().len() != thread_ids.len()
    {
        return reject(400, "review_suggestion_selection_invalid");
    }
    let revision = change.current()?;
    let repository = git2::Repository::open(&profile.provider.repository_path)
        .map_err(|_| Failure::Unavailable)?;
    let commit = repository
        .find_commit(git2::Oid::from_str(&revision.commit).map_err(|_| Failure::Unavailable)?)
        .map_err(|_| Failure::Unavailable)?;
    let tree = commit.tree().map_err(|_| Failure::Unavailable)?;
    let mut edits = Vec::with_capacity(thread_ids.len());
    for id in thread_ids {
        let thread = change
            .threads
            .iter()
            .find(|thread| &thread.id == id)
            .ok_or_else(|| Failure::Api(404, "review_suggestion_missing".into()))?;
        if thread.resolved
            || thread.anchor.revision != revision.id
            || thread.anchor.side != domain::Side::Revision
            || thread.anchor.start_line == 0
            || thread.anchor.end_line < thread.anchor.start_line
        {
            return reject(409, "review_suggestion_stale");
        }
        let replacements: Vec<_> = thread
            .comments
            .iter()
            .filter_map(|comment| replacement(&comment.body))
            .collect();
        if replacements.len() != 1 {
            return reject(400, "review_suggestion_ambiguous");
        }
        let path = Path::new(&thread.anchor.file);
        if path.is_absolute()
            || path
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return reject(400, "review_suggestion_path_invalid");
        }
        let entry = tree
            .get_path(path)
            .map_err(|_| Failure::Api(409, "review_suggestion_file_missing".into()))?;
        if !matches!(entry.filemode(), 0o100644 | 0o100755) {
            return reject(400, "review_suggestion_file_invalid");
        }
        let blob = repository
            .find_blob(entry.id())
            .map_err(|_| Failure::Unavailable)?;
        if blob.size() > MAX_SUGGESTION_BLOB_BYTES || blob.is_binary() {
            return reject(400, "review_suggestion_file_invalid");
        }
        let text = std::str::from_utf8(blob.content())
            .map_err(|_| Failure::Api(400, "review_suggestion_encoding".into()))?;
        let start = thread.anchor.start_line as usize - 1;
        let count = (thread.anchor.end_line - thread.anchor.start_line + 1) as usize;
        let lines: Vec<_> = text.split_inclusive('\n').collect();
        if start.checked_add(count).is_none_or(|end| end > lines.len()) {
            return reject(409, "review_suggestion_range_invalid");
        }
        edits.push(SuggestionEdit {
            path: thread.anchor.file.clone(),
            start_line: thread.anchor.start_line,
            end_line: thread.anchor.end_line,
            expected_original: lines
                .get(start..start + count)
                .ok_or(Failure::Unavailable)?
                .concat(),
            replacement: replacements
                .first()
                .ok_or(Failure::Unavailable)?
                .to_string(),
        });
    }
    Ok(edits)
}

fn replacement(body: &str) -> Option<&str> {
    let mut opening = None;
    let mut result = None;
    let mut offset = 0;
    for line in body.split_inclusive('\n') {
        let marker = line.trim_end_matches(['\r', '\n']);
        if marker == "```suggestion" {
            if opening.is_some() || result.is_some() {
                return None;
            }
            opening = Some(offset + line.len());
        } else if marker == "```" {
            if let Some(start) = opening.take() {
                result = Some(&body[start..offset]);
            }
        }
        offset += line.len();
    }
    if opening.is_some() { None } else { result }
}

#[cfg(test)]
mod tests {
    use super::replacement;
    #[test]
    fn exact_fence_preserves_replacement_and_rejects_ambiguous_or_unclosed() {
        assert_eq!(
            replacement("Use this:\n```suggestion\nlet x = 2;\n```\n"),
            Some("let x = 2;\n")
        );
        assert_eq!(replacement("```suggestion\n```"), Some(""));
        assert_eq!(
            replacement("```suggestion\na\n```\n```suggestion\nb\n```"),
            None
        );
        assert_eq!(replacement("```suggestion\na"), None);
    }
}
