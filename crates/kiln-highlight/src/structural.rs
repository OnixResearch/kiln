//! Opt-in syntax-aware ("structural") diff of one file's two blob versions.
//!
//! Both versions are parsed with the pinned grammar and reduced to leaf
//! tokens, so whitespace and line breaks never count as a change. Identical
//! top-level items pair first, in order; identical items that changed place
//! are reported as moved; every remaining token is diffed by identity. Lines
//! holding unmatched ("novel") tokens become rows anchored on the exact blob
//! line numbers, with context drawn only from lines paired through matched
//! tokens.
//!
//! Every input and every unit of matching work is bounded. Exceeding a bound,
//! binary content, an unsupported language, a parse error or an interruption
//! yields a [`StructuralOutcome::Fallback`] and never a partial result.

use std::cell::Cell;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::ops::{ControlFlow, Range};

use serde::Serialize;
use tree_sitter::{Node, ParseOptions, ParseState, Parser, Tree};

use crate::{Language, grammar, textual};

/// Largest blob, in bytes, diffed structurally on either side.
pub const MAX_STRUCTURAL_BYTES: usize = 256 * 1024;
/// Most leaf tokens one side may hold before the diff falls back.
pub const MAX_STRUCTURAL_NODES: usize = 40_000;
/// Most deterministic matching steps one diff may spend.
pub const MAX_STRUCTURAL_STEPS: usize = 4_000_000;
/// Unchanged lines shown before and after each changed region.
pub const STRUCTURAL_CONTEXT_LINES: usize = 3;

/// Matching polls the interruption flag at least this often, in steps.
const INTERRUPT_POLL_STEPS: usize = 1_024;
/// How many single-child wrappers below the root are unwrapped to find items.
///
/// Four levels reach the entries of a YAML mapping (stream, document, block
/// node, mapping); deeper wrappers keep their items whole.
const MAX_CONTAINER_DESCENT: usize = 4;
/// Node kinds containing any of these are one atomic token.
const ATOMIC_KIND_FRAGMENTS: [&str; 2] = ["string", "comment"];
/// Literal node kinds that are one atomic token without a matching fragment.
const ATOMIC_KINDS: [&str; 4] = ["char_literal", "rune_literal", "heredoc_body", "regex"];

/// Deterministic bounds for one structural diff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuralBounds {
    pub max_bytes: usize,
    pub max_nodes: usize,
    pub max_steps: usize,
}

impl StructuralBounds {
    pub const DEFAULT: Self = Self {
        max_bytes: MAX_STRUCTURAL_BYTES,
        max_nodes: MAX_STRUCTURAL_NODES,
        max_steps: MAX_STRUCTURAL_STEPS,
    };
}

impl Default for StructuralBounds {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Why a file was not diffed structurally; the caller shows the line diff.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StructuralFallback {
    UnsupportedLanguage,
    Binary,
    TooLarge,
    TooManyNodes,
    WorkBound,
    Interrupted,
    ParseError,
}

/// Which blob version a row's line belongs to.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RowSide {
    Old,
    New,
    Both,
}

/// One displayed line.
///
/// `old_line`/`new_line` are 1-based blob line numbers; `novel` holds sorted,
/// disjoint `[start, end)` UTF-16 code-unit spans of novel tokens on the line.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StructuralRow {
    pub side: RowSide,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub text: String,
    pub novel: Vec<[u32; 2]>,
}

/// Changed regions close enough on the new side to share context.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StructuralHunk {
    pub rows: Vec<StructuralRow>,
}

/// An unchanged item that moved: 1-based inclusive line ranges on each side.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MovedBlock {
    pub old_start: u32,
    pub old_end: u32,
    pub new_start: u32,
    pub new_end: u32,
}

/// A complete structural diff of one file.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StructuralFile {
    pub language: Language,
    pub hunks: Vec<StructuralHunk>,
    pub moved: Vec<MovedBlock>,
    pub novel_old_tokens: usize,
    pub novel_new_tokens: usize,
}

/// A structural diff, or the reason the caller must fall back to lines.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum StructuralOutcome {
    Structural(StructuralFile),
    Fallback { reason: StructuralFallback },
}

// r[impl kiln.review.structural]
/// Diff two versions of one file by syntax.
///
/// `old`/`new` are full blob contents; `None` means the file is absent on
/// that side (added/deleted). `interrupted` is polled during parsing and
/// matching (the shell implements the wall-clock bound with it).
///
/// The checks run in a fixed order: language support, byte bound, binary
/// content, parsing (with interruption), parse errors, the token bound, then
/// matching under the step bound and interruption.
pub fn structural_diff(
    language: Option<Language>,
    old: Option<&[u8]>,
    new: Option<&[u8]>,
    bounds: &StructuralBounds,
    interrupted: &dyn Fn() -> bool,
) -> StructuralOutcome {
    match diff(language, old, new, bounds, interrupted) {
        Ok(file) => StructuralOutcome::Structural(file),
        Err(reason) => StructuralOutcome::Fallback { reason },
    }
}

fn diff(
    language: Option<Language>,
    old: Option<&[u8]>,
    new: Option<&[u8]>,
    bounds: &StructuralBounds,
    interrupted: &dyn Fn() -> bool,
) -> Result<StructuralFile, StructuralFallback> {
    let language = language.ok_or(StructuralFallback::UnsupportedLanguage)?;
    // Offsets are reported as `u32`, so a larger blob is too large whatever
    // the configured bound.
    let fits = |bytes: &[u8]| bytes.len() <= bounds.max_bytes && u32::try_from(bytes.len()).is_ok();
    if !old.is_none_or(fits) || !new.is_none_or(fits) {
        return Err(StructuralFallback::TooLarge);
    }
    let old = old
        .map(|bytes| textual(bytes).ok_or(StructuralFallback::Binary))
        .transpose()?;
    let new = new
        .map(|bytes| textual(bytes).ok_or(StructuralFallback::Binary))
        .transpose()?;
    let grammar =
        grammar::parser_language(language).ok_or(StructuralFallback::UnsupportedLanguage)?;
    let old_tree = old
        .map(|text| parse(&grammar, text, interrupted))
        .transpose()?;
    let new_tree = new
        .map(|text| parse(&grammar, text, interrupted))
        .transpose()?;
    if [&old_tree, &new_tree]
        .into_iter()
        .flatten()
        .any(|tree| tree.root_node().has_error())
    {
        return Err(StructuralFallback::ParseError);
    }
    let mut interner = Interner::default();
    let old = Side::new(
        old.unwrap_or_default(),
        old_tree.as_ref(),
        bounds.max_nodes,
        &mut interner,
    )?;
    let new = Side::new(
        new.unwrap_or_default(),
        new_tree.as_ref(),
        bounds.max_nodes,
        &mut interner,
    )?;
    let mut work = Work {
        remaining: bounds.max_steps,
        since_poll: 0,
        interrupted,
    };
    work.poll()?;
    let matching = Matching::new(&old, &new, &mut work)?;
    work.poll()?;
    Ok(render(language, &old, &new, &matching))
}

/// Parse one side, stopping when `interrupted` reports true.
fn parse(
    grammar: &tree_sitter::Language,
    text: &str,
    interrupted: &dyn Fn() -> bool,
) -> Result<Tree, StructuralFallback> {
    if interrupted() {
        return Err(StructuralFallback::Interrupted);
    }
    let mut parser = Parser::new();
    parser
        .set_language(grammar)
        .map_err(|_| StructuralFallback::ParseError)?;
    let cancelled = Cell::new(false);
    let mut progress = |_: &ParseState| {
        if interrupted() {
            cancelled.set(true);
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    };
    let bytes = text.as_bytes();
    let tree = parser.parse_with_options(
        &mut |offset, _| bytes.get(offset..).unwrap_or_default(),
        None,
        Some(ParseOptions::new().progress_callback(&mut progress)),
    );
    match tree {
        Some(tree) => Ok(tree),
        None if cancelled.get() => Err(StructuralFallback::Interrupted),
        None => Err(StructuralFallback::ParseError),
    }
}

/// Dense identities for `(node kind, exact text)` pairs across both sides.
#[derive(Default)]
struct Interner<'a> {
    ids: HashMap<(&'static str, &'a str), u32>,
}

impl<'a> Interner<'a> {
    fn id(&mut self, kind: &'static str, text: &'a str) -> Result<u32, StructuralFallback> {
        let next = u32::try_from(self.ids.len()).map_err(|_| StructuralFallback::TooManyNodes)?;
        Ok(*self.ids.entry((kind, text)).or_insert(next))
    }
}

/// One leaf or atomic token: an identity and its whitespace-trimmed bytes.
#[derive(Clone, Copy, Debug)]
struct Tok {
    id: u32,
    start: usize,
    end: usize,
}

/// One blob version: its lines, tokens and top-level items.
struct Side<'a> {
    text: &'a str,
    lines: Lines,
    tokens: Vec<Tok>,
    /// Token index ranges of the top-level items, in document order.
    items: Vec<Range<usize>>,
}

impl<'a> Side<'a> {
    fn new(
        text: &'a str,
        tree: Option<&Tree>,
        max_nodes: usize,
        interner: &mut Interner<'a>,
    ) -> Result<Self, StructuralFallback> {
        let (tokens, items) = match tree {
            Some(tree) => {
                let tokens = collect_tokens(tree.root_node(), text, max_nodes, interner)?;
                let items = collect_items(tree.root_node(), &tokens);
                (tokens, items)
            }
            None => (Vec::new(), Vec::new()),
        };
        Ok(Self {
            text,
            lines: Lines::new(text),
            tokens,
            items,
        })
    }

    fn ids(&self) -> Vec<u32> {
        self.tokens.iter().map(|token| token.id).collect()
    }

    /// 0-based first and last line a token covers.
    fn token_lines(&self, token: Tok) -> (usize, usize) {
        (
            self.lines.of(token.start),
            self.lines.of(token.end.saturating_sub(1)),
        )
    }

    /// 1-based inclusive line range of an item.
    fn item_lines(&self, item: &Range<usize>) -> (u32, u32) {
        let first = self
            .tokens
            .get(item.start)
            .map_or(0, |token| self.token_lines(*token).0);
        let last = item
            .end
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
            .map_or(first, |token| self.token_lines(*token).1);
        (line_number(first), line_number(last))
    }

    /// Whether any token covers each line.
    fn covered(&self) -> Vec<bool> {
        let mut covered = vec![false; self.lines.count()];
        for token in &self.tokens {
            let (first, last) = self.token_lines(*token);
            for line in covered.iter_mut().take(last.saturating_add(1)).skip(first) {
                *line = true;
            }
        }
        covered
    }
}

fn is_atomic(kind: &str) -> bool {
    ATOMIC_KIND_FRAGMENTS
        .iter()
        .any(|fragment| kind.contains(fragment))
        || ATOMIC_KINDS.contains(&kind)
}

/// Leaf tokens in document order; atomic literal nodes are one token each.
fn collect_tokens<'a>(
    root: Node<'_>,
    text: &'a str,
    max_nodes: usize,
    interner: &mut Interner<'a>,
) -> Result<Vec<Tok>, StructuralFallback> {
    let mut tokens = Vec::new();
    let mut cursor = root.walk();
    loop {
        let node = cursor.node();
        if !is_atomic(node.kind()) && cursor.goto_first_child() {
            continue;
        }
        let range = node.byte_range();
        let raw = text
            .get(range.clone())
            .ok_or(StructuralFallback::ParseError)?;
        let body = raw.trim();
        if !body.is_empty() {
            let start = range
                .start
                .saturating_add(raw.len().saturating_sub(raw.trim_start().len()));
            tokens.push(Tok {
                id: interner.id(node.kind(), body)?,
                start,
                end: start.saturating_add(body.len()),
            });
            if tokens.len() > max_nodes {
                return Err(StructuralFallback::TooManyNodes);
            }
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return Ok(tokens);
            }
        }
    }
}

/// Token ranges of the named children of the root, unwrapping single-child
/// wrappers such as a JSON document's one object.
fn collect_items(root: Node<'_>, tokens: &[Tok]) -> Vec<Range<usize>> {
    let mut container = root;
    for _ in 0..MAX_CONTAINER_DESCENT {
        if container.named_child_count() != 1 {
            break;
        }
        let Some(only) = container.named_child(0) else {
            break;
        };
        if only.named_child_count() == 0 || is_atomic(only.kind()) {
            break;
        }
        container = only;
    }
    let mut cursor = container.walk();
    container
        .named_children(&mut cursor)
        .filter_map(|child| {
            let start = tokens.partition_point(|token| token.start < child.start_byte());
            let end = tokens.partition_point(|token| token.start < child.end_byte());
            (start < end).then_some(start..end)
        })
        .collect()
}

/// Line starts of a text split on `\n`; a trailing `\n` starts no new line.
struct Lines {
    starts: Vec<usize>,
    len: usize,
}

impl Lines {
    fn new(text: &str) -> Self {
        let mut starts = Vec::new();
        if !text.is_empty() {
            starts.push(0);
        }
        starts.extend(
            text.bytes()
                .enumerate()
                .filter(|(_, byte)| *byte == b'\n')
                .map(|(index, _)| index.saturating_add(1))
                .filter(|start| *start < text.len()),
        );
        Self {
            starts,
            len: text.len(),
        }
    }

    fn count(&self) -> usize {
        self.starts.len()
    }

    /// 0-based line holding a byte offset.
    fn of(&self, byte: usize) -> usize {
        self.starts
            .partition_point(|start| *start <= byte)
            .saturating_sub(1)
    }

    /// Byte range of a line without its `\n`.
    fn range(&self, text: &str, line: usize) -> (usize, usize) {
        let start = self.starts.get(line).copied().unwrap_or(self.len);
        let mut end = self
            .starts
            .get(line.saturating_add(1))
            .copied()
            .unwrap_or(self.len);
        if end > start && text.as_bytes().get(end.saturating_sub(1)) == Some(&b'\n') {
            end = end.saturating_sub(1);
        }
        (start, end)
    }

    fn text<'t>(&self, text: &'t str, line: usize) -> &'t str {
        let (start, end) = self.range(text, line);
        text.get(start..end).unwrap_or_default()
    }
}

/// The matching step budget and interruption poll.
struct Work<'a> {
    remaining: usize,
    since_poll: usize,
    interrupted: &'a dyn Fn() -> bool,
}

impl Work<'_> {
    fn poll(&mut self) -> Result<(), StructuralFallback> {
        self.since_poll = 0;
        if (self.interrupted)() {
            return Err(StructuralFallback::Interrupted);
        }
        Ok(())
    }

    fn tick(&mut self) -> Result<(), StructuralFallback> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or(StructuralFallback::WorkBound)?;
        self.since_poll = self.since_poll.saturating_add(1);
        if self.since_poll >= INTERRUPT_POLL_STEPS {
            self.poll()?;
        }
        Ok(())
    }
}

/// Token pairings between the two sides.
struct Matching {
    old: Vec<Option<usize>>,
    new: Vec<Option<usize>>,
    /// Whether an old token belongs to a moved item.
    old_moved: Vec<bool>,
    moved: Vec<MovedBlock>,
}

impl Matching {
    fn new(
        old: &Side<'_>,
        new: &Side<'_>,
        work: &mut Work<'_>,
    ) -> Result<Self, StructuralFallback> {
        let mut matching = Self {
            old: vec![None; old.tokens.len()],
            new: vec![None; new.tokens.len()],
            old_moved: vec![false; old.tokens.len()],
            moved: Vec::new(),
        };
        let old_ids = old.ids();
        let new_ids = new.ids();
        let mut sequences: HashMap<&[u32], u32> = HashMap::new();
        let old_items = item_ids(&mut sequences, &old_ids, &old.items);
        let new_items = item_ids(&mut sequences, &new_ids, &new.items);

        let mut old_paired = vec![false; old.items.len()];
        let mut new_paired = vec![false; new.items.len()];
        for (old_item, new_item) in lcs(&old_items, &new_items, work)? {
            matching.pair_items(old, new, old_item, new_item, false);
            set(&mut old_paired, old_item);
            set(&mut new_paired, new_item);
        }

        let mut unpaired: BTreeMap<u32, VecDeque<usize>> = BTreeMap::new();
        for (index, id) in old_items.iter().enumerate() {
            if !old_paired.get(index).copied().unwrap_or(true) {
                unpaired.entry(*id).or_default().push_back(index);
            }
        }
        for (new_item, id) in new_items.iter().enumerate() {
            if new_paired.get(new_item).copied().unwrap_or(true) {
                continue;
            }
            let Some(old_item) = unpaired.get_mut(id).and_then(VecDeque::pop_front) else {
                continue;
            };
            work.tick()?;
            matching.pair_items(old, new, old_item, new_item, true);
            let (Some(old_range), Some(new_range)) =
                (old.items.get(old_item), new.items.get(new_item))
            else {
                continue;
            };
            let (old_start, old_end) = old.item_lines(old_range);
            let (new_start, new_end) = new.item_lines(new_range);
            matching.moved.push(MovedBlock {
                old_start,
                old_end,
                new_start,
                new_end,
            });
        }

        let old_rest = unmatched(&matching.old);
        let new_rest = unmatched(&matching.new);
        let pick = |ids: &[u32], rest: &[usize]| -> Vec<u32> {
            rest.iter()
                .filter_map(|index| ids.get(*index).copied())
                .collect()
        };
        let pairs = lcs(&pick(&old_ids, &old_rest), &pick(&new_ids, &new_rest), work)?;
        for (old_index, new_index) in pairs {
            if let (Some(old_token), Some(new_token)) =
                (old_rest.get(old_index), new_rest.get(new_index))
            {
                matching.pair(*old_token, *new_token, false);
            }
        }
        Ok(matching)
    }

    fn pair_items(
        &mut self,
        old: &Side<'_>,
        new: &Side<'_>,
        old_item: usize,
        new_item: usize,
        moved: bool,
    ) {
        let (Some(old_range), Some(new_range)) = (old.items.get(old_item), new.items.get(new_item))
        else {
            return;
        };
        for (old_token, new_token) in old_range.clone().zip(new_range.clone()) {
            self.pair(old_token, new_token, moved);
        }
    }

    fn pair(&mut self, old_token: usize, new_token: usize, moved: bool) {
        if let Some(slot) = self.old.get_mut(old_token) {
            *slot = Some(new_token);
        }
        if let Some(slot) = self.new.get_mut(new_token) {
            *slot = Some(old_token);
        }
        if moved {
            set(&mut self.old_moved, old_token);
        }
    }
}

/// Dense identities of items' exact token sequences, shared by both sides.
fn item_ids<'s>(
    sequences: &mut HashMap<&'s [u32], u32>,
    ids: &'s [u32],
    items: &[Range<usize>],
) -> Vec<u32> {
    items
        .iter()
        .map(|item| {
            let sequence = ids.get(item.clone()).unwrap_or_default();
            let next = u32::try_from(sequences.len()).unwrap_or(u32::MAX);
            *sequences.entry(sequence).or_insert(next)
        })
        .collect()
}

fn set(flags: &mut [bool], index: usize) {
    if let Some(flag) = flags.get_mut(index) {
        *flag = true;
    }
}

fn unmatched(pairs: &[Option<usize>]) -> Vec<usize> {
    pairs
        .iter()
        .enumerate()
        .filter(|(_, pair)| pair.is_none())
        .map(|(index, _)| index)
        .collect()
}

/// Index pairs of a longest common subsequence, ascending on both sides.
///
/// Common prefix and suffix are stripped, then Myers' greedy O(ND) search
/// runs on the rest. Every comparison and every diagonal visit is one step.
fn lcs(
    a: &[u32],
    b: &[u32],
    work: &mut Work<'_>,
) -> Result<Vec<(usize, usize)>, StructuralFallback> {
    let mut prefix: usize = 0;
    while let (Some(x), Some(y)) = (a.get(prefix), b.get(prefix)) {
        work.tick()?;
        if x != y {
            break;
        }
        prefix = prefix.saturating_add(1);
    }
    let mut suffix: usize = 0;
    loop {
        let (Some(a_index), Some(b_index)) = (
            a.len().checked_sub(suffix.saturating_add(1)),
            b.len().checked_sub(suffix.saturating_add(1)),
        ) else {
            break;
        };
        if a_index < prefix || b_index < prefix {
            break;
        }
        work.tick()?;
        if a.get(a_index) != b.get(b_index) {
            break;
        }
        suffix = suffix.saturating_add(1);
    }
    let a_end = a.len().saturating_sub(suffix);
    let b_end = b.len().saturating_sub(suffix);
    let a_mid = a.get(prefix..a_end).unwrap_or_default();
    let b_mid = b.get(prefix..b_end).unwrap_or_default();
    let mut pairs: Vec<(usize, usize)> = (0..prefix).map(|index| (index, index)).collect();
    pairs.extend(
        myers(a_mid, b_mid, work)?
            .into_iter()
            .map(|(x, y)| (x.saturating_add(prefix), y.saturating_add(prefix))),
    );
    pairs.extend(
        (0..suffix).map(|offset| (a_end.saturating_add(offset), b_end.saturating_add(offset))),
    );
    Ok(pairs)
}

/// Furthest-reaching x per diagonal `k_low, k_low + 2, …` after `d` edits.
///
/// Positions fit `u32`: tokens are non-empty and blobs are under 4 GiB.
struct Frontier {
    k_low: isize,
    xs: Vec<Option<u32>>,
}

impl Frontier {
    fn get(&self, k: isize) -> Option<usize> {
        let offset = k.checked_sub(self.k_low)?;
        if offset < 0 || offset % 2 != 0 {
            return None;
        }
        self.xs
            .get(usize::try_from(offset / 2).ok()?)
            .copied()
            .flatten()
            .and_then(|x| usize::try_from(x).ok())
    }
}

fn signed(value: usize) -> isize {
    isize::try_from(value).unwrap_or(isize::MAX)
}

/// `y = x - k` on diagonal `k`, if non-negative.
fn y_of(x: usize, k: isize) -> Option<usize> {
    usize::try_from(signed(x).checked_sub(k)?).ok()
}

/// Where diagonal `k` starts after one more edit, and the diagonal it came
/// from: the furthest valid of a deletion from `k - 1` or an insertion from
/// `k + 1`, preferring the insertion on a tie.
fn edit_start(prev: &Frontier, k: isize, n: usize, m: usize) -> Option<(usize, isize)> {
    let down = prev
        .get(k.saturating_add(1))
        .filter(|x| y_of(*x, k).is_some_and(|y| y <= m));
    let right = prev
        .get(k.saturating_sub(1))
        .map(|x| x.saturating_add(1))
        .filter(|x| *x <= n);
    match (down, right) {
        (Some(down), Some(right)) if right > down => Some((right, k.saturating_sub(1))),
        (Some(down), _) => Some((down, k.saturating_add(1))),
        (None, Some(right)) => Some((right, k.saturating_sub(1))),
        (None, None) => None,
    }
}

/// Follow equal elements from `(x, y)`, returning the final x.
fn snake(
    a: &[u32],
    b: &[u32],
    mut x: usize,
    mut y: usize,
    work: &mut Work<'_>,
) -> Result<usize, StructuralFallback> {
    while let (Some(left), Some(right)) = (a.get(x), b.get(y)) {
        work.tick()?;
        if left != right {
            break;
        }
        x = x.saturating_add(1);
        y = y.saturating_add(1);
    }
    Ok(x)
}

fn myers(
    a: &[u32],
    b: &[u32],
    work: &mut Work<'_>,
) -> Result<Vec<(usize, usize)>, StructuralFallback> {
    let (n, m) = (a.len(), b.len());
    if n == 0 || m == 0 {
        return Ok(Vec::new());
    }
    let target = signed(n).saturating_sub(signed(m));
    let mut trace = vec![Frontier {
        k_low: 0,
        xs: vec![u32::try_from(snake(a, b, 0, 0, work)?).ok()],
    }];
    let limit = signed(n.saturating_add(m));
    let mut d: isize = 0;
    while trace.last().and_then(|frontier| frontier.get(target)) != Some(n) {
        d = d.saturating_add(1);
        if d > limit {
            return Err(StructuralFallback::WorkBound);
        }
        let mut k_low = d.saturating_neg().max(signed(m).saturating_neg());
        if (k_low.saturating_add(d)) % 2 != 0 {
            k_low = k_low.saturating_add(1);
        }
        let k_high = d.min(signed(n));
        let Some(prev) = trace.last() else {
            break;
        };
        let width = usize::try_from(k_high.saturating_sub(k_low) / 2).unwrap_or_default();
        let mut xs = Vec::with_capacity(width.saturating_add(1));
        let mut k = k_low;
        while k <= k_high {
            work.tick()?;
            let x = match edit_start(prev, k, n, m) {
                Some((x, _)) => match y_of(x, k) {
                    Some(y) => Some(snake(a, b, x, y, work)?),
                    None => None,
                },
                None => None,
            };
            xs.push(x.and_then(|x| u32::try_from(x).ok()));
            k = k.saturating_add(2);
        }
        trace.push(Frontier { k_low, xs });
    }

    let mut pairs = Vec::new();
    let (mut x, mut y) = (n, m);
    for depth in (1..trace.len()).rev() {
        let Some(prev) = trace.get(depth.saturating_sub(1)) else {
            break;
        };
        let k = signed(x).saturating_sub(signed(y));
        let Some((start, prev_k)) = edit_start(prev, k, n, m) else {
            break;
        };
        while x > start && y > 0 {
            x = x.saturating_sub(1);
            y = y.saturating_sub(1);
            pairs.push((x, y));
        }
        let Some(prev_x) = prev.get(prev_k) else {
            break;
        };
        let Some(prev_y) = y_of(prev_x, prev_k) else {
            break;
        };
        (x, y) = (prev_x, prev_y);
    }
    while x > 0 && y > 0 {
        x = x.saturating_sub(1);
        y = y.saturating_sub(1);
        pairs.push((x, y));
    }
    pairs.reverse();
    Ok(pairs)
}

fn line_number(index: usize) -> u32 {
    u32::try_from(index.saturating_add(1)).unwrap_or(u32::MAX)
}

fn units(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// Novel tokens of one side grouped by 0-based line, as UTF-16 spans.
fn novel_lines(side: &Side<'_>, pairs: &[Option<usize>]) -> BTreeMap<usize, Vec<[u32; 2]>> {
    let mut bytes: BTreeMap<usize, Vec<(usize, usize)>> = BTreeMap::new();
    for (token, pair) in side.tokens.iter().zip(pairs) {
        if pair.is_some() {
            continue;
        }
        let (first, last) = side.token_lines(*token);
        for line in first..=last {
            let (line_start, line_end) = side.lines.range(side.text, line);
            let start = token.start.max(line_start);
            let end = token.end.min(line_end);
            let spans = bytes.entry(line).or_default();
            if start >= end {
                continue;
            }
            match spans.last_mut() {
                Some(previous) if previous.1 >= start => previous.1 = previous.1.max(end),
                _ => spans.push((start, end)),
            }
        }
    }
    bytes
        .into_iter()
        .map(|(line, spans)| {
            let (mut offset, _) = side.lines.range(side.text, line);
            let mut count = 0usize;
            let measure = |from: usize, to: usize| {
                side.text
                    .get(from..to)
                    .map_or(0, |part| part.encode_utf16().count())
            };
            let spans = spans
                .into_iter()
                .map(|(start, end)| {
                    count = count.saturating_add(measure(offset, start));
                    let span_start = count;
                    count = count.saturating_add(measure(start, end));
                    offset = end;
                    [units(span_start), units(count)]
                })
                .collect();
            (line, spans)
        })
        .collect()
}

/// A novel row placed on the new side's line axis.
///
/// `low..=high` are the new lines the row occupies; an old row occupies none
/// and sits just before `low` (so `high = low - 1`).
struct Placed {
    low: i64,
    high: i64,
    order: (i64, u8, usize),
    row: StructuralRow,
}

fn to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// One run of novel rows with no unchanged new line between them.
struct Region {
    low: i64,
    high: i64,
    old: Vec<StructuralRow>,
    new: Vec<StructuralRow>,
}

fn render(
    language: Language,
    old: &Side<'_>,
    new: &Side<'_>,
    matching: &Matching,
) -> StructuralFile {
    let old_novel = novel_lines(old, &matching.old);
    let new_novel = novel_lines(new, &matching.new);

    // Old line -> new line through the first in-place matched token on it.
    let mut old_to_new: Vec<Option<usize>> = vec![None; old.lines.count()];
    for (index, (token, pair)) in old.tokens.iter().zip(&matching.old).enumerate() {
        let (Some(pair), false) = (pair, matching.old_moved.get(index).copied().unwrap_or(true))
        else {
            continue;
        };
        let Some(new_token) = new.tokens.get(*pair) else {
            continue;
        };
        if let Some(slot @ None) = old_to_new.get_mut(old.lines.of(token.start)) {
            *slot = Some(new.lines.of(new_token.start));
        }
    }

    let old_novel_lines: Vec<usize> = old_novel.keys().copied().collect();
    let mut placed = Vec::new();
    let mut last_mapped: Option<usize> = None;
    let mut next_line = 0;
    for (line, novel) in old_novel {
        for mapped in old_to_new.iter().take(line).skip(next_line).flatten() {
            last_mapped = Some(*mapped);
        }
        next_line = line;
        // Before the new line this one maps to, else after the new line the
        // nearest preceding mapped old line maps to.
        let (low, phase, position) = match old_to_new.get(line).copied().flatten() {
            Some(mapped) => (
                to_i64(mapped).saturating_add(1),
                0,
                to_i64(mapped).saturating_add(1),
            ),
            None => {
                let after = last_mapped.map_or(0, |mapped| to_i64(mapped).saturating_add(1));
                (after.saturating_add(1), 2, after)
            }
        };
        placed.push(Placed {
            low,
            high: low.saturating_sub(1),
            order: (position, phase, line),
            row: StructuralRow {
                side: RowSide::Old,
                old_line: Some(line_number(line)),
                new_line: None,
                text: old.lines.text(old.text, line).to_owned(),
                novel,
            },
        });
    }
    for (line, novel) in new_novel {
        let position = to_i64(line).saturating_add(1);
        placed.push(Placed {
            low: position,
            high: position,
            order: (position, 1, line),
            row: StructuralRow {
                side: RowSide::New,
                old_line: None,
                new_line: Some(line_number(line)),
                text: new.lines.text(new.text, line).to_owned(),
                novel,
            },
        });
    }
    placed.sort_by_key(|row| row.order);

    let mut regions: Vec<Region> = Vec::new();
    for row in placed {
        let joins = regions
            .last()
            .is_some_and(|region| row.low <= region.high.saturating_add(1));
        if !joins {
            regions.push(Region {
                low: row.low,
                high: row.high,
                old: Vec::new(),
                new: Vec::new(),
            });
        }
        let Some(region) = regions.last_mut() else {
            continue;
        };
        region.high = region.high.max(row.high);
        match row.row.side {
            RowSide::Old => region.old.push(row.row),
            _ => region.new.push(row.row),
        }
    }

    let context = context_pairs(old, new, matching, &old_novel_lines);
    let reach = to_i64(STRUCTURAL_CONTEXT_LINES);
    let last_line = to_i64(new.lines.count());
    let next_lows: Vec<i64> = regions
        .iter()
        .skip(1)
        .map(|region| region.low)
        .chain([i64::MAX])
        .collect();
    let mut hunks: Vec<StructuralHunk> = Vec::new();
    let mut previous_high: Option<i64> = None;
    let mut emitted = 0i64;
    for (region, next_low) in regions.into_iter().zip(next_lows) {
        let separate = previous_high.is_none_or(|high| {
            region.low.saturating_sub(high).saturating_sub(1) > reach.saturating_mul(2)
        });
        if separate {
            hunks.push(StructuralHunk { rows: Vec::new() });
        }
        let Some(hunk) = hunks.last_mut() else {
            continue;
        };
        let mut push_context = |from: i64, to: i64, rows: &mut Vec<StructuralRow>| {
            for line in from.max(emitted.saturating_add(1)).max(1)..=to.min(last_line) {
                emitted = line;
                let Some(index) = usize::try_from(line.saturating_sub(1)).ok() else {
                    continue;
                };
                if let Some(Some(old_line)) = context.get(index) {
                    rows.push(StructuralRow {
                        side: RowSide::Both,
                        old_line: Some(line_number(*old_line)),
                        new_line: Some(line_number(index)),
                        text: new.lines.text(new.text, index).to_owned(),
                        novel: Vec::new(),
                    });
                }
            }
        };
        push_context(
            region.low.saturating_sub(reach),
            region.low.saturating_sub(1),
            &mut hunk.rows,
        );
        hunk.rows.extend(region.old);
        hunk.rows.extend(region.new);
        // Context after a region stops short of the next region, whose own
        // leading context continues from there.
        push_context(
            region.high.saturating_add(1),
            region
                .high
                .saturating_add(reach)
                .min(next_low.saturating_sub(1)),
            &mut hunk.rows,
        );
        previous_high = Some(region.high);
    }

    StructuralFile {
        language,
        hunks,
        moved: matching.moved.clone(),
        novel_old_tokens: matching.old.iter().filter(|pair| pair.is_none()).count(),
        novel_new_tokens: matching.new.iter().filter(|pair| pair.is_none()).count(),
    }
}

/// For each new line, the old line with byte-identical text it pairs with.
///
/// Lines pair through matched tokens; a line no token covers (such as a
/// blank line) pairs at the offset of the nearest preceding paired line when
/// that old line is also uncovered and identical. Old novel lines never pair.
fn context_pairs(
    old: &Side<'_>,
    new: &Side<'_>,
    matching: &Matching,
    old_novel: &[usize],
) -> Vec<Option<usize>> {
    let mut pairs: Vec<Option<usize>> = vec![None; new.lines.count()];
    let identical = |old_line: usize, new_line: usize| {
        old_line < old.lines.count()
            && old_novel.binary_search(&old_line).is_err()
            && old.lines.text(old.text, old_line) == new.lines.text(new.text, new_line)
    };
    for (new_token, pair) in new.tokens.iter().zip(&matching.new) {
        let Some(old_token) = pair.and_then(|index| old.tokens.get(index)) else {
            continue;
        };
        let (first, last) = new.token_lines(*new_token);
        let old_first = old.lines.of(old_token.start);
        for (offset, line) in (first..=last).enumerate() {
            let old_line = old_first.saturating_add(offset);
            if let Some(slot @ None) = pairs.get_mut(line)
                && identical(old_line, line)
            {
                *slot = Some(old_line);
            }
        }
    }
    let old_covered = old.covered();
    let new_covered = new.covered();
    let mut anchor: Option<(usize, usize)> = None;
    for (line, slot) in pairs.iter_mut().enumerate() {
        if let Some(old_line) = *slot {
            anchor = Some((line, old_line));
            continue;
        }
        if new_covered.get(line).copied().unwrap_or(true) {
            continue;
        }
        let Some((new_anchor, old_anchor)) = anchor else {
            continue;
        };
        let old_line = old_anchor.saturating_add(line.saturating_sub(new_anchor));
        if !old_covered.get(old_line).copied().unwrap_or(true) && identical(old_line, line) {
            *slot = Some(old_line);
        }
    }
    pairs
}
