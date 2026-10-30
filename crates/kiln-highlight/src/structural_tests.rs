use std::cell::Cell;

use serde_json::{Value, json};

use crate::{
    Language, MovedBlock, RowSide, StructuralBounds, StructuralFallback, StructuralFile,
    StructuralOutcome, StructuralRow, structural_diff,
};

const NEVER: &dyn Fn() -> bool = &|| false;

fn diff_with(
    language: Option<Language>,
    old: Option<&[u8]>,
    new: Option<&[u8]>,
    bounds: &StructuralBounds,
    interrupted: &dyn Fn() -> bool,
) -> StructuralOutcome {
    structural_diff(language, old, new, bounds, interrupted)
}

fn rust(old: Option<&str>, new: Option<&str>) -> StructuralOutcome {
    diff_with(
        Some(Language::Rust),
        old.map(str::as_bytes),
        new.map(str::as_bytes),
        &StructuralBounds::DEFAULT,
        NEVER,
    )
}

fn file(outcome: StructuralOutcome) -> StructuralFile {
    match outcome {
        StructuralOutcome::Structural(file) => file,
        StructuralOutcome::Fallback { reason } => unreachable!("unexpected fallback {reason:?}"),
    }
}

fn fallback(reason: StructuralFallback) -> StructuralOutcome {
    StructuralOutcome::Fallback { reason }
}

fn rows(file: &StructuralFile) -> Vec<&StructuralRow> {
    file.hunks.iter().flat_map(|hunk| &hunk.rows).collect()
}

/// A non-context row as `(side, old_line, new_line, novel)`.
type Changed = (RowSide, Option<u32>, Option<u32>, Vec<[u32; 2]>);

fn changed(file: &StructuralFile) -> Vec<Changed> {
    rows(file)
        .into_iter()
        .filter(|row| row.side != RowSide::Both)
        .map(|row| (row.side, row.old_line, row.new_line, row.novel.clone()))
        .collect()
}

fn units(text: &str) -> u32 {
    u32::try_from(text.encode_utf16().count()).unwrap_or(u32::MAX)
}

/// 1-based line of the first line containing `needle`: what a plain line
/// diff reports for a one-line change to that line.
fn line_of(text: &str, needle: &str) -> Option<u32> {
    let index = text.lines().position(|line| line.contains(needle))?;
    u32::try_from(index.checked_add(1)?).ok()
}

/// The UTF-16 span of `needle` within its line.
fn span_of(text: &str, needle: &str) -> Option<[u32; 2]> {
    let line = text.lines().find(|line| line.contains(needle))?;
    let start = units(line.get(..line.find(needle)?)?);
    Some([start, start.checked_add(units(needle))?])
}

/// Every context row repeats an old line's exact text.
fn context_is_exact(file: &StructuralFile, old: &str, new: &str) -> bool {
    let line = |text: &str, number: Option<u32>| {
        let index = usize::try_from(number?).ok()?.checked_sub(1)?;
        text.lines().nth(index).map(str::to_owned)
    };
    rows(file)
        .into_iter()
        .filter(|row| row.side == RowSide::Both)
        .all(|row| {
            row.novel.is_empty()
                && line(old, row.old_line).as_deref() == Some(row.text.as_str())
                && line(new, row.new_line).as_deref() == Some(row.text.as_str())
        })
}

const SWAP_OLD: &str = "fn alpha() {\n    one();\n}\n\nfn beta() {\n    two();\n}\n";
const SWAP_NEW: &str = "fn beta() {\n    two();\n}\n\nfn alpha() {\n    one();\n}\n";

const LARGER_OLD: &str = "// Greeting helpers.
use std::fmt;

fn first() -> u32 {
    1
}

fn label() -> String {
    let s = \"é😀\"; let v = 10;
    format!(\"{s}{v}\")
}

fn last() -> u32 {
    3
}
";

// r[verify kiln.review.structural]
#[test]
fn reformatting_only_is_not_a_change() {
    let old = "fn main() {\n    let x = foo(1, 2);\n    bar();\n}\n";
    let new = "fn main()\n{\n  let x = foo(\n    1, 2);\n  bar(); }\n";
    let result = file(rust(Some(old), Some(new)));
    assert_eq!(result.language, Language::Rust);
    assert!(result.hunks.is_empty());
    assert!(result.moved.is_empty());
    assert_eq!((result.novel_old_tokens, result.novel_new_tokens), (0, 0));

    // Negative control: a changed token is a change.
    let edited = "fn main()\n{\n  let x = foo(\n    1, 3);\n  bar(); }\n";
    let result = file(rust(Some(old), Some(edited)));
    assert_eq!((result.novel_old_tokens, result.novel_new_tokens), (1, 1));
}

// r[verify kiln.review.structural]
#[test]
fn swapped_functions_are_moved_without_rows() {
    let result = file(rust(Some(SWAP_OLD), Some(SWAP_NEW)));
    assert!(result.hunks.is_empty());
    assert_eq!((result.novel_old_tokens, result.novel_new_tokens), (0, 0));
    assert_eq!(
        result.moved,
        vec![MovedBlock {
            old_start: 1,
            old_end: 3,
            new_start: 5,
            new_end: 7,
        }]
    );

    // Negative control: unmoved identical text reports no move.
    let same = file(rust(Some(SWAP_OLD), Some(SWAP_OLD)));
    assert!(same.moved.is_empty());
    assert!(same.hunks.is_empty());
}

// r[verify kiln.review.structural]
#[test]
fn moved_function_with_an_edit_marks_only_the_edit() {
    let old = "fn alpha() {\n    say(\"hello\");\n}\n\nfn beta() {\n    two();\n}\n";
    let new = "fn beta() {\n    two();\n}\n\nfn alpha() {\n    say(\"howdy\");\n}\n";
    let result = file(rust(Some(old), Some(new)));
    assert!(result.moved.is_empty());
    assert_eq!((result.novel_old_tokens, result.novel_new_tokens), (1, 1));
    assert_eq!(
        changed(&result),
        vec![
            (
                RowSide::Old,
                line_of(old, "hello"),
                None,
                span_of(old, "\"hello\"").into_iter().collect(),
            ),
            (
                RowSide::New,
                None,
                line_of(new, "howdy"),
                span_of(new, "\"howdy\"").into_iter().collect(),
            ),
        ]
    );
    assert!(context_is_exact(&result, old, new));
}

// r[verify kiln.review.structural.scenario.anchors]
#[test]
fn single_line_edit_anchors_on_line_diff_lines() {
    // The new version also drops a blank line above the edit, so the edited
    // line has different numbers on each side.
    let new = LARGER_OLD
        .replacen("use std::fmt;\n\n", "use std::fmt;\n", 1)
        .replace("let v = 10;", "let v = 20;");
    let result = file(rust(Some(LARGER_OLD), Some(&new)));
    let old_line = line_of(LARGER_OLD, "let v = 10;");
    let new_line = line_of(&new, "let v = 20;");
    assert_eq!((old_line, new_line), (Some(9), Some(8)));
    let old_span = span_of(LARGER_OLD, "10");
    // "é" is one UTF-16 unit and "😀" two, so the span is not a char index.
    let prefix = "    let s = \"é😀\"; let v = ";
    assert_eq!(old_span, Some([units(prefix), units(prefix) + 2]));
    assert_ne!(
        old_span.map(|span| span[0]),
        u32::try_from(prefix.chars().count()).ok()
    );
    assert_eq!(
        changed(&result),
        vec![
            (RowSide::Old, old_line, None, old_span.into_iter().collect()),
            (
                RowSide::New,
                None,
                new_line,
                span_of(&new, "20").into_iter().collect()
            ),
        ]
    );
    let all = rows(&result);
    assert_eq!(result.hunks.len(), 1);
    assert!(context_is_exact(&result, LARGER_OLD, &new));
    // Context precedes the old row and follows the new row.
    assert_eq!(
        all.iter()
            .map(|row| (row.side, row.old_line, row.new_line))
            .collect::<Vec<_>>(),
        vec![
            (RowSide::Both, Some(6), Some(5)),
            (RowSide::Both, Some(7), Some(6)),
            (RowSide::Both, Some(8), Some(7)),
            (RowSide::Old, Some(9), None),
            (RowSide::New, None, Some(8)),
            (RowSide::Both, Some(10), Some(9)),
            (RowSide::Both, Some(11), Some(10)),
            (RowSide::Both, Some(12), Some(11)),
        ]
    );
}

// r[verify kiln.review.structural]
#[test]
fn hunks_split_only_beyond_twice_the_context() {
    let consts = |edits: &[usize]| -> String {
        (1..=20)
            .map(|index| {
                let value = if edits.contains(&index) { 0 } else { index };
                format!("const C{index}: u32 = {value};\n")
            })
            .collect()
    };
    let old = consts(&[]);
    // Six unchanged lines between the edits: one hunk.
    let near = consts(&[5, 12]);
    let result = file(rust(Some(&old), Some(&near)));
    assert_eq!(result.hunks.len(), 1);
    assert!(context_is_exact(&result, &old, &near));
    let lines: Vec<Option<u32>> = rows(&result).iter().map(|row| row.new_line).collect();
    assert_eq!(
        lines,
        [2, 3, 4]
            .into_iter()
            .map(Some)
            .chain([None, Some(5)])
            .chain((6..=11).map(Some))
            .chain([None, Some(12)])
            .chain((13..=15).map(Some))
            .collect::<Vec<_>>()
    );
    // Seven unchanged lines between the edits: two hunks.
    let far = consts(&[5, 13]);
    let result = file(rust(Some(&old), Some(&far)));
    assert_eq!(result.hunks.len(), 2);
    assert!(context_is_exact(&result, &old, &far));
}

// r[verify kiln.review.structural]
#[test]
fn added_and_deleted_files_are_wholly_novel() {
    let text = "fn main() {\n\n    let s = \"a\nb\";\n}\n";
    let non_empty = vec![Some(1), Some(3), Some(4), Some(5)];

    let added = file(rust(None, Some(text)));
    assert_eq!(added.novel_old_tokens, 0);
    assert!(added.novel_new_tokens > 0);
    let added_rows = rows(&added);
    assert!(added_rows.iter().all(|row| row.side == RowSide::New));
    assert_eq!(
        added_rows
            .iter()
            .map(|row| row.new_line)
            .collect::<Vec<_>>(),
        non_empty
    );
    assert_eq!(
        added_rows.first().map(|row| row.novel.clone()),
        Some(vec![[0, 2], [3, 9], [10, 11]])
    );

    let deleted = file(rust(Some(text), None));
    assert_eq!(deleted.novel_new_tokens, 0);
    assert_eq!(deleted.novel_old_tokens, added.novel_new_tokens);
    let deleted_rows = rows(&deleted);
    assert!(deleted_rows.iter().all(|row| row.side == RowSide::Old));
    assert_eq!(
        deleted_rows
            .iter()
            .map(|row| row.old_line)
            .collect::<Vec<_>>(),
        non_empty
    );

    let absent = file(rust(None, None));
    assert!(absent.hunks.is_empty());
    assert_eq!((absent.novel_old_tokens, absent.novel_new_tokens), (0, 0));
}

// r[verify kiln.review.structural.scenario.fallback]
#[test]
fn unsupported_language_falls_back_first() {
    let binary = b"\0\0";
    assert_eq!(
        diff_with(
            None,
            Some(binary),
            Some(b"x"),
            &StructuralBounds::DEFAULT,
            NEVER
        ),
        fallback(StructuralFallback::UnsupportedLanguage)
    );
}

// r[verify kiln.review.structural.scenario.fallback]
#[test]
fn oversized_side_falls_back() {
    let bounds = StructuralBounds {
        max_bytes: 8,
        ..StructuralBounds::DEFAULT
    };
    let small = b"fn a(){}";
    let large = b"fn main() {}\0";
    // The byte bound is checked before binary content.
    assert_eq!(
        diff_with(
            Some(Language::Rust),
            Some(small),
            Some(large),
            &bounds,
            NEVER
        ),
        fallback(StructuralFallback::TooLarge)
    );
    assert!(matches!(
        diff_with(Some(Language::Rust), Some(small), None, &bounds, NEVER),
        StructuralOutcome::Structural(_)
    ));
}

// r[verify kiln.review.structural.scenario.fallback]
#[test]
fn binary_side_falls_back() {
    let text = b"fn main() {}\n";
    for binary in [&b"fn main() {}\0"[..], &[0xff, 0xfe][..]] {
        assert_eq!(
            diff_with(
                Some(Language::Rust),
                Some(text),
                Some(binary),
                &StructuralBounds::DEFAULT,
                NEVER
            ),
            fallback(StructuralFallback::Binary)
        );
        assert_eq!(
            diff_with(
                Some(Language::Rust),
                Some(binary),
                None,
                &StructuralBounds::DEFAULT,
                NEVER
            ),
            fallback(StructuralFallback::Binary)
        );
    }
}

// r[verify kiln.review.structural.scenario.fallback]
#[test]
fn parse_error_falls_back() {
    assert_eq!(
        rust(Some("fn main() {}\n"), Some("fn main( {\n")),
        fallback(StructuralFallback::ParseError)
    );
    assert_eq!(
        rust(None, Some("fn main( {\n")),
        fallback(StructuralFallback::ParseError)
    );
}

// r[verify kiln.review.structural.scenario.fallback]
#[test]
fn too_many_tokens_falls_back() {
    let bounds = StructuralBounds {
        max_nodes: 6,
        ..StructuralBounds::DEFAULT
    };
    let run = |text: &str| {
        diff_with(
            Some(Language::Rust),
            Some(b"fn a() {}"),
            Some(text.as_bytes()),
            &bounds,
            NEVER,
        )
    };
    // `fn a() {}` is exactly six tokens.
    assert!(matches!(run("fn b() {}"), StructuralOutcome::Structural(_)));
    assert_eq!(
        run("fn b() { c }"),
        fallback(StructuralFallback::TooManyNodes)
    );
}

// r[verify kiln.review.structural.scenario.fallback]
#[test]
fn step_budget_falls_back() {
    let new = LARGER_OLD.replace("let v = 10;", "let v = 20;");
    let run = |max_steps: usize| {
        diff_with(
            Some(Language::Rust),
            Some(LARGER_OLD.as_bytes()),
            Some(new.as_bytes()),
            &StructuralBounds {
                max_steps,
                ..StructuralBounds::DEFAULT
            },
            NEVER,
        )
    };
    assert_eq!(run(1), fallback(StructuralFallback::WorkBound));
    assert!(matches!(
        run(StructuralBounds::DEFAULT.max_steps),
        StructuralOutcome::Structural(_)
    ));
}

// r[verify kiln.review.structural.scenario.fallback]
#[test]
fn interruption_falls_back() {
    let new = LARGER_OLD.replace("let v = 10;", "let v = 20;");
    let run = |interrupted: &dyn Fn() -> bool| {
        diff_with(
            Some(Language::Rust),
            Some(LARGER_OLD.as_bytes()),
            Some(new.as_bytes()),
            &StructuralBounds::DEFAULT,
            interrupted,
        )
    };
    assert_eq!(run(&|| true), fallback(StructuralFallback::Interrupted));
    // Interruption raised after parsing still discards the whole result.
    for after in 1..=3 {
        let polls = Cell::new(0usize);
        let late = || {
            polls.set(polls.get() + 1);
            polls.get() > after
        };
        assert_eq!(run(&late), fallback(StructuralFallback::Interrupted));
    }
    assert!(matches!(run(NEVER), StructuralOutcome::Structural(_)));
}

#[test]
fn outcomes_serialize_with_a_status_tag() {
    let new = SWAP_NEW.replace("one();", "uno();");
    let value = serde_json::to_value(rust(Some(SWAP_OLD), Some(&new))).unwrap_or(Value::Null);
    assert_eq!(value.get("status"), Some(&json!("structural")));
    assert_eq!(value.get("language"), Some(&json!("rust")));
    assert_eq!(value.get("novel_old_tokens"), Some(&json!(1)));
    assert_eq!(value.get("novel_new_tokens"), Some(&json!(1)));
    assert_eq!(value.get("moved"), Some(&json!([])));
    let rows = value
        .pointer("/hunks/0/rows")
        .cloned()
        .unwrap_or(Value::Null);
    let changed: Vec<&Value> = rows
        .as_array()
        .into_iter()
        .flatten()
        .filter(|row| row.get("side") != Some(&json!("both")))
        .collect();
    assert_eq!(
        changed,
        vec![
            &json!({"side": "old", "old_line": 2, "new_line": null, "text": "    one();", "novel": [[4, 7]]}),
            &json!({"side": "new", "old_line": null, "new_line": 6, "text": "    uno();", "novel": [[4, 7]]}),
        ]
    );
    assert_eq!(
        serde_json::to_value(fallback(StructuralFallback::TooManyNodes)).unwrap_or(Value::Null),
        json!({"status": "fallback", "reason": "too_many_nodes"})
    );
}
