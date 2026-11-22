//! Exact, source-bound selection over one already-admitted lifecycle artifact.
//! No filesystem, implicit search, or transitive prerequisite traversal occurs here.
use serde::{Deserialize, Serialize};

use super::{spec, task_ordering};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContextSelector {
    Requirement { id: String },
    Task { id: String },
    Section { path: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextReadBounds {
    pub max_artifact_bytes: usize,
    pub max_selected_bytes: usize,
    pub max_selector_bytes: usize,
    pub max_heading_depth: usize,
    pub max_diagnostics: usize,
    pub max_references: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextReference {
    pub kind: String,
    pub id: String,
    pub route: Option<String>,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextSpan {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextSelectionStatus {
    Selected,
    Missing,
    Unsupported,
    Ambiguous,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextSelection {
    pub status: ContextSelectionStatus,
    pub document_valid: bool,
    pub span: Option<ContextSpan>,
    pub text: Option<String>,
    pub diagnostics: Vec<String>,
    pub omitted_diagnostics: usize,
    pub diagnostics_route: Option<String>,
    pub references: Vec<ContextReference>,
    pub omitted_references: usize,
    pub references_route: Option<String>,
    pub completeness: String,
}

fn lines(markdown: &str) -> Vec<(usize, &str)> {
    let mut start = 0;
    markdown
        .split_inclusive('\n')
        .map(|line| {
            let at = start;
            start += line.len();
            (at, line)
        })
        .collect()
}

fn span(rows: &[(usize, &str)], first: usize, last: usize, total: usize) -> ContextSpan {
    ContextSpan {
        start_byte: rows[first].0,
        end_byte: rows.get(last).map_or(total, |line| line.0),
        start_line: first + 1,
        end_line: last,
    }
}

fn selector_valid(selector: &ContextSelector, bounds: &ContextReadBounds) -> bool {
    match selector {
        ContextSelector::Requirement { id } | ContextSelector::Task { id } => {
            !id.is_empty()
                && id.len() <= bounds.max_selector_bytes
                && id.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
                && id.as_bytes().last().is_some_and(u8::is_ascii_alphanumeric)
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        }
        ContextSelector::Section { path } => {
            !path.is_empty()
                && path.len() <= bounds.max_heading_depth
                && path.iter().map(String::len).sum::<usize>() <= bounds.max_selector_bytes
                && path.iter().all(|part| {
                    !part.is_empty()
                        && part.len() <= bounds.max_selector_bytes
                        && part.trim() == part
                        && !part.contains('\n')
                        && !part.contains('\r')
                })
        }
    }
}
fn definition_ids(heading: &str, body: &[(usize, &str)]) -> Vec<String> {
    use crate::verified::content_bound_requirement_registry::declaration_markers;
    let heading_ids = declaration_markers(heading);
    if !heading_ids.is_empty() {
        return heading_ids;
    }
    let mut fenced = false;
    let mut comment = false;
    for (_, line) in body {
        if spec::fence_boundary(line) && !comment {
            fenced = !fenced;
            continue;
        }
        let visible = spec::visible_text_without_comments(line, &mut comment);
        if fenced {
            continue;
        }
        let declaration = visible.trim_start();
        if declaration.starts_with("r[") {
            return declaration_markers(declaration);
        }
    }
    Vec::new()
}

fn requirement_definitions(
    rows: &[(usize, &str)],
    headings: &[spec::VisibleHeading],
    artifact: &str,
    diagnostics: &mut Vec<String>,
) -> Vec<(String, usize, usize)> {
    let mut definitions = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for heading in headings {
        if heading.level != 3 || !heading.text.starts_with("Requirement:") {
            continue;
        }
        let end =
            spec::next_visible_heading_at_or_above(headings, heading.line_index, 3, rows.len());
        let body_end = spec::next_visible_heading_at_or_above(headings, heading.line_index, 4, end);
        let ids = definition_ids(&heading.text, &rows[heading.line_index + 1..body_end]);
        if ids.len() != 1 {
            diagnostics.push(format!(
                "{}:{}: requirement declaration needs exactly one bare defining ID",
                artifact,
                heading.line_index + 1
            ));
            continue;
        }
        if !seen.insert(ids[0].clone()) {
            diagnostics.push(format!(
                "{}:{}: duplicate defining requirement ID {}",
                artifact,
                heading.line_index + 1,
                ids[0]
            ));
        }
        definitions.push((ids[0].clone(), heading.line_index, end));
    }
    definitions
}

fn visible_requirement_links(text: &str) -> std::collections::BTreeSet<String> {
    use crate::verified::content_bound_requirement_registry::declaration_markers;
    let mut links = std::collections::BTreeSet::new();
    let mut fenced = false;
    let mut comment = false;
    for line in text.lines() {
        if spec::fence_boundary(line) && !comment {
            fenced = !fenced;
            continue;
        }
        let visible = spec::visible_text_without_comments(line, &mut comment);
        if !fenced {
            links.extend(declaration_markers(&visible).into_iter().filter(|id| {
                id.contains('.')
                    && id.bytes().all(|b| {
                        b.is_ascii_lowercase()
                            || b.is_ascii_digit()
                            || matches!(b, b'.' | b'_' | b'-')
                    })
            }));
        }
    }
    links
}

fn task_block_end(rows: &[(usize, &str)], start: usize) -> usize {
    let indent = rows[start].1.len() - rows[start].1.trim_start().len();
    let mut fenced = false;
    let mut comment = false;
    for (index, (_, line)) in rows.iter().enumerate().skip(start + 1) {
        if spec::fence_boundary(line) && !comment {
            fenced = !fenced;
            continue;
        }
        let visible = spec::visible_text_without_comments(line, &mut comment);
        if fenced {
            continue;
        }
        let current_indent = visible.len() - visible.trim_start().len();
        if current_indent <= indent
            && (task_ordering::task_state(visible.trim_start()).is_some()
                || super::markdown::markdown_heading(&visible).is_some())
        {
            return index;
        }
    }
    rows.len()
}

/// Resolves a single declaration or complete heading ancestry. `artifact` is a
/// declared route used only in follow-up references, never a filesystem path.
pub fn select_context(
    markdown: &str,
    selector: &ContextSelector,
    artifact: &str,
    policy: &crate::verified::policy::Policy,
    bounds: &ContextReadBounds,
) -> Result<ContextSelection, String> {
    if markdown.len() > bounds.max_artifact_bytes {
        return Err("context artifact byte bound exceeded".into());
    }
    if !selector_valid(selector, bounds) {
        return Err("invalid or unbounded context selector".into());
    }
    if bounds.max_diagnostics == 0 || bounds.max_references == 0 || bounds.max_selected_bytes == 0 {
        return Err("context policy bounds must be positive".into());
    }
    let rows = lines(markdown);
    let headings =
        spec::visible_heading_inventory(&rows.iter().map(|(_, line)| *line).collect::<Vec<_>>());
    let mut diagnostics = Vec::new();
    let mut fenced = false;
    let mut commented = false;
    for (_, line) in &rows {
        if spec::fence_boundary(line) && !commented {
            fenced = !fenced;
        } else {
            let _ = spec::visible_text_without_comments(line, &mut commented);
        }
    }
    if fenced {
        diagnostics.push(format!("{artifact}: unclosed markdown fence"));
    }
    let mut references = Vec::new();
    let mut match_ranges = Vec::new();
    let mut unsupported = false;
    match selector {
        ContextSelector::Requirement { id } => {
            let source = crate::verified::repo::SpecSource {
                id: artifact.to_owned(),
                path: artifact.to_owned(),
                markdown: markdown.to_owned(),
            };
            diagnostics.extend(
                spec::validate_spec_source_with_policy(&source, &policy.artifact_substance_policy)
                    .1,
            );
            for (definition, first, end) in
                requirement_definitions(&rows, &headings, artifact, &mut diagnostics)
            {
                if &definition == id {
                    match_ranges.push((first, end));
                }
            }
        }
        ContextSelector::Task { id } => {
            let graph = task_ordering::build_task_ordering_graph(markdown, policy);
            unsupported = !graph.active;
            diagnostics.extend(graph.issues.iter().map(|issue| {
                format!(
                    "{}:{}: {}: {}",
                    artifact,
                    issue.line.unwrap_or(0),
                    issue.code,
                    issue.message
                )
            }));
            if graph.issues.iter().any(|issue| {
                issue.task.as_deref() == Some(id)
                    && matches!(
                        issue.code.as_str(),
                        task_ordering::TASK_ORDER_ISSUE_ID_DUPLICATE
                            | task_ordering::TASK_ORDER_ISSUE_ID_MULTIPLE
                    )
            }) {
                match_ranges.push((0, 0));
                match_ranges.push((0, 0));
            } else if let Some(node) = graph.nodes.iter().find(|node| &node.id == id) {
                let index = node.line - 1;
                match_ranges.push((index, task_block_end(&rows, index)));
                for predecessor in &node.predecessors {
                    references.push(ContextReference {
                        kind: "task".into(),
                        id: predecessor.clone(),
                        route: None,
                        required: true,
                    });
                }
                for issue in &graph.issues {
                    if issue.task.as_deref() == Some(id) {
                        if let Some(predecessor) = &issue.predecessor {
                            references.push(ContextReference {
                                kind: "task".into(),
                                id: predecessor.clone(),
                                route: None,
                                required: true,
                            });
                        }
                    }
                }
            }
        }
        ContextSelector::Section { path } => {
            let mut stack: Vec<&str> = Vec::new();
            for heading in &headings {
                if heading.level > stack.len() + 1 {
                    // A skipped heading level has no invented ancestor.
                    stack.clear();
                }
                stack.truncate(heading.level.saturating_sub(1));
                while stack.len() < heading.level.saturating_sub(1) {
                    stack.push("");
                }
                stack.push(&heading.text);
                if stack == path.iter().map(String::as_str).collect::<Vec<_>>() {
                    let end = spec::next_visible_heading_at_or_above(
                        &headings,
                        heading.line_index,
                        heading.level,
                        rows.len(),
                    );
                    match_ranges.push((heading.line_index, end));
                }
            }
            if artifact.ends_with("/spec.md") {
                let source = crate::verified::repo::SpecSource {
                    id: artifact.to_owned(),
                    path: artifact.to_owned(),
                    markdown: markdown.to_owned(),
                };
                diagnostics.extend(
                    spec::validate_spec_source_with_policy(
                        &source,
                        &policy.artifact_substance_policy,
                    )
                    .1,
                );
                let _ = requirement_definitions(&rows, &headings, artifact, &mut diagnostics);
            } else if artifact.ends_with("/tasks.md") {
                let graph = task_ordering::build_task_ordering_graph(markdown, policy);
                diagnostics.extend(graph.issues.iter().map(|issue| {
                    format!(
                        "{}:{}: {}: {}",
                        artifact,
                        issue.line.unwrap_or(0),
                        issue.code,
                        issue.message
                    )
                }));
            }
        }
    }
    diagnostics.sort();
    diagnostics.dedup();
    let status = if unsupported {
        ContextSelectionStatus::Unsupported
    } else {
        match match_ranges.len() {
            0 => ContextSelectionStatus::Missing,
            1 => ContextSelectionStatus::Selected,
            _ => ContextSelectionStatus::Ambiguous,
        }
    };
    let document_valid = diagnostics.is_empty();
    let (selected_span, text) = if status == ContextSelectionStatus::Selected {
        let (first, last) = match_ranges[0];
        let selected_span = span(&rows, first, last, markdown.len());
        if selected_span.end_byte - selected_span.start_byte > bounds.max_selected_bytes {
            return Err(
                "context selected byte bound exceeded; use the whole-artifact route".into(),
            );
        }
        let text = &markdown[selected_span.start_byte..selected_span.end_byte];
        if let ContextSelector::Task { .. } = selector {
            for requirement in visible_requirement_links(text) {
                references.push(ContextReference {
                    kind: "requirement".into(),
                    id: requirement.clone(),
                    route: None,
                    required: true,
                });
            }
        }
        (Some(selected_span), Some(text.to_owned()))
    } else {
        (None, None)
    };
    references.sort_by(|a, b| (&a.kind, &a.id).cmp(&(&b.kind, &b.id)));
    references.dedup();
    let omitted_diagnostics = diagnostics.len().saturating_sub(bounds.max_diagnostics);
    diagnostics.truncate(bounds.max_diagnostics);
    let omitted_references = references.len().saturating_sub(bounds.max_references);
    references.truncate(bounds.max_references);
    Ok(ContextSelection {
        status,
        document_valid,
        span: selected_span,
        text,
        diagnostics,
        omitted_diagnostics,
        diagnostics_route: None,
        references,
        omitted_references,
        references_route: None,
        completeness: if status == ContextSelectionStatus::Selected {
            "complete_selected_block_no_transitive_expansion"
        } else {
            "no_selected_block"
        }
        .into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_and_fenced_requirement_examples_are_not_prerequisites() {
        let links = visible_requirement_links("Work r[demo.real] `r[demo.inline]`\n```\nr[demo.fenced]\n```\n<!-- r[demo.hidden] -->\n");
        assert_eq!(links.into_iter().collect::<Vec<_>>(), vec!["demo.real"]);
    }

    #[test]
    fn task_references_report_omissions_without_expansion() {
        let policy = crate::verified::policy::parse_policy_json(include_str!(
            "../../../../../cairn-policy/generated/cairn-policy.json"
        ))
        .expect("generated policy");
        let bounds = ContextReadBounds {
            max_artifact_bytes: 4096,
            max_selected_bytes: 1024,
            max_selector_bytes: 128,
            max_heading_depth: 6,
            max_diagnostics: 1,
            max_references: 1,
        };
        let source = "# Tasks\n- [ ] [task:base] Base\n- [ ] [task:other] Other\n- [ ] [task:work] [after:base] [after:other] Work r[demo.link]\n";
        let read = select_context(
            source,
            &ContextSelector::Task { id: "work".into() },
            ".cairn/changes/demo/tasks.md",
            &policy,
            &bounds,
        )
        .expect("selection");
        assert_eq!(read.status, ContextSelectionStatus::Selected);
        assert_eq!(read.references.len(), 1);
        assert_eq!(read.omitted_references, 2);
        assert!(read
            .text
            .as_deref()
            .unwrap_or_default()
            .contains("r[demo.link]"));
    }

    #[test]
    fn task_without_declared_identity_is_unsupported_not_synthetic() {
        let policy = crate::verified::policy::parse_policy_json(include_str!(
            "../../../../../cairn-policy/generated/cairn-policy.json"
        ))
        .expect("generated policy");
        let bounds = ContextReadBounds {
            max_artifact_bytes: 4096,
            max_selected_bytes: 1024,
            max_selector_bytes: 128,
            max_heading_depth: 6,
            max_diagnostics: 8,
            max_references: 8,
        };
        let source = "# Tasks\n- [ ] Work without a declared task identity\n";
        let read = select_context(
            source,
            &ContextSelector::Task { id: "work".into() },
            ".cairn/changes/demo/tasks.md",
            &policy,
            &bounds,
        )
        .expect("selection");
        assert_eq!(read.status, ContextSelectionStatus::Unsupported);
        assert!(read.span.is_none());
    }

    #[test]
    fn source_span_preserves_unicode_and_crlf() {
        let policy = crate::verified::policy::parse_policy_json(include_str!(
            "../../../../../cairn-policy/generated/cairn-policy.json"
        ))
        .expect("generated policy");
        let bounds = ContextReadBounds {
            max_artifact_bytes: 4096,
            max_selected_bytes: 1024,
            max_selector_bytes: 128,
            max_heading_depth: 6,
            max_diagnostics: 8,
            max_references: 8,
        };
        let source = "# Design\r\n## Café\r\n### Nested\r\n雪\r\n## Next\r\n";
        let read = select_context(
            source,
            &ContextSelector::Section {
                path: vec!["Design".into(), "Café".into()],
            },
            "design.md",
            &policy,
            &bounds,
        )
        .expect("selection");
        let span = read.span.expect("source span");
        assert_eq!(
            &source[span.start_byte..span.end_byte],
            read.text.as_deref().unwrap_or_default()
        );
        assert_eq!(
            read.text.as_deref(),
            Some("## Café\r\n### Nested\r\n雪\r\n")
        );
    }
}
