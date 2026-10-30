//! Conversion of byte-range highlight events into per-line UTF-16 tokens.

use tree_sitter_highlight::{Error, HighlightEvent};

use crate::classes::Capture;
use crate::{Bounds, Class, Reason, Token};

/// Lines of `text` split on `\n`, where a trailing `\n` starts no new line.
pub(crate) fn count(text: &str) -> usize {
    let newlines = text.bytes().filter(|byte| *byte == b'\n').count();
    if text.is_empty() || text.ends_with('\n') {
        newlines
    } else {
        newlines.saturating_add(1)
    }
}

/// A classed byte range of one source text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Span {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) class: Class,
}

impl Span {
    pub(crate) fn shifted(self, offset: usize) -> Self {
        Self {
            start: self.start.saturating_add(offset),
            end: self.end.saturating_add(offset),
            class: self.class,
        }
    }
}

/// Classed spans from one layer's properly nested highlight events.
///
/// Nested captures resolve to the innermost one that maps to a class; a
/// reset capture makes nested text plain. A highlighter error is a
/// [`Reason::ParseError`].
pub(crate) fn spans(
    events: impl Iterator<Item = Result<HighlightEvent, Error>>,
    captures: &[Capture],
) -> Result<Vec<Span>, Reason> {
    let mut stack: Vec<Capture> = Vec::new();
    let mut spans: Vec<Span> = Vec::new();
    for event in events {
        match event.map_err(|_| Reason::ParseError)? {
            HighlightEvent::HighlightStart(highlight) => stack.push(
                captures
                    .get(highlight.0)
                    .copied()
                    .unwrap_or(Capture::Transparent),
            ),
            HighlightEvent::HighlightEnd => {
                stack.pop();
            }
            HighlightEvent::Source { start, end } => {
                if let Some(class) = effective(&stack) {
                    spans.push(Span { start, end, class });
                }
            }
        }
    }
    Ok(spans)
}

/// Overlay injected spans on outer spans: inner classes win where present.
///
/// Inner spans that overlap an earlier inner span are dropped, so the result
/// stays sorted and disjoint whenever the outer spans are.
pub(crate) fn overlay(outer: Vec<Span>, mut inner: Vec<Span>) -> Vec<Span> {
    if inner.is_empty() {
        return outer;
    }
    inner.sort_by_key(|span| (span.start, span.end));
    let mut covered = Vec::with_capacity(inner.len());
    let mut last_end = 0;
    for span in inner {
        if span.start >= last_end && span.start < span.end {
            last_end = span.end;
            covered.push(span);
        }
    }
    let mut result = Vec::with_capacity(outer.len().saturating_add(covered.len()));
    let mut first = 0;
    for span in outer {
        while covered
            .get(first)
            .is_some_and(|cover| cover.end <= span.start)
        {
            first = first.saturating_add(1);
        }
        let mut start = span.start;
        let mut next = first;
        while start < span.end {
            match covered.get(next) {
                Some(cover) if cover.start < span.end => {
                    if cover.start > start {
                        result.push(Span {
                            start,
                            end: cover.start,
                            class: span.class,
                        });
                    }
                    start = start.max(cover.end);
                    next = next.saturating_add(1);
                }
                _ => {
                    result.push(Span { start, ..span });
                    break;
                }
            }
        }
    }
    result.extend(covered);
    result.sort_by_key(|span| span.start);
    result
}

// r[impl kiln.highlight.core.scenario.offsets]
/// Split byte spans at line breaks into per-line UTF-16 tokens.
///
/// Adjacent spans of one class merge. Any out-of-order or out-of-range span
/// is a [`Reason::ParseError`], and any token beyond a bound is
/// [`Reason::TooLarge`], so a returned stream is always sorted,
/// non-overlapping and within its line.
pub(crate) fn collect(
    text: &str,
    count: usize,
    spans: &[Span],
    bounds: &Bounds,
    budget: &mut usize,
) -> Result<Vec<Vec<Token>>, Reason> {
    let mut sink = Sink {
        lines: vec![Vec::new(); count],
        bounds,
        budget,
    };
    let mut cursor = Cursor::default();
    for span in spans {
        cursor.walk(text, span.start, None, &mut sink)?;
        cursor.walk(text, span.end, Some(span.class), &mut sink)?;
    }
    Ok(sink.lines)
}

fn effective(stack: &[Capture]) -> Option<Class> {
    for capture in stack.iter().rev() {
        match capture {
            Capture::Class(class) => return Some(*class),
            Capture::Plain => return None,
            Capture::Transparent => {}
        }
    }
    None
}

#[derive(Default)]
struct Cursor {
    byte: usize,
    line: usize,
    column: u32,
}

struct Sink<'a> {
    lines: Vec<Vec<Token>>,
    bounds: &'a Bounds,
    budget: &'a mut usize,
}

impl Cursor {
    /// Advance to byte `to`, emitting `class` over the text passed.
    fn walk(
        &mut self,
        text: &str,
        to: usize,
        class: Option<Class>,
        sink: &mut Sink<'_>,
    ) -> Result<(), Reason> {
        if to < self.byte {
            return Err(Reason::ParseError);
        }
        let passed = text.get(self.byte..to).ok_or(Reason::ParseError)?;
        let mut start = self.column;
        for character in passed.chars() {
            if character == '\n' {
                if let Some(class) = class {
                    sink.push(self.line, start, self.column, class)?;
                }
                self.line = self.line.saturating_add(1);
                self.column = 0;
                start = 0;
            } else {
                let width = u32::try_from(character.len_utf16()).map_err(|_| Reason::TooLarge)?;
                self.column = self.column.checked_add(width).ok_or(Reason::TooLarge)?;
            }
        }
        if let Some(class) = class {
            sink.push(self.line, start, self.column, class)?;
        }
        self.byte = to;
        Ok(())
    }
}

impl Sink<'_> {
    fn push(&mut self, line: usize, start: u32, end: u32, class: Class) -> Result<(), Reason> {
        if start >= end {
            return Ok(());
        }
        let tokens = self.lines.get_mut(line).ok_or(Reason::ParseError)?;
        if let Some(last) = tokens.last_mut() {
            if last.end > start {
                return Err(Reason::ParseError);
            }
            if last.end == start && last.class == class {
                last.end = end;
                return Ok(());
            }
        }
        if tokens.len() >= self.bounds.max_tokens_per_line || *self.budget == 0 {
            return Err(Reason::TooLarge);
        }
        *self.budget -= 1;
        tokens.push(Token { start, end, class });
        Ok(())
    }
}
