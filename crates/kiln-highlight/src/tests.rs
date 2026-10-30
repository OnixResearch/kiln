use serde_json::{Value, json};
use tree_sitter_highlight::{Error, Highlight as Captured, HighlightEvent};

use crate::classes::Capture;
use crate::lines::Span;
use crate::{
    Bounds, Class, DiffFile, DiffRow, DiffSide, Highlight, Language, Reason, Status, Token,
    grammar, highlight, highlight_diff, highlight_diff_files, language_for_path, lines,
};

/// A language, a text, and the class each needle in it must show.
type Fixture = (Language, &'static str, &'static [(&'static str, Class)]);

/// One positive fixture per language.
const FIXTURES: &[Fixture] = &[
    (
        Language::Rust,
        "// note\nfn main() {\n    let s = \"hi\";\n    println!(\"{}\", 42);\n}\n",
        &[
            ("// note", Class::Comment),
            ("fn", Class::Keyword),
            ("let", Class::Keyword),
            ("\"hi\"", Class::String),
            ("println", Class::Macro),
            ("42", Class::Constant),
        ],
    ),
    (
        Language::Javascript,
        "// note\nconst x = 'a';\nfunction f() { return 1; }\n",
        &[
            ("// note", Class::Comment),
            ("const", Class::Keyword),
            ("'a'", Class::String),
            ("return", Class::Keyword),
            ("1", Class::Number),
        ],
    ),
    (
        Language::Typescript,
        "// note\ninterface A { b: string }\nconst x: A = { b: 'a' };\n",
        &[
            ("// note", Class::Comment),
            ("interface", Class::Keyword),
            ("const", Class::Keyword),
            ("'a'", Class::String),
            ("string", Class::Type),
        ],
    ),
    (
        Language::Tsx,
        "// note\nconst e = <div className=\"a\">hi</div>;\n",
        &[
            ("// note", Class::Comment),
            ("const", Class::Keyword),
            ("\"a\"", Class::String),
            ("div", Class::Tag),
        ],
    ),
    (
        Language::Python,
        "# note\ndef f():\n    return \"s\"\n",
        &[
            ("# note", Class::Comment),
            ("def", Class::Keyword),
            ("return", Class::Keyword),
            ("\"s\"", Class::String),
        ],
    ),
    (
        Language::Nix,
        "# note\n{ x = \"s\"; y = let a = 1; in a; }\n",
        &[
            ("# note", Class::Comment),
            ("\"s\"", Class::String),
            ("let", Class::Keyword),
            ("1", Class::Number),
        ],
    ),
    (
        Language::Nickel,
        "# note\nlet x = \"s\" in x\n",
        &[
            ("# note", Class::Comment),
            ("let", Class::Keyword),
            ("s", Class::String),
        ],
    ),
    (
        Language::Toml,
        "# note\n[a]\nb = \"s\"\nn = 1\n",
        &[
            ("# note", Class::Comment),
            ("\"s\"", Class::String),
            ("1", Class::Number),
        ],
    ),
    (
        Language::Json,
        "{\"a\": \"s\", \"n\": 1, \"t\": true}\n",
        &[
            ("\"s\"", Class::String),
            ("1", Class::Number),
            ("true", Class::Constant),
        ],
    ),
    (
        Language::Yaml,
        "# note\na: \"s\"\nb: true\n",
        &[
            ("# note", Class::Comment),
            ("\"s\"", Class::String),
            ("true", Class::Constant),
        ],
    ),
    (
        Language::Css,
        "/* note */\n@media screen { a { color: red; } }\n",
        &[
            ("/* note */", Class::Comment),
            ("@media", Class::Keyword),
            ("color", Class::Property),
        ],
    ),
    (
        Language::Html,
        "<!-- note -->\n<p class=\"x\">hi</p>\n<script>let y = 'z';</script>\n",
        &[
            ("<!-- note -->", Class::Comment),
            ("p", Class::Tag),
            ("class", Class::Attribute),
            ("'z'", Class::String),
            ("let", Class::Keyword),
        ],
    ),
    (
        Language::Bash,
        "# note\nif true; then echo \"s\"; fi\n",
        &[
            ("# note", Class::Comment),
            ("if", Class::Keyword),
            ("\"s\"", Class::String),
        ],
    ),
    (
        Language::Markdown,
        "# Title\n\nSee [docs](https://x.y) and `code`.\n\n```rust\nfn x() {}\n```\n",
        &[
            ("Title", Class::Heading),
            ("https://x.y", Class::Link),
            ("code", Class::String),
            ("fn", Class::Keyword),
        ],
    ),
    (
        Language::C,
        "/* note */\n#include <stdio.h>\nint main(void) { return 0; }\n",
        &[
            ("/* note */", Class::Comment),
            ("return", Class::Keyword),
            ("<stdio.h>", Class::String),
            ("0", Class::Number),
        ],
    ),
    (
        Language::Go,
        "// note\npackage main\nfunc f() string { return \"s\" }\n",
        &[
            ("// note", Class::Comment),
            ("func", Class::Keyword),
            ("\"s\"", Class::String),
        ],
    ),
];

fn utf16(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// The text a token addresses on one line.
fn addressed(line: &str, token: Token) -> String {
    let units = utf16(line);
    let start = usize::try_from(token.start()).unwrap_or(usize::MAX);
    let end = usize::try_from(token.end()).unwrap_or(usize::MAX);
    units
        .get(start..end)
        .map(String::from_utf16_lossy)
        .unwrap_or_default()
}

/// The class of the token that exactly covers the first `needle` in `text`.
fn class_of(result: &Highlight, text: &str, needle: &str) -> Option<Class> {
    for (index, line) in text.split('\n').enumerate() {
        let Some(byte) = line.find(needle) else {
            continue;
        };
        let start = u32::try_from(utf16(line.get(..byte).unwrap_or("")).len()).ok()?;
        let end = start.checked_add(u32::try_from(utf16(needle).len()).ok()?)?;
        let tokens = result.lines().get(index)?;
        return tokens
            .iter()
            .find(|token| token.start() <= start && end <= token.end())
            .map(|token| token.class());
    }
    None
}

/// Sorted, non-overlapping, non-empty tokens within each line's UTF-16 length.
fn assert_well_formed(result: &Highlight, lines: &[&str], label: &str) {
    if result.status() == Status::Skipped {
        assert!(result.lines().is_empty(), "{label}: skipped carries tokens");
        assert!(result.reason().is_some(), "{label}: skipped has no reason");
        return;
    }
    assert_eq!(result.lines().len(), lines.len(), "{label}: line count");
    for (tokens, line) in result.lines().iter().zip(lines) {
        let length = u32::try_from(utf16(line).len()).unwrap_or(u32::MAX);
        let mut previous_end = 0;
        for token in tokens {
            assert!(token.start() < token.end(), "{label}: empty {token:?}");
            assert!(token.start() >= previous_end, "{label}: overlap {token:?}");
            assert!(token.end() <= length, "{label}: {token:?} beyond {line:?}");
            previous_end = token.end();
        }
    }
}

fn text_lines(text: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    if text.is_empty() || text.ends_with('\n') {
        lines.pop();
    }
    lines
}

// r[verify kiln.highlight.core]
#[test]
fn every_pinned_grammar_and_query_builds() {
    let built = grammar::all_configured();
    assert_eq!(built.len(), Language::ALL.len() + 1);
    for (name, ok) in built {
        assert!(ok, "{name} highlight configuration failed to build");
    }
}

// r[verify kiln.highlight.core]
#[test]
fn every_language_highlights_its_keywords_strings_and_comments() {
    let covered: Vec<Language> = FIXTURES.iter().map(|fixture| fixture.0).collect();
    assert_eq!(covered, Language::ALL.to_vec(), "one fixture per language");
    let mut mismatches = Vec::new();
    for (language, text, expected) in FIXTURES {
        let result = highlight(Some(*language), text.as_bytes(), &Bounds::DEFAULT);
        assert_eq!(result.status(), Status::Highlighted, "{language:?}");
        assert_eq!(result.language(), Some(*language));
        for (needle, class) in *expected {
            let found = class_of(&result, text, needle);
            if found != Some(*class) {
                mismatches.push(format!("{language:?} {needle:?}: {found:?} != {class:?}"));
            }
        }
    }
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

// r[verify kiln.highlight.core]
#[test]
fn tokens_are_sorted_disjoint_and_within_their_lines_for_every_fixture() {
    let mut extra = vec![
        (
            Language::Rust,
            "/* open\ncomment */ fn x() {}\r\nlet é = \"😀\";",
        ),
        (
            Language::Markdown,
            "Setext\n===\n\n> quote *em* **strong**\n",
        ),
        (Language::Html, "<style>a { color: red }</style><b>x</b>"),
        (Language::Rust, ""),
        (Language::Rust, "\n\n"),
        (Language::Python, "def broken(:\n  'unterminated\n"),
    ];
    extra.extend(
        FIXTURES
            .iter()
            .map(|(language, text, _)| (*language, *text)),
    );
    for (language, text) in extra {
        let result = highlight(Some(language), text.as_bytes(), &Bounds::DEFAULT);
        assert_eq!(
            result.status(),
            Status::Highlighted,
            "{language:?} {text:?}"
        );
        assert_well_formed(
            &result,
            &text_lines(text),
            &format!("{language:?} {text:?}"),
        );
    }
}

// r[verify kiln.highlight.core.scenario.offsets]
#[test]
fn offsets_count_utf16_units_across_multibyte_and_astral_characters() {
    let text = "/* é😀 */ let s = \"ü😀x\"; // 𝄞 note\nfn ok() {}\n";
    let result = highlight(Some(Language::Rust), text.as_bytes(), &Bounds::DEFAULT);
    let first = text.split('\n').next().unwrap_or("");
    let tokens = result.lines().first().cloned().unwrap_or_default();
    let shown: Vec<(String, Class)> = tokens
        .iter()
        .map(|token| (addressed(first, *token), token.class()))
        .collect();
    assert!(
        shown.contains(&("/* é😀 */".to_owned(), Class::Comment)),
        "{shown:?}"
    );
    assert!(
        shown.contains(&("let".to_owned(), Class::Keyword)),
        "{shown:?}"
    );
    assert!(
        shown.contains(&("\"ü😀x\"".to_owned(), Class::String)),
        "{shown:?}"
    );
    assert!(
        shown.contains(&("// 𝄞 note".to_owned(), Class::Comment)),
        "{shown:?}"
    );
    // "/* é😀 */ " is 10 UTF-16 units but 13 bytes, so byte offsets would differ.
    let keyword = tokens
        .iter()
        .find(|token| token.class() == Class::Keyword)
        .copied();
    assert_eq!(
        keyword.map(|token| (token.start(), token.end())),
        Some((10, 13))
    );
    assert_well_formed(&result, &text_lines(text), "offsets");
}

// r[verify kiln.highlight.core.scenario.skipped]
#[test]
fn unsupported_binary_and_oversized_inputs_are_explicit_skips() {
    let skip = |result: Highlight, reason: Reason| {
        assert_eq!(result.status(), Status::Skipped);
        assert_eq!(result.reason(), Some(reason));
        assert!(result.lines().is_empty());
    };
    skip(
        highlight(None, b"plain text", &Bounds::DEFAULT),
        Reason::UnsupportedLanguage,
    );
    skip(
        highlight(Some(Language::Rust), b"fn\0main", &Bounds::DEFAULT),
        Reason::Binary,
    );
    skip(
        highlight(Some(Language::Rust), &[0x66, 0x6e, 0xff], &Bounds::DEFAULT),
        Reason::Binary,
    );
    let text = b"fn a() {}\nfn b() {}\n";
    let tight = |change: fn(&mut Bounds)| {
        let mut bounds = Bounds::DEFAULT;
        change(&mut bounds);
        bounds
    };
    skip(
        highlight(Some(Language::Rust), text, &tight(|b| b.max_bytes = 8)),
        Reason::TooLarge,
    );
    skip(
        highlight(Some(Language::Rust), text, &tight(|b| b.max_lines = 1)),
        Reason::TooLarge,
    );
    skip(
        highlight(
            Some(Language::Rust),
            text,
            &tight(|b| b.max_tokens_per_line = 2),
        ),
        Reason::TooLarge,
    );
    skip(
        highlight(Some(Language::Rust), text, &tight(|b| b.max_tokens = 3)),
        Reason::TooLarge,
    );
    // Exactly at the bounds still highlights.
    let exact = Bounds {
        max_bytes: text.len(),
        max_lines: 2,
        ..Bounds::DEFAULT
    };
    assert_eq!(
        highlight(Some(Language::Rust), text, &exact).status(),
        Status::Highlighted
    );
    // The default byte bound is 512 KiB.
    let large = "a".repeat(crate::MAX_BYTES + 1);
    skip(
        highlight(Some(Language::Rust), large.as_bytes(), &Bounds::DEFAULT),
        Reason::TooLarge,
    );
}

// r[verify kiln.highlight.core.scenario.skipped]
#[test]
fn highlighter_failures_and_inconsistent_spans_are_parse_errors() {
    let captures = [Capture::Class(Class::Keyword)];
    assert_eq!(
        lines::spans(vec![Err(Error::Unknown)].into_iter(), &captures),
        Err(Reason::ParseError)
    );
    let span = |start, end| Span {
        start,
        end,
        class: Class::Keyword,
    };
    let run = |spans: &[Span]| {
        let mut budget = usize::MAX;
        lines::collect("ab\ncd\n", 2, spans, &Bounds::DEFAULT, &mut budget)
    };
    assert_eq!(
        run(&[span(3, 4), span(0, 1)]),
        Err(Reason::ParseError),
        "out-of-order spans"
    );
    assert_eq!(
        run(&[span(0, 99)]),
        Err(Reason::ParseError),
        "beyond the text"
    );
    let events = vec![
        Ok(HighlightEvent::HighlightStart(Captured(0))),
        Ok(HighlightEvent::Source { start: 1, end: 4 }),
        Ok(HighlightEvent::HighlightEnd),
    ];
    let spans = lines::spans(events.into_iter(), &captures).unwrap_or_default();
    let split: Option<Vec<Vec<(u32, u32)>>> = run(&spans).ok().map(|lines| {
        lines
            .iter()
            .map(|line| line.iter().map(|t| (t.start(), t.end())).collect())
            .collect()
    });
    assert_eq!(
        split,
        Some(vec![vec![(1, 2)], vec![(0, 1)]]),
        "split at \\n"
    );
}

#[test]
fn injected_spans_replace_outer_classes_only_where_they_exist() {
    let span = |start, end, class| Span { start, end, class };
    let outer = vec![span(0, 10, Class::Heading), span(12, 14, Class::Comment)];
    let inner = vec![
        span(3, 5, Class::Link),
        span(4, 6, Class::String),
        span(9, 13, Class::Keyword),
    ];
    assert_eq!(
        lines::overlay(outer, inner),
        vec![
            span(0, 3, Class::Heading),
            span(3, 5, Class::Link),
            span(5, 9, Class::Heading),
            span(9, 13, Class::Keyword),
            span(13, 14, Class::Comment),
        ],
        "overlapping inner spans are dropped; outer gaps keep their class"
    );
}

#[test]
fn paths_select_languages_and_reject_unknown_names() {
    let cases = [
        ("src/main.rs", Some(Language::Rust)),
        ("web/app.MJS", Some(Language::Javascript)),
        ("a/b.ts", Some(Language::Typescript)),
        ("view.tsx", Some(Language::Tsx)),
        ("tool.py", Some(Language::Python)),
        ("flake.nix", Some(Language::Nix)),
        ("config.ncl", Some(Language::Nickel)),
        ("Cargo.toml", Some(Language::Toml)),
        ("Cargo.lock", Some(Language::Toml)),
        ("flake.lock", Some(Language::Json)),
        ("data.json", Some(Language::Json)),
        (".github/ci.yml", Some(Language::Yaml)),
        ("web.css", Some(Language::Css)),
        ("index.html", Some(Language::Html)),
        ("run.sh", Some(Language::Bash)),
        (".envrc", Some(Language::Bash)),
        ("README.md", Some(Language::Markdown)),
        ("lib.h", Some(Language::C)),
        ("main.go", Some(Language::Go)),
        ("notes.txt", None),
        ("Makefile", None),
        (".rs", None),
        ("archive.tar.gz", None),
        ("", None),
    ];
    for (path, expected) in cases {
        assert_eq!(language_for_path(path), expected, "{path}");
    }
}

#[test]
fn serialized_results_match_the_v1_shape() {
    let result = highlight(Some(Language::Rust), b"fn x() {}\n", &Bounds::DEFAULT);
    let value = serde_json::to_value(&result).unwrap_or(Value::Null);
    assert_eq!(value.get("schema"), Some(&json!("kiln.highlight.v1")));
    assert_eq!(value.get("language"), Some(&json!("rust")));
    assert_eq!(value.get("status"), Some(&json!("highlighted")));
    assert_eq!(value.get("reason"), Some(&Value::Null));
    assert_eq!(value.pointer("/lines/0/0"), Some(&json!([0, 2, "keyword"])));
    let skipped = serde_json::to_value(Highlight::skipped(None, Reason::TruncatedInput))
        .unwrap_or(Value::Null);
    assert_eq!(
        skipped,
        json!({"schema":"kiln.highlight.v1","language":null,"status":"skipped","reason":"truncated_input","lines":[]})
    );
}

const MIXED_DIFF: &str = "\
diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,2 +1,2 @@
 // keep
-fn old() {}
+fn new() { \"é😀\" }
diff --git a/notes.txt b/notes.txt
index 3333333..4444444 100644
--- a/notes.txt
+++ b/notes.txt
@@ -1,2 +1 @@
--- dashes that look like a header
 plain words
diff --git a/web/app.js b/web/app.js
new file mode 100644
index 0000000..5555555
--- /dev/null
+++ b/web/app.js
@@ -0,0 +1 @@
+const a = 'x';
diff --git a/tool.py b/tool.py
deleted file mode 100644
index 6666666..0000000
--- a/tool.py
+++ /dev/null
@@ -1 +0,0 @@
-def gone(): return \"s\"
diff --git a/old.js b/new.ts
similarity index 90%
rename from old.js
rename to new.ts
index 7777777..8888888 100644
--- a/old.js
+++ b/new.ts
@@ -1 +1 @@
-var v = 1;
+let v: number = 1;
\\ No newline at end of file
diff --git a/logo.png b/logo.png
index 9999999..aaaaaaa 100644
Binary files a/logo.png and b/logo.png differ
";

/// The class at the diff row starting with `prefix`, for `needle` in it.
fn diff_class(result: &Highlight, prefix: &str, needle: &str) -> Option<Class> {
    let (index, line) = MIXED_DIFF
        .split('\n')
        .enumerate()
        .find(|(_, line)| line.starts_with(prefix))?;
    let byte = line.find(needle)?;
    let start = u32::try_from(utf16(line.get(..byte)?).len()).ok()?;
    result
        .lines()
        .get(index)?
        .iter()
        .find(|token| token.start() <= start && start < token.end())
        .map(|token| token.class())
}

fn diff_tokens(result: &Highlight, prefix: &str) -> Vec<Token> {
    MIXED_DIFF
        .split('\n')
        .position(|line| line.starts_with(prefix))
        .and_then(|index| result.lines().get(index).cloned())
        .unwrap_or_default()
}

// r[verify kiln.highlight.core]
#[test]
fn unified_diffs_map_each_file_side_back_to_diff_rows() {
    let result = highlight_diff(MIXED_DIFF, &Bounds::DEFAULT);
    assert_eq!(result.status(), Status::Highlighted);
    assert_eq!(result.language(), None);
    assert_well_formed(&result, &text_lines(MIXED_DIFF), "mixed diff");
    // Modified Rust: context, deletion and addition, offsets after the prefix.
    assert_eq!(diff_class(&result, " // keep", "//"), Some(Class::Comment));
    assert_eq!(diff_class(&result, "-fn old", "fn"), Some(Class::Keyword));
    assert_eq!(diff_class(&result, "+fn new", "fn"), Some(Class::Keyword));
    assert_eq!(diff_class(&result, "+fn new", "\"é"), Some(Class::String));
    let added = diff_tokens(&result, "+fn new");
    assert_eq!(added.first().map(|token| token.start()), Some(1));
    let line = "+fn new() { \"é😀\" }";
    assert!(
        added
            .iter()
            .any(|token| addressed(line, *token) == "\"é😀\""),
        "{added:?}"
    );
    // Unsupported file in a mixed diff: no tokens, even on header-like rows.
    assert!(diff_tokens(&result, "--- dashes").is_empty());
    assert!(diff_tokens(&result, " plain words").is_empty());
    // Added file (old side /dev/null) and deleted file (new side /dev/null).
    assert_eq!(
        diff_class(&result, "+const a", "const"),
        Some(Class::Keyword)
    );
    assert_eq!(diff_class(&result, "+const a", "'x'"), Some(Class::String));
    assert_eq!(
        diff_class(&result, "-def gone", "def"),
        Some(Class::Keyword)
    );
    assert_eq!(
        diff_class(&result, "-def gone", "\"s\""),
        Some(Class::String)
    );
    // Rename across languages: each side uses its own path.
    assert_eq!(diff_class(&result, "-var v", "var"), Some(Class::Keyword));
    assert_eq!(diff_class(&result, "+let v", "number"), Some(Class::Type));
    // Headers, hunk lines, meta rows and binary files carry no tokens.
    for prefix in [
        "diff --git",
        "index ",
        "--- a/src",
        "+++ b/src",
        "@@ ",
        "rename from",
        "\\ No newline",
        "Binary files",
        "--- /dev/null",
        "+++ /dev/null",
    ] {
        assert!(diff_tokens(&result, prefix).is_empty(), "{prefix}");
    }
}

// r[verify kiln.highlight.core.scenario.skipped]
#[test]
fn diffs_without_highlightable_files_or_over_bounds_are_skipped() {
    let binary = "diff --git a/a.png b/a.png\nBinary files a/a.png and b/a.png differ\n";
    let result = highlight_diff(binary, &Bounds::DEFAULT);
    assert_eq!(
        (result.status(), result.reason()),
        (Status::Skipped, Some(Reason::Binary))
    );
    let unsupported = "--- a/x.txt\n+++ b/x.txt\n@@ -1 +1 @@\n-a\n+b\n";
    let result = highlight_diff(unsupported, &Bounds::DEFAULT);
    assert_eq!(
        (result.status(), result.reason()),
        (Status::Skipped, Some(Reason::UnsupportedLanguage))
    );
    assert!(result.lines().is_empty());
    let few_files = Bounds {
        max_diff_files: 5,
        ..Bounds::DEFAULT
    };
    let result = highlight_diff(MIXED_DIFF, &few_files);
    assert_eq!(result.reason(), Some(Reason::TruncatedInput));
    assert!(result.lines().is_empty());
    let few_tokens = Bounds {
        max_tokens: 4,
        ..Bounds::DEFAULT
    };
    assert_eq!(
        highlight_diff(MIXED_DIFF, &few_tokens).reason(),
        Some(Reason::TooLarge)
    );
    let few_bytes = Bounds {
        max_bytes: 64,
        ..Bounds::DEFAULT
    };
    assert_eq!(
        highlight_diff(MIXED_DIFF, &few_bytes).reason(),
        Some(Reason::TooLarge)
    );
    // An empty diff has nothing to skip.
    let empty = highlight_diff("", &Bounds::DEFAULT);
    assert_eq!(empty.status(), Status::Highlighted);
    assert!(empty.lines().is_empty());
}

// r[verify kiln.highlight.core]
#[test]
fn truncated_diffs_still_highlight_the_incomplete_last_file() {
    let cut = "--- a/a.rs\n+++ b/a.rs\n@@ -1,9 +1,9 @@\n fn a() {}\n-let x = 1;\n+let";
    let result = highlight_diff(cut, &Bounds::DEFAULT);
    assert_eq!(result.status(), Status::Highlighted);
    assert_eq!(result.lines().len(), 6);
    assert_well_formed(&result, &text_lines(cut), "truncated diff");
    assert_eq!(
        result
            .lines()
            .get(3)
            .and_then(|line| line.first())
            .map(|t| (t.start(), t.class())),
        Some((1, Class::Keyword))
    );
}

// r[verify kiln.highlight.core]
#[test]
fn structured_diffs_flatten_rows_with_unprefixed_offsets() {
    let row = |side, text| DiffRow { side, text };
    let files = [
        DiffFile {
            old_path: Some("src/a.rs"),
            new_path: Some("src/a.rs"),
            binary: false,
            hunks: vec![
                vec![
                    row(DiffSide::Both, "// keep\n"),
                    row(DiffSide::Old, "fn old() {}\n"),
                    row(DiffSide::New, "fn new() {}"),
                    row(DiffSide::Neither, "\n\\ No newline at end of file\n"),
                ],
                vec![row(DiffSide::New, "let é = \"😀\";\n")],
            ],
        },
        DiffFile {
            old_path: Some("image.png"),
            new_path: Some("image.png"),
            binary: true,
            hunks: Vec::new(),
        },
        DiffFile {
            old_path: None,
            new_path: Some("notes.txt"),
            binary: false,
            hunks: vec![vec![row(DiffSide::New, "words\n")]],
        },
    ];
    let result = highlight_diff_files(&files, &Bounds::DEFAULT);
    assert_eq!(result.status(), Status::Highlighted);
    let starts: Vec<Option<(u32, Class)>> = result
        .lines()
        .iter()
        .map(|line| line.first().map(|token| (token.start(), token.class())))
        .collect();
    assert_eq!(
        starts,
        vec![
            Some((0, Class::Comment)),
            Some((0, Class::Keyword)),
            Some((0, Class::Keyword)),
            None,
            Some((0, Class::Keyword)),
            None,
        ]
    );
    let string = result
        .lines()
        .get(4)
        .and_then(|line| line.iter().find(|token| token.class() == Class::String))
        .map(|token| addressed("let é = \"😀\";", *token));
    assert_eq!(string.as_deref(), Some("\"😀\""));
    let too_many = Bounds {
        max_diff_files: 2,
        ..Bounds::DEFAULT
    };
    assert_eq!(
        highlight_diff_files(&files, &too_many).reason(),
        Some(Reason::TruncatedInput)
    );
    let too_long = Bounds {
        max_lines: 5,
        ..Bounds::DEFAULT
    };
    assert_eq!(
        highlight_diff_files(&files, &too_long).reason(),
        Some(Reason::TooLarge)
    );
}
