//! Diff highlighting: each file side is highlighted as context-free fragments.
//!
//! A diff is split per file; each file's language comes from its old and new
//! paths. Within every hunk the old side (context and deleted rows) and the
//! new side (context and added rows) are reconstructed and highlighted
//! separately, then tokens are mapped back to diff rows. Context rows take the
//! new side's tokens. Header, hunk and meta rows carry no tokens.

use crate::{Bounds, Highlight, Language, Reason, Token, grammar, language_for_path, lines};

/// Which reconstructed side a diff row belongs to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiffSide {
    /// A context row present on both sides.
    Both,
    /// A deleted row.
    Old,
    /// An added row.
    New,
    /// A meta row, such as a missing end-of-file newline marker.
    Neither,
}

/// One row of a structured diff hunk; `text` excludes any `+`/`-`/` ` prefix
/// and may end with one `\n`, which no token covers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiffRow<'a> {
    pub side: DiffSide,
    pub text: &'a str,
}

/// One file of a structured diff; `None` paths are absent sides.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffFile<'a> {
    pub old_path: Option<&'a str>,
    pub new_path: Option<&'a str>,
    pub binary: bool,
    pub hunks: Vec<Vec<DiffRow<'a>>>,
}

/// A row positioned in the output, with its token offset shift.
#[derive(Clone, Copy, Debug)]
struct Row<'a> {
    output: usize,
    side: DiffSide,
    content: &'a str,
    shift: u32,
}

#[derive(Debug, Default)]
struct Plan<'a> {
    old_path: Option<&'a str>,
    new_path: Option<&'a str>,
    binary: bool,
    hunks: Vec<Vec<Row<'a>>>,
}

/// Highlight a unified diff; `lines[i]` aligns with line `i` of the diff text.
///
/// Content offsets include the one-character `+`/`-`/` ` prefix.
pub fn highlight_diff(diff: &str, bounds: &Bounds) -> Highlight {
    if diff.len() > bounds.max_bytes {
        return Highlight::skipped(None, Reason::TooLarge);
    }
    let count = lines::count(diff);
    if count > bounds.max_lines {
        return Highlight::skipped(None, Reason::TooLarge);
    }
    let plans = parse_unified(diff, count);
    highlight_plans(plans, count, bounds)
}

/// Highlight a structured diff; `lines` enumerates every row of every hunk of
/// every file in order, with offsets relative to each row's `text`.
pub fn highlight_diff_files(files: &[DiffFile<'_>], bounds: &Bounds) -> Highlight {
    if files.len() > bounds.max_diff_files {
        return Highlight::skipped(None, Reason::TruncatedInput);
    }
    let rows = files.iter().flat_map(|file| file.hunks.iter().flatten());
    let (count, bytes) = rows.fold((0_usize, 0_usize), |(count, bytes), row| {
        (
            count.saturating_add(1),
            bytes.saturating_add(row.text.len()),
        )
    });
    if bytes > bounds.max_bytes || count > bounds.max_lines {
        return Highlight::skipped(None, Reason::TooLarge);
    }
    let mut output = 0_usize;
    let plans = files
        .iter()
        .map(|file| Plan {
            old_path: file.old_path,
            new_path: file.new_path,
            binary: file.binary,
            hunks: file
                .hunks
                .iter()
                .map(|hunk| {
                    hunk.iter()
                        .map(|row| {
                            let content = row.text.strip_suffix('\n').unwrap_or(row.text);
                            let side = if content.contains('\n') {
                                DiffSide::Neither
                            } else {
                                row.side
                            };
                            let positioned = Row {
                                output,
                                side,
                                content,
                                shift: 0,
                            };
                            output = output.saturating_add(1);
                            positioned
                        })
                        .collect()
                })
                .collect(),
        })
        .collect();
    highlight_plans(plans, count, bounds)
}

fn highlight_plans(plans: Vec<Plan<'_>>, count: usize, bounds: &Bounds) -> Highlight {
    if plans.len() > bounds.max_diff_files {
        return Highlight::skipped(None, Reason::TruncatedInput);
    }
    let mut output: Vec<Vec<Token>> = vec![Vec::new(); count];
    let mut budget = bounds.max_tokens;
    let mut highlighted = false;
    let mut first_skip = None;
    for plan in &plans {
        if plan.binary {
            first_skip.get_or_insert(Reason::Binary);
            continue;
        }
        let new_language = plan.new_path.or(plan.old_path).and_then(language_for_path);
        let old_language = plan.old_path.or(plan.new_path).and_then(language_for_path);
        for hunk in &plan.hunks {
            for (side, language) in [(DiffSide::Old, old_language), (DiffSide::New, new_language)] {
                match highlight_side(hunk, side, language, new_language, bounds, &mut budget) {
                    Side::Empty => {}
                    Side::Tokens(mapped) => {
                        highlighted = true;
                        for (index, tokens) in mapped {
                            if let Some(slot) = output.get_mut(index) {
                                *slot = tokens;
                            }
                        }
                    }
                    Side::Skipped(Reason::TooLarge) => {
                        return Highlight::skipped(None, Reason::TooLarge);
                    }
                    Side::Skipped(reason) => {
                        first_skip.get_or_insert(reason);
                    }
                }
            }
        }
    }
    match first_skip {
        Some(reason) if !highlighted => Highlight::skipped(None, reason),
        _ => Highlight::highlighted(None, output),
    }
}

enum Side {
    Empty,
    Tokens(Vec<(usize, Vec<Token>)>),
    Skipped(Reason),
}

/// Highlight one side of one hunk and map its tokens to output rows.
fn highlight_side(
    hunk: &[Row<'_>],
    side: DiffSide,
    language: Option<Language>,
    new_language: Option<Language>,
    bounds: &Bounds,
    budget: &mut usize,
) -> Side {
    let rows: Vec<&Row<'_>> = hunk
        .iter()
        .filter(|row| row.side == side || row.side == DiffSide::Both)
        .collect();
    if rows.is_empty() {
        return Side::Empty;
    }
    let Some(language) = language else {
        return Side::Skipped(Reason::UnsupportedLanguage);
    };
    let mut text = String::new();
    for row in &rows {
        text.push_str(row.content);
        text.push('\n');
    }
    let highlighted = match grammar::highlight_lines(language, &text, rows.len(), bounds, budget) {
        Ok(lines) => lines,
        Err(reason) => return Side::Skipped(reason),
    };
    // Context rows show the new side unless only the old side is supported.
    let keep_context = side == DiffSide::New || new_language.is_none();
    let mut mapped = Vec::with_capacity(rows.len());
    for (row, tokens) in rows.into_iter().zip(highlighted) {
        if row.side == DiffSide::Both && !keep_context {
            continue;
        }
        let shifted = tokens
            .into_iter()
            .filter_map(|token| {
                Some(Token {
                    start: token.start.checked_add(row.shift)?,
                    end: token.end.checked_add(row.shift)?,
                    class: token.class,
                })
            })
            .collect();
        mapped.push((row.output, shifted));
    }
    Side::Tokens(mapped)
}

/// Split a unified diff into per-file hunks of positioned rows.
fn parse_unified(diff: &str, count: usize) -> Vec<Plan<'_>> {
    let mut plans: Vec<Plan<'_>> = Vec::new();
    let mut current: Option<Plan<'_>> = None;
    let mut hunk: Option<Hunk<'_>> = None;
    for (index, line) in diff.split('\n').take(count).enumerate() {
        if let Some(open) = hunk.as_mut() {
            if open.accepts(line) {
                open.push(index, line);
                continue;
            }
            if let Some(closed) = hunk.take() {
                current
                    .get_or_insert_with(Plan::default)
                    .hunks
                    .push(closed.rows);
            }
        }
        if let Some(rest) = line.strip_prefix("diff --git ") {
            plans.extend(current.take());
            let (old_path, new_path) = git_header_paths(rest);
            current = Some(Plan {
                old_path,
                new_path,
                ..Plan::default()
            });
        } else if let Some(path) = line.strip_prefix("--- ") {
            if current.as_ref().is_none_or(|plan| !plan.hunks.is_empty()) {
                plans.extend(current.take());
            }
            current.get_or_insert_with(Plan::default).old_path = header_path(path);
        } else if let Some(path) = line.strip_prefix("+++ ") {
            current.get_or_insert_with(Plan::default).new_path = header_path(path);
        } else if let Some(path) = line.strip_prefix("rename from ") {
            current.get_or_insert_with(Plan::default).old_path = Some(unquote(path));
        } else if let Some(path) = line.strip_prefix("rename to ") {
            current.get_or_insert_with(Plan::default).new_path = Some(unquote(path));
        } else if line.starts_with("Binary files ") || line == "GIT binary patch" {
            current.get_or_insert_with(Plan::default).binary = true;
        } else if let Some(header) = line.strip_prefix("@@ ") {
            current.get_or_insert_with(Plan::default);
            hunk = Some(Hunk::open(header));
        }
    }
    if let Some(closed) = hunk.take() {
        current
            .get_or_insert_with(Plan::default)
            .hunks
            .push(closed.rows);
    }
    plans.extend(current);
    plans
}

/// An open hunk counting down its remaining old and new rows.
struct Hunk<'a> {
    old_left: u32,
    new_left: u32,
    rows: Vec<Row<'a>>,
}

impl<'a> Hunk<'a> {
    fn open(header: &str) -> Self {
        let mut ranges = header.split(' ');
        let old_left = ranges
            .next()
            .and_then(|range| range.strip_prefix('-'))
            .and_then(range_length);
        let new_left = ranges
            .next()
            .and_then(|range| range.strip_prefix('+'))
            .and_then(range_length);
        // An unreadable header leaves the hunk open until a non-row line.
        Self {
            old_left: old_left.unwrap_or(u32::MAX),
            new_left: new_left.unwrap_or(u32::MAX),
            rows: Vec::new(),
        }
    }

    fn accepts(&self, line: &str) -> bool {
        match line.as_bytes().first() {
            Some(b'\\') => true,
            _ if self.old_left == 0 && self.new_left == 0 => false,
            Some(b' ') | None => self.old_left > 0 && self.new_left > 0,
            Some(b'-') => self.old_left > 0,
            Some(b'+') => self.new_left > 0,
            Some(_) => false,
        }
    }

    fn push(&mut self, output: usize, line: &'a str) {
        let side = match line.as_bytes().first() {
            Some(b' ') | None => {
                self.old_left = self.old_left.saturating_sub(1);
                self.new_left = self.new_left.saturating_sub(1);
                DiffSide::Both
            }
            Some(b'-') => {
                self.old_left = self.old_left.saturating_sub(1);
                DiffSide::Old
            }
            Some(b'+') => {
                self.new_left = self.new_left.saturating_sub(1);
                DiffSide::New
            }
            Some(_) => DiffSide::Neither,
        };
        self.rows.push(Row {
            output,
            side,
            content: line.get(1..).unwrap_or(""),
            shift: 1,
        });
    }
}

/// The row count of a `start[,length]` hunk range; a bare start means one.
fn range_length(range: &str) -> Option<u32> {
    match range.split_once(',') {
        Some((start, length)) => {
            start.parse::<u32>().ok()?;
            length.parse().ok()
        }
        None => range.parse::<u32>().ok().map(|_| 1),
    }
}

/// The path of a `---`/`+++` header; `/dev/null` is an absent side.
fn header_path(raw: &str) -> Option<&str> {
    let path = raw.split('\t').next().unwrap_or(raw);
    let path = unquote(path.trim_end());
    if path == "/dev/null" {
        return None;
    }
    Some(
        path.strip_prefix("a/")
            .or_else(|| path.strip_prefix("b/"))
            .unwrap_or(path),
    )
}

/// Fallback paths from `diff --git a/<old> b/<new>` for header-less files.
fn git_header_paths(rest: &str) -> (Option<&str>, Option<&str>) {
    let rest = rest.trim_end();
    match rest
        .strip_prefix("a/")
        .and_then(|tail| tail.split_once(" b/"))
    {
        Some((old, new)) => (Some(old), Some(new)),
        None => (None, None),
    }
}

fn unquote(path: &str) -> &str {
    path.strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .unwrap_or(path)
}
