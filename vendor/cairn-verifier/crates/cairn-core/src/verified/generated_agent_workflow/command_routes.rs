//! Test support: the command-route check over workflow descriptor text.
//!
//! Policy admission does not read descriptor text, so consumer policies load
//! whatever their workflows say. Cairn's tests run this scanner over the
//! shipped default policy, which consumers embed unchanged: every Cairn
//! command a shipped workflow tells the agent to run must resolve to a command
//! catalog entry in a class that workflow allows.

use super::model::{
    GeneratedAgentWorkflowPolicy, GeneratedWorkflowCommand, GeneratedWorkflowDescriptor,
};

/// A line's text after Markdown inline formatting is resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Piece {
    Word(String),
    /// The edge of a code span, link label, bracket, parenthesis or quote, or
    /// sentence punctuation. A command never continues across a break.
    Break,
}

/// Splits Markdown `text` into words and breaks.
///
/// - Code fence lines (three or more backticks or tildes) become a break, so
///   a fence info string such as `cairn` is never a word; fenced lines are
///   read.
/// - Paired emphasis markers (`*`, `**`, `_`, `__`) are dropped without a
///   break, so `**cairn** gate tasks` and `cairn **gate** tasks` read as
///   `cairn gate tasks`. An unpaired or intraword marker stays part of its
///   word, so `_cairn` and `snake_cairn` are not `cairn`.
/// - A code span's content is read; its edges are breaks.
/// - A link label is read; its edges are breaks and its destination or
///   reference is skipped.
/// - Parentheses, brackets, braces, quotes, and `.`, `,`, `:`, `;`, `!`, `?`
///   before whitespace or the end of a line are breaks.
pub(super) fn pieces(text: &str) -> Vec<Piece> {
    let mut pieces = Vec::new();
    for line in text.lines() {
        if is_code_fence_line(line) {
            pieces.push(Piece::Break);
        } else {
            line_pieces(line, &mut pieces);
        }
    }
    pieces
}

fn is_code_fence_line(line: &str) -> bool {
    let line = line.trim_start();
    let Some(marker) = line
        .chars()
        .next()
        .filter(|first| matches!(first, '`' | '~'))
    else {
        return false;
    };
    let run = line.len() - line.trim_start_matches(marker).len();
    run >= 3 && (marker == '~' || !line[run..].contains('`'))
}

/// Maximal runs of `marker` in `chars`, as `(start, length)`.
fn runs(chars: &[char], marker: impl Fn(char) -> bool) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if marker(chars[index]) {
            let start = index;
            while index < chars.len() && chars[index] == chars[start] {
                index += 1;
            }
            runs.push((start, index - start));
        } else {
            index += 1;
        }
    }
    runs
}

/// For each backtick run that opens a code span, the start of the run of
/// equal length that closes it. Each run is visited once per length.
fn code_spans(chars: &[char]) -> Vec<Option<usize>> {
    let mut closes = vec![None; chars.len()];
    let backticks = runs(chars, |character| character == '`');
    let mut by_length =
        std::collections::BTreeMap::<usize, std::collections::VecDeque<usize>>::new();
    for (index, &(_, length)) in backticks.iter().enumerate() {
        by_length.entry(length).or_default().push_back(index);
    }
    let mut next = 0;
    while next < backticks.len() {
        let (start, length) = backticks[next];
        let queue = by_length.entry(length).or_default();
        while queue.front().is_some_and(|&queued| queued <= next) {
            queue.pop_front();
        }
        match queue.pop_front() {
            Some(close) => {
                closes[start] = Some(backticks[close].0);
                next = close + 1;
            }
            None => next += 1,
        }
    }
    closes
}

/// Marks the characters of paired emphasis runs outside code spans.
fn paired_emphasis(chars: &[char], code: &[Option<usize>]) -> Vec<bool> {
    let mut strip = vec![false; chars.len()];
    let mut in_code = vec![false; chars.len()];
    let mut index = 0;
    while index < chars.len() {
        if let Some(close) = code[index] {
            let end = close + run_length(chars, index);
            in_code[index..end].fill(true);
            index = end;
        } else {
            index += 1;
        }
    }
    let punctuation =
        |character: Option<&char>| character.is_none_or(|character| !character.is_alphanumeric());
    let mut openers: [Vec<(usize, usize)>; 2] = [Vec::new(), Vec::new()];
    for (start, length) in runs(chars, |character| matches!(character, '*' | '_')) {
        if in_code[start] {
            continue;
        }
        let before = start.checked_sub(1).and_then(|index| chars.get(index));
        let after = chars.get(start + length);
        let opens =
            after.is_some_and(|character| !character.is_whitespace()) && punctuation(before);
        let closes =
            before.is_some_and(|character| !character.is_whitespace()) && punctuation(after);
        let stack = &mut openers[usize::from(chars[start] == '_')];
        if closes {
            if let Some((open, open_length)) = stack.pop() {
                strip[open..open + open_length].fill(true);
                strip[start..start + length].fill(true);
                continue;
            }
        }
        if opens {
            stack.push((start, length));
        }
    }
    strip
}

const BREAK_CHARACTERS: &[char] = &[
    '(', ')', '[', ']', '{', '}', '"', '\u{201c}', '\u{201d}', '\u{2018}', '\u{2019}',
];
const SENTENCE_PUNCTUATION: &[char] = &['.', ',', ':', ';', '!', '?'];

struct LinePieces<'a> {
    pieces: &'a mut Vec<Piece>,
    word: String,
}

impl LinePieces<'_> {
    fn flush(&mut self) {
        if !self.word.is_empty() {
            self.pieces
                .push(Piece::Word(std::mem::take(&mut self.word)));
        }
    }

    fn split(&mut self) {
        self.flush();
        if self.pieces.last() != Some(&Piece::Break) {
            self.pieces.push(Piece::Break);
        }
    }

    fn plain(&mut self, character: char, next: Option<char>) {
        let edge = next.is_none_or(|next| next.is_whitespace() || BREAK_CHARACTERS.contains(&next));
        // An apostrophe inside a word (`Cairn's`) is part of it; any other is
        // a quote.
        let quote =
            character == '\'' && (self.word.is_empty() || !next.is_some_and(char::is_alphanumeric));
        if character.is_whitespace() {
            self.flush();
        } else if BREAK_CHARACTERS.contains(&character)
            || quote
            || (SENTENCE_PUNCTUATION.contains(&character) && edge)
        {
            self.split();
        } else {
            self.word.push(character);
        }
    }
}

fn line_pieces(line: &str, pieces: &mut Vec<Piece>) {
    let chars = line.chars().collect::<Vec<_>>();
    let code = code_spans(&chars);
    let emphasis = paired_emphasis(&chars, &code);
    let mut out = LinePieces {
        pieces,
        word: String::new(),
    };
    let mut index = 0;
    while index < chars.len() {
        let character = chars[index];
        if let Some(close) = code[index] {
            let length = run_length(&chars, index);
            out.split();
            for position in index + length..close {
                out.plain(chars[position], chars.get(position + 1).copied());
            }
            out.split();
            index = close + length;
            continue;
        }
        if emphasis[index] {
            index += 1;
            continue;
        }
        if character == ']' {
            out.split();
            index += 1;
            // Skip an inline destination or a reference label.
            let closer = match chars.get(index) {
                Some('(') => Some(')'),
                Some('[') => Some(']'),
                _ => None,
            };
            if let Some(closer) = closer {
                if let Some(offset) = chars[index..].iter().position(|&next| next == closer) {
                    index += offset + 1;
                }
            }
            continue;
        }
        out.plain(character, chars.get(index + 1).copied());
        index += 1;
    }
    // A line break inside a paragraph is a space, so a command may wrap.
    out.flush();
}

fn run_length(chars: &[char], start: usize) -> usize {
    chars[start..]
        .iter()
        .take_while(|&&next| next == chars[start])
        .count()
}

/// A lowercase command word: ASCII letters, digits and hyphens, starting with
/// a letter. Placeholders (`CHANGE`, `<change>`) and options are not words.
pub(super) fn is_command_word(value: &str) -> bool {
    value
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

/// Catalog entries in a trie keyed by their invocation's command words: the
/// words after the leading `cairn`, up to the first placeholder or option.
#[derive(Default)]
pub(super) struct CommandTrie<'a> {
    children: std::collections::BTreeMap<String, CommandTrie<'a>>,
    commands: Vec<&'a GeneratedWorkflowCommand>,
}

pub(super) fn command_trie(policy: &GeneratedAgentWorkflowPolicy) -> CommandTrie<'_> {
    let mut root = CommandTrie::default();
    for command in &policy.command_catalog {
        let pieces = pieces(&command.invocation);
        let mut words = pieces.iter();
        if words.next() != Some(&Piece::Word("cairn".to_string())) {
            continue;
        }
        let mut node = &mut root;
        let mut depth = 0;
        for piece in words {
            let Piece::Word(word) = piece else { break };
            if !is_command_word(word) {
                break;
            }
            node = node.children.entry(word.clone()).or_default();
            depth += 1;
        }
        if depth > 0 {
            node.commands.push(command);
        }
    }
    root
}

/// A `cairn` mention followed by at least one command word.
#[derive(Debug)]
pub(super) enum Mention<'t, 'a> {
    /// The longest catalog command-word prefix of the mention and its entries.
    Resolved {
        words: String,
        commands: &'t [&'a GeneratedWorkflowCommand],
    },
    /// No catalog entry starts with the mention's first command word.
    Uncatalogued { word: String },
}

/// Calls `visit` for each `cairn` mention in `pieces`. Each mention walks the
/// trie one word at a time, so it reads at most the catalog's command depth
/// plus one following word. Returns the number of piece reads and trie steps.
pub(super) fn scan_mentions<'t, 'a>(
    pieces: &[Piece],
    trie: &'t CommandTrie<'a>,
    mut visit: impl FnMut(Mention<'t, 'a>),
) -> usize {
    let cairn = Piece::Word("cairn".to_string());
    let mut steps = 0;
    for (index, piece) in pieces.iter().enumerate() {
        steps += 1;
        if *piece != cairn {
            continue;
        }
        let Some(Piece::Word(first)) = pieces.get(index + 1) else {
            continue;
        };
        if !is_command_word(first) {
            continue;
        }
        let mut node = trie;
        let mut longest = None;
        let mut end = index + 1;
        while let Some(Piece::Word(word)) = pieces.get(end) {
            steps += 1;
            let Some(child) = node.children.get(word.as_str()) else {
                break;
            };
            node = child;
            end += 1;
            if !node.commands.is_empty() {
                longest = Some((end, node));
            }
        }
        visit(match longest {
            Some((end, node)) => Mention::Resolved {
                words: pieces[index + 1..end]
                    .iter()
                    .filter_map(|piece| match piece {
                        Piece::Word(word) => Some(word.as_str()),
                        Piece::Break => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" "),
                commands: &node.commands,
            },
            None => Mention::Uncatalogued {
                word: first.clone(),
            },
        });
    }
    steps
}

/// Every rendered text field of a descriptor with its field path.
pub(super) fn descriptor_text_fields<'d>(
    descriptor: &'d GeneratedWorkflowDescriptor,
    path: &str,
) -> Vec<(String, &'d str)> {
    let mut fields = [
        ("goal", &descriptor.goal),
        ("progress_rule", &descriptor.progress_rule),
        ("immediate_action", &descriptor.immediate_action),
        ("output_shape", &descriptor.output_shape),
    ]
    .into_iter()
    .map(|(field, text)| (format!("{path}.{field}"), text.as_str()))
    .collect::<Vec<_>>();
    for (field, values) in [
        ("success_evidence", &descriptor.success_evidence),
        ("required_evidence", &descriptor.required_evidence),
        ("constraints", &descriptor.constraints),
        ("non_claims", &descriptor.non_claims),
    ] {
        fields.extend(
            values
                .iter()
                .enumerate()
                .map(|(index, text)| (format!("{path}.{field}[{index}]"), text.as_str())),
        );
    }
    fields
}

/// A workflow text field that names a command outside its allowed classes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RouteViolation {
    pub(super) field_path: String,
    pub(super) message: String,
}

// r[impl cairn.generated_agent_workflows.command_routes]
// r[impl cairn.generated_agent_workflows.command_routes.permitted]
// r[impl cairn.generated_agent_workflows.command_routes.refused]
/// Every `cairn <command>` a descriptor's text names must resolve, by longest
/// command-word prefix, to catalog entries whose classes the descriptor
/// allows. Returns each distinct violation in field-path order.
pub(super) fn command_route_violations(
    policy: &GeneratedAgentWorkflowPolicy,
) -> Vec<RouteViolation> {
    let trie = command_trie(policy);
    let mut violations = std::collections::BTreeSet::new();
    for (index, descriptor) in policy.descriptors.iter().enumerate() {
        let path = format!("generated_agent_workflow_policy.descriptors[{index}]");
        for (field_path, text) in descriptor_text_fields(descriptor, &path) {
            scan_mentions(&pieces(text), &trie, |mention| match mention {
                Mention::Uncatalogued { word } => {
                    violations.insert(RouteViolation {
                        field_path: field_path.clone(),
                        message: format!(
                            "workflow {} names cairn {word}, which matches no command catalog invocation",
                            descriptor.id
                        ),
                    });
                }
                Mention::Resolved { words, commands } => {
                    for command in commands {
                        if !descriptor.command_classes.contains(&command.command_class) {
                            violations.insert(RouteViolation {
                                field_path: field_path.clone(),
                                message: format!(
                                    "workflow {} names cairn {words}, catalog command {} of class {}, which its command classes do not permit",
                                    descriptor.id, command.id, command.command_class
                                ),
                            });
                        }
                    }
                }
            });
        }
    }
    violations.into_iter().collect()
}

/// The catalog IDs `text` names, in order; an uncatalogued mention reads as
/// `uncatalogued:<word>`.
pub(super) fn named_commands(policy: &GeneratedAgentWorkflowPolicy, text: &str) -> Vec<String> {
    let trie = command_trie(policy);
    let mut named = Vec::new();
    scan_mentions(&pieces(text), &trie, |mention| match mention {
        Mention::Resolved { commands, .. } => {
            named.extend(commands.iter().map(|command| command.id.clone()));
        }
        Mention::Uncatalogued { word } => named.push(format!("uncatalogued:{word}")),
    });
    named
}
