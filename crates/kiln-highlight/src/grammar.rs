//! Pinned grammars and their highlight configurations, built once per process.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::sync::LazyLock;

use tree_sitter::{Query, QueryCursor, StreamingIterator};
use tree_sitter_highlight::{HighlightConfiguration, Highlighter};

use crate::classes::{self, Capture};
use crate::lines::{self, Span};
use crate::{Bounds, Language, Reason, Token};

/// A parser grammar: every public language plus internal injection targets.
#[derive(Clone, Copy, Debug)]
enum Grammar {
    Public(Language),
    MarkdownInline,
}

impl Grammar {
    fn all() -> impl Iterator<Item = Self> {
        Language::ALL
            .into_iter()
            .map(Self::Public)
            .chain([Self::MarkdownInline])
    }

    const fn index(self) -> usize {
        match self {
            Self::Public(language) => language as usize,
            Self::MarkdownInline => Language::ALL.len(),
        }
    }

    /// Resolve an injection, code-fence or marker-comment language name.
    fn injected(name: &str) -> Option<Self> {
        let lower = name
            .trim_matches(|c: char| c.is_whitespace() || matches!(c, '#' | '/' | '*'))
            .to_ascii_lowercase();
        let language = match lower.as_str() {
            "markdown_inline" => return Some(Self::MarkdownInline),
            "rust" | "rs" => Language::Rust,
            "javascript" | "js" | "jsx" | "mjs" | "cjs" => Language::Javascript,
            "typescript" | "ts" => Language::Typescript,
            "tsx" => Language::Tsx,
            "python" | "py" => Language::Python,
            "nix" => Language::Nix,
            "nickel" | "ncl" => Language::Nickel,
            "toml" => Language::Toml,
            "json" | "jsonc" => Language::Json,
            "yaml" | "yml" => Language::Yaml,
            "css" => Language::Css,
            "html" => Language::Html,
            "bash" | "sh" | "shell" => Language::Bash,
            "markdown" | "md" => Language::Markdown,
            "c" | "h" => Language::C,
            "go" | "golang" => Language::Go,
            _ => return None,
        };
        Some(Self::Public(language))
    }

    /// The grammar, its highlight query and its injection query.
    fn definition(self) -> (tree_sitter::Language, Cow<'static, str>, &'static str) {
        use tree_sitter_javascript as javascript;
        use tree_sitter_typescript as typescript;
        match self {
            // Rust's own injections re-parse every macro token tree as Rust,
            // tripling the cost for tokens the outer layer already classes.
            Self::Public(Language::Rust) => (
                tree_sitter_rust::LANGUAGE.into(),
                Cow::Borrowed(tree_sitter_rust::HIGHLIGHTS_QUERY),
                "",
            ),
            Self::Public(Language::Javascript) => (
                javascript::LANGUAGE.into(),
                Cow::Owned(
                    [javascript::HIGHLIGHT_QUERY, javascript::JSX_HIGHLIGHT_QUERY].join("\n"),
                ),
                javascript::INJECTIONS_QUERY,
            ),
            // TypeScript highlights extend the JavaScript ones; later patterns win.
            Self::Public(Language::Typescript) => (
                typescript::LANGUAGE_TYPESCRIPT.into(),
                Cow::Owned([javascript::HIGHLIGHT_QUERY, typescript::HIGHLIGHTS_QUERY].join("\n")),
                javascript::INJECTIONS_QUERY,
            ),
            Self::Public(Language::Tsx) => (
                typescript::LANGUAGE_TSX.into(),
                Cow::Owned(
                    [
                        javascript::HIGHLIGHT_QUERY,
                        javascript::JSX_HIGHLIGHT_QUERY,
                        typescript::HIGHLIGHTS_QUERY,
                    ]
                    .join("\n"),
                ),
                javascript::INJECTIONS_QUERY,
            ),
            Self::Public(Language::Python) => (
                tree_sitter_python::LANGUAGE.into(),
                Cow::Borrowed(tree_sitter_python::HIGHLIGHTS_QUERY),
                "",
            ),
            Self::Public(Language::Nix) => (
                tree_sitter_nix::LANGUAGE.into(),
                Cow::Borrowed(tree_sitter_nix::HIGHLIGHTS_QUERY),
                tree_sitter_nix::INJECTIONS_QUERY,
            ),
            // The Nickel crate ships its query file without exporting it.
            Self::Public(Language::Nickel) => (
                tree_sitter_nickel::LANGUAGE.into(),
                Cow::Borrowed(include_str!("../queries/nickel/highlights.scm")),
                "",
            ),
            Self::Public(Language::Toml) => (
                tree_sitter_toml_ng::LANGUAGE.into(),
                Cow::Borrowed(tree_sitter_toml_ng::HIGHLIGHTS_QUERY),
                "",
            ),
            Self::Public(Language::Json) => (
                tree_sitter_json::LANGUAGE.into(),
                Cow::Borrowed(tree_sitter_json::HIGHLIGHTS_QUERY),
                "",
            ),
            Self::Public(Language::Yaml) => (
                tree_sitter_yaml::LANGUAGE.into(),
                Cow::Borrowed(tree_sitter_yaml::HIGHLIGHTS_QUERY),
                "",
            ),
            Self::Public(Language::Css) => (
                tree_sitter_css::LANGUAGE.into(),
                Cow::Borrowed(tree_sitter_css::HIGHLIGHTS_QUERY),
                "",
            ),
            Self::Public(Language::Html) => (
                tree_sitter_html::LANGUAGE.into(),
                Cow::Borrowed(tree_sitter_html::HIGHLIGHTS_QUERY),
                tree_sitter_html::INJECTIONS_QUERY,
            ),
            Self::Public(Language::Bash) => (
                tree_sitter_bash::LANGUAGE.into(),
                Cow::Borrowed(tree_sitter_bash::HIGHLIGHT_QUERY),
                "",
            ),
            Self::Public(Language::Markdown) => (
                tree_sitter_md::LANGUAGE.into(),
                Cow::Borrowed(tree_sitter_md::HIGHLIGHT_QUERY_BLOCK),
                tree_sitter_md::INJECTION_QUERY_BLOCK,
            ),
            // Inline HTML and LaTeX injections would re-parse every paragraph.
            Self::MarkdownInline => (
                tree_sitter_md::INLINE_LANGUAGE.into(),
                Cow::Borrowed(tree_sitter_md::HIGHLIGHT_QUERY_INLINE),
                "",
            ),
            Self::Public(Language::C) => (
                tree_sitter_c::LANGUAGE.into(),
                Cow::Borrowed(tree_sitter_c::HIGHLIGHT_QUERY),
                "",
            ),
            Self::Public(Language::Go) => (
                tree_sitter_go::LANGUAGE.into(),
                Cow::Borrowed(tree_sitter_go::HIGHLIGHTS_QUERY),
                "",
            ),
        }
    }
}

/// Injected layers nest at most this deep below the requested language.
///
/// Two levels cover Markdown fenced blocks whose language injects again, such
/// as HTML with a script; deeper nesting renders with the outer classes.
const MAX_INJECTION_DEPTH: usize = 2;

/// One grammar's highlight configuration and injection query.
struct Entry {
    language: tree_sitter::Language,
    config: HighlightConfiguration,
    injections: Option<Query>,
    content: Option<u32>,
    language_capture: Option<u32>,
}

/// Every grammar, configured against one shared capture-name table.
struct Registry {
    entries: Vec<Option<Entry>>,
    captures: Vec<Capture>,
}

static REGISTRY: LazyLock<Registry> = LazyLock::new(Registry::build);

impl Registry {
    fn build() -> Self {
        let mut entries: Vec<Option<Entry>> = Grammar::all()
            .map(|grammar| {
                let (language, highlights, injections) = grammar.definition();
                // Injections are resolved by `spans`, never inside the
                // highlighter: its merged multi-layer event stream misnests
                // when an outer and an inner capture start at one offset.
                let config = HighlightConfiguration::new(
                    language.clone(),
                    format!("{grammar:?}"),
                    &highlights,
                    "",
                    "",
                )
                .ok()?;
                let injections = if injections.is_empty() {
                    None
                } else {
                    Some(Query::new(&language, injections).ok()?)
                };
                let content = injections
                    .as_ref()
                    .and_then(|query| query.capture_index_for_name("injection.content"));
                let language_capture = injections
                    .as_ref()
                    .and_then(|query| query.capture_index_for_name("injection.language"));
                Some(Entry {
                    language,
                    config,
                    injections,
                    content,
                    language_capture,
                })
            })
            .collect();
        let mut seen = BTreeSet::new();
        let mut names: Vec<String> = Vec::new();
        for entry in entries.iter().flatten() {
            for name in entry.config.names() {
                if seen.insert(*name) {
                    names.push((*name).to_owned());
                }
            }
        }
        for entry in entries.iter_mut().flatten() {
            entry.config.configure(&names);
        }
        let captures = names.iter().map(|name| classes::capture(name)).collect();
        Self { entries, captures }
    }

    fn entry(&self, grammar: Grammar) -> Option<&Entry> {
        self.entries.get(grammar.index()).and_then(Option::as_ref)
    }

    /// Classed byte spans of `source`, with injected languages overlaid.
    fn spans(
        &self,
        highlighter: &mut Highlighter,
        grammar: Grammar,
        source: &str,
        depth: usize,
    ) -> Result<Vec<Span>, Reason> {
        let entry = self.entry(grammar).ok_or(Reason::ParseError)?;
        let events = highlighter
            .highlight(&entry.config, source.as_bytes(), None, |_| None)
            .map_err(|_| Reason::ParseError)?;
        let spans = lines::spans(events, &self.captures)?;
        let Some(query) = entry
            .injections
            .as_ref()
            .filter(|_| depth < MAX_INJECTION_DEPTH)
        else {
            return Ok(spans);
        };
        let parser = highlighter.parser();
        parser
            .set_language(&entry.language)
            .map_err(|_| Reason::ParseError)?;
        let tree = parser.parse(source, None).ok_or(Reason::ParseError)?;
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), source.as_bytes());
        let mut inner = Vec::new();
        while let Some(found) = matches.next() {
            let captured = found
                .captures
                .iter()
                .find(|capture| Some(capture.index) == entry.language_capture)
                .and_then(|capture| source.get(capture.node.byte_range()));
            let set = query
                .property_settings(found.pattern_index)
                .iter()
                .find(|property| &*property.key == "injection.language")
                .and_then(|property| property.value.as_deref());
            let Some(injected) = captured.or(set).and_then(Grammar::injected) else {
                continue;
            };
            for capture in found
                .captures
                .iter()
                .filter(|capture| Some(capture.index) == entry.content)
            {
                let range = capture.node.byte_range();
                let Some(text) = source.get(range.clone()) else {
                    continue;
                };
                // An injection that fails renders with the outer classes.
                if let Ok(nested) = self.spans(highlighter, injected, text, depth.saturating_add(1))
                {
                    inner.extend(nested.into_iter().map(|span| span.shifted(range.start)));
                }
            }
        }
        Ok(lines::overlay(spans, inner))
    }
}

/// Whether every pinned grammar and query built successfully.
#[cfg(test)]
pub(crate) fn all_configured() -> Vec<(String, bool)> {
    let registry = &*REGISTRY;
    Grammar::all()
        .map(|grammar| (format!("{grammar:?}"), registry.entry(grammar).is_some()))
        .collect()
}

/// Highlight a whole text, returning `count` lines of tokens.
pub(crate) fn highlight_lines(
    language: Language,
    text: &str,
    count: usize,
    bounds: &Bounds,
    budget: &mut usize,
) -> Result<Vec<Vec<Token>>, Reason> {
    let mut highlighter = Highlighter::new();
    let spans = REGISTRY.spans(&mut highlighter, Grammar::Public(language), text, 0)?;
    lines::collect(text, count, &spans, bounds, budget)
}
