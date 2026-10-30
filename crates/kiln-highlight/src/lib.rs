//! Pure, bounded syntax highlighting for Kiln code views.
//!
//! Given a language (or a path) and text, this crate returns per-line token
//! spans from pinned Tree-sitter grammars. Spans are UTF-16 code-unit offsets
//! within each line, sorted and non-overlapping, drawn from a fixed class
//! vocabulary. Nothing here performs I/O, reads a clock or touches the
//! environment; shells decide when to call it and how to attach the result.
//!
//! Every input is bounded by named limits. An input over a bound, binary
//! content, an unsupported language or a highlighter failure yields an
//! explicit [`Status::Skipped`] result with a [`Reason`] and no tokens; it is
//! never an error and never a partial token stream beyond a bound.

// r[impl kiln.highlight.core]

mod classes;
mod diff;
mod grammar;
mod lines;

use serde::{Serialize, Serializer};

pub use diff::{DiffFile, DiffRow, DiffSide, highlight_diff, highlight_diff_files};

/// Schema identifier carried by every serialized [`Highlight`].
pub const SCHEMA: &str = "kiln.highlight.v1";
/// Largest input, in bytes, that is highlighted.
pub const MAX_BYTES: usize = 512 * 1024;
/// Largest input, in lines, that is highlighted.
pub const MAX_LINES: usize = 20_000;
/// Most tokens one line may carry before the whole input is skipped.
pub const MAX_TOKENS_PER_LINE: usize = 1_024;
/// Most tokens one result may carry before the whole input is skipped.
pub const MAX_TOKENS: usize = 131_072;
/// Most files one diff may contain before the whole diff is skipped.
pub const MAX_DIFF_FILES: usize = 256;

/// Deterministic size bounds for one highlighting request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Bounds {
    pub max_bytes: usize,
    pub max_lines: usize,
    pub max_tokens_per_line: usize,
    pub max_tokens: usize,
    pub max_diff_files: usize,
}

impl Bounds {
    pub const DEFAULT: Self = Self {
        max_bytes: MAX_BYTES,
        max_lines: MAX_LINES,
        max_tokens_per_line: MAX_TOKENS_PER_LINE,
        max_tokens: MAX_TOKENS,
        max_diff_files: MAX_DIFF_FILES,
    };
}

impl Default for Bounds {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// A supported source language.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Rust,
    Javascript,
    Typescript,
    Tsx,
    Python,
    Nix,
    Nickel,
    Toml,
    Json,
    Yaml,
    Css,
    Html,
    Bash,
    Markdown,
    C,
    Go,
}

impl Language {
    pub const ALL: [Self; 16] = [
        Self::Rust,
        Self::Javascript,
        Self::Typescript,
        Self::Tsx,
        Self::Python,
        Self::Nix,
        Self::Nickel,
        Self::Toml,
        Self::Json,
        Self::Yaml,
        Self::Css,
        Self::Html,
        Self::Bash,
        Self::Markdown,
        Self::C,
        Self::Go,
    ];

    /// The serialized language name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Javascript => "javascript",
            Self::Typescript => "typescript",
            Self::Tsx => "tsx",
            Self::Python => "python",
            Self::Nix => "nix",
            Self::Nickel => "nickel",
            Self::Toml => "toml",
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Css => "css",
            Self::Html => "html",
            Self::Bash => "bash",
            Self::Markdown => "markdown",
            Self::C => "c",
            Self::Go => "go",
        }
    }
}

/// The fixed token class vocabulary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Class {
    Keyword,
    String,
    Comment,
    Number,
    Constant,
    Type,
    Function,
    Macro,
    Attribute,
    Property,
    Variable,
    Operator,
    Punctuation,
    Tag,
    Escape,
    Heading,
    Link,
}

impl Class {
    /// The serialized class name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Keyword => "keyword",
            Self::String => "string",
            Self::Comment => "comment",
            Self::Number => "number",
            Self::Constant => "constant",
            Self::Type => "type",
            Self::Function => "function",
            Self::Macro => "macro",
            Self::Attribute => "attribute",
            Self::Property => "property",
            Self::Variable => "variable",
            Self::Operator => "operator",
            Self::Punctuation => "punctuation",
            Self::Tag => "tag",
            Self::Escape => "escape",
            Self::Heading => "heading",
            Self::Link => "link",
        }
    }
}

/// Whether tokens were produced.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Highlighted,
    Skipped,
}

/// Why an input was not highlighted.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// No pinned grammar covers the language or path.
    UnsupportedLanguage,
    /// The content contains NUL bytes or is not UTF-8.
    Binary,
    /// The input or its token stream exceeds a byte, line or token bound.
    TooLarge,
    /// The highlighter failed on this input.
    ParseError,
    /// A diff holds more files than [`Bounds::max_diff_files`], so only a
    /// truncated part of it could be highlighted; none of it is.
    TruncatedInput,
}

/// One span `[start, end)` of a line in UTF-16 code units.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Token {
    start: u32,
    end: u32,
    class: Class,
}

impl Token {
    pub const fn start(self) -> u32 {
        self.start
    }
    pub const fn end(self) -> u32 {
        self.end
    }
    pub const fn class(self) -> Class {
        self.class
    }
}

impl Serialize for Token {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (self.start, self.end, self.class).serialize(serializer)
    }
}

/// A `kiln.highlight.v1` result.
///
/// For text, `lines[i]` holds the tokens of line `i` of the text split on
/// `\n`; a trailing `\n` does not start another line. Skipped results carry
/// no lines.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Highlight {
    schema: &'static str,
    language: Option<Language>,
    status: Status,
    reason: Option<Reason>,
    lines: Vec<Vec<Token>>,
}

impl Highlight {
    /// An explicit skip with no tokens.
    pub const fn skipped(language: Option<Language>, reason: Reason) -> Self {
        Self {
            schema: SCHEMA,
            language,
            status: Status::Skipped,
            reason: Some(reason),
            lines: Vec::new(),
        }
    }

    const fn highlighted(language: Option<Language>, lines: Vec<Vec<Token>>) -> Self {
        Self {
            schema: SCHEMA,
            language,
            status: Status::Highlighted,
            reason: None,
            lines,
        }
    }

    pub const fn language(&self) -> Option<Language> {
        self.language
    }
    pub const fn status(&self) -> Status {
        self.status
    }
    pub const fn reason(&self) -> Option<Reason> {
        self.reason
    }
    pub fn lines(&self) -> &[Vec<Token>] {
        &self.lines
    }
}

/// Detect a supported language from a repository path's file name.
pub fn language_for_path(path: &str) -> Option<Language> {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name {
        "Cargo.lock" | "uv.lock" | "poetry.lock" => return Some(Language::Toml),
        "flake.lock" | "composer.lock" => return Some(Language::Json),
        ".bashrc" | ".bash_profile" | ".profile" | ".envrc" | "PKGBUILD" => {
            return Some(Language::Bash);
        }
        _ => {}
    }
    let (stem, extension) = name.rsplit_once('.')?;
    if stem.is_empty() {
        return None;
    }
    Some(match extension.to_ascii_lowercase().as_str() {
        "rs" => Language::Rust,
        "js" | "mjs" | "cjs" | "jsx" => Language::Javascript,
        "ts" | "mts" | "cts" => Language::Typescript,
        "tsx" => Language::Tsx,
        "py" | "pyi" => Language::Python,
        "nix" => Language::Nix,
        "ncl" => Language::Nickel,
        "toml" => Language::Toml,
        "json" | "jsonc" => Language::Json,
        "yaml" | "yml" => Language::Yaml,
        "css" => Language::Css,
        "html" | "htm" => Language::Html,
        "sh" | "bash" => Language::Bash,
        "md" | "markdown" => Language::Markdown,
        "c" | "h" => Language::C,
        "go" => Language::Go,
        _ => return None,
    })
}

// r[impl kiln.highlight.core.scenario.skipped]
/// Highlight one text in one language.
///
/// The checks run in a fixed order: byte bound, binary content, language
/// support, line bound, then the highlighter and its token bounds.
pub fn highlight(language: Option<Language>, text: &[u8], bounds: &Bounds) -> Highlight {
    if text.len() > bounds.max_bytes {
        return Highlight::skipped(language, Reason::TooLarge);
    }
    let Some(text) = textual(text) else {
        return Highlight::skipped(language, Reason::Binary);
    };
    let Some(language) = language else {
        return Highlight::skipped(None, Reason::UnsupportedLanguage);
    };
    let count = lines::count(text);
    if count > bounds.max_lines {
        return Highlight::skipped(Some(language), Reason::TooLarge);
    }
    let mut budget = bounds.max_tokens;
    match grammar::highlight_lines(language, text, count, bounds, &mut budget) {
        Ok(lines) => Highlight::highlighted(Some(language), lines),
        Err(reason) => Highlight::skipped(Some(language), reason),
    }
}

fn textual(bytes: &[u8]) -> Option<&str> {
    if bytes.contains(&0) {
        return None;
    }
    std::str::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests;
