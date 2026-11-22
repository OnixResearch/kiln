const REQUIREMENT_HEADING_LEVEL: usize = 3;
const SCENARIO_HEADING_LEVEL: usize = 4;
const COMPAT_ORDINAL_WIDTH: usize = 8;
const REQUIREMENT_HEADING_PREFIX: &str = "Requirement:";
const SCENARIO_HEADING_PREFIX: &str = "Scenario:";
const TASK_TODO_PREFIX: &str = "- [ ]";
const TASK_IN_PROGRESS_PREFIX: &str = "- [~]";
const TASK_DONE_PREFIX_LOWER: &str = "- [x]";
const TASK_DONE_PREFIX_UPPER: &str = "- [X]";
const MARKER_PREFIX: &str = "r[";
const MARKER_SUFFIX: char = ']';
const MARKER_EXTENSION_SEPARATOR: char = '+';

pub(super) fn parse_specs(
    mut sources: Vec<crate::verified::repo::SpecSource>,
) -> (
    Vec<super::types::ParsedSpec>,
    Vec<super::types::LifecycleViewIssue>,
) {
    // r[impl cairn.lifecycle_views.semantic_model.markdown]
    sources.sort_by(|left, right| left.path.cmp(&right.path).then(left.id.cmp(&right.id)));
    let mut parsed = Vec::with_capacity(sources.len());
    let mut issues = Vec::new();
    for source in sources {
        parsed.push(parse_spec(&source, &mut issues));
    }
    (parsed, issues)
}

pub(super) fn parse_tasks(
    change: &str,
    path: &str,
    markdown: &str,
) -> (
    Vec<super::types::ParsedTask>,
    Vec<super::types::LifecycleViewIssue>,
) {
    let mut tasks = Vec::new();
    let mut issues = Vec::new();
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        let Some((state, body)) = task_line(trimmed) else {
            continue;
        };
        let ordinal = tasks.len();
        let id = format!(
            "task:{change}:{ordinal:0width$}",
            width = COMPAT_ORDINAL_WIDTH
        );
        let label = body.trim().to_string();
        if label.is_empty() {
            issues.push(super::types::LifecycleViewIssue {
                code: "lifecycle_view.task.empty_label".to_string(),
                message: "task label must not be empty".to_string(),
                source: path.to_string(),
                target: Some(id.clone()),
            });
        }
        let marker_scan = scan_markers(trimmed);
        issues.extend(marker_issues(path, &marker_scan.malformed));
        tasks.push(super::types::ParsedTask {
            id,
            label,
            state,
            covers: marker_scan.bare,
            path: path.to_string(),
        });
    }
    (tasks, issues)
}

fn parse_spec(
    source: &crate::verified::repo::SpecSource,
    issues: &mut Vec<super::types::LifecycleViewIssue>,
) -> super::types::ParsedSpec {
    let lines = source.markdown.lines().collect::<Vec<_>>();
    let mut requirements = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let Some((level, heading)) = markdown_heading(line) else {
            continue;
        };
        if level != REQUIREMENT_HEADING_LEVEL || !heading.starts_with(REQUIREMENT_HEADING_PREFIX) {
            continue;
        }
        requirements.push(parse_requirement(
            source,
            &lines,
            index,
            heading,
            requirements.len(),
            issues,
        ));
    }
    super::types::ParsedSpec {
        id: format!("spec:{}", source.id),
        label: spec_label(source),
        path: source.path.clone(),
        owner_change: source_owner_change(&source.id),
        requirements,
    }
}

fn parse_requirement(
    source: &crate::verified::repo::SpecSource,
    lines: &[&str],
    heading_index: usize,
    heading: &str,
    ordinal: usize,
    issues: &mut Vec<super::types::LifecycleViewIssue>,
) -> super::types::ParsedRequirement {
    let block_end = next_heading_at_or_above(
        lines,
        heading_index.saturating_add(1),
        REQUIREMENT_HEADING_LEVEL,
    );
    let marker_end = next_heading_at_or_above(
        lines,
        heading_index.saturating_add(1),
        SCENARIO_HEADING_LEVEL,
    )
    .min(block_end);
    let marker_scan = combine_marker_scans(
        scan_markers(heading),
        scan_block_markers(&lines[heading_index.saturating_add(1)..marker_end]),
    );
    issues.extend(marker_issues(&source.path, &marker_scan.malformed));
    let reference = marker_scan.bare.first().cloned();
    let id = reference
        .as_ref()
        .map(|reference| format!("requirement:{reference}"))
        .unwrap_or_else(|| {
            format!(
                "{}:requirement:{ordinal:0width$}",
                source.id,
                width = COMPAT_ORDINAL_WIDTH
            )
        });
    let mut scenarios = Vec::new();
    for index in heading_index.saturating_add(1)..block_end {
        let Some((level, scenario_heading)) = markdown_heading(lines[index]) else {
            continue;
        };
        if level != SCENARIO_HEADING_LEVEL || !scenario_heading.starts_with(SCENARIO_HEADING_PREFIX)
        {
            continue;
        }
        scenarios.push(parse_scenario(
            source,
            lines,
            index,
            scenario_heading,
            &id,
            scenarios.len(),
            issues,
        ));
    }
    super::types::ParsedRequirement {
        id,
        reference,
        label: heading_label(heading, REQUIREMENT_HEADING_PREFIX),
        scenarios,
    }
}

fn parse_scenario(
    source: &crate::verified::repo::SpecSource,
    lines: &[&str],
    heading_index: usize,
    heading: &str,
    requirement_id: &str,
    ordinal: usize,
    issues: &mut Vec<super::types::LifecycleViewIssue>,
) -> super::types::ParsedScenario {
    let block_end = next_heading_at_or_above(
        lines,
        heading_index.saturating_add(1),
        SCENARIO_HEADING_LEVEL,
    );
    let marker_scan = combine_marker_scans(
        scan_markers(heading),
        scan_block_markers(&lines[heading_index.saturating_add(1)..block_end]),
    );
    issues.extend(marker_issues(&source.path, &marker_scan.malformed));
    let reference = marker_scan.bare.first().cloned();
    let id = reference
        .as_ref()
        .map(|reference| format!("scenario:{requirement_id}:{reference}"))
        .unwrap_or_else(|| {
            format!(
                "scenario:{requirement_id}:{ordinal:0width$}",
                width = COMPAT_ORDINAL_WIDTH
            )
        });
    super::types::ParsedScenario {
        id,
        reference,
        label: heading_label(heading, SCENARIO_HEADING_PREFIX),
    }
}

fn task_line(trimmed: &str) -> Option<(super::types::ParsedTaskState, &str)> {
    if let Some(body) = trimmed.strip_prefix(TASK_TODO_PREFIX) {
        return Some((super::types::ParsedTaskState::Todo, body));
    }
    if let Some(body) = trimmed.strip_prefix(TASK_IN_PROGRESS_PREFIX) {
        return Some((super::types::ParsedTaskState::InProgress, body));
    }
    if let Some(body) = trimmed.strip_prefix(TASK_DONE_PREFIX_LOWER) {
        return Some((super::types::ParsedTaskState::Done, body));
    }
    trimmed
        .strip_prefix(TASK_DONE_PREFIX_UPPER)
        .map(|body| (super::types::ParsedTaskState::Done, body))
}

fn spec_label(source: &crate::verified::repo::SpecSource) -> String {
    source
        .markdown
        .lines()
        .find_map(|line| {
            let (level, heading) = markdown_heading(line)?;
            (level == 1).then(|| heading.to_string())
        })
        .unwrap_or_else(|| source.id.clone())
}

fn source_owner_change(source_id: &str) -> Option<String> {
    source_id
        .strip_prefix("change/")
        .and_then(|rest| rest.split_once('/'))
        .map(|(change, _spec)| change.to_string())
}

fn heading_label(heading: &str, prefix: &str) -> String {
    heading
        .strip_prefix(prefix)
        .map(str::trim)
        .unwrap_or(heading)
        .to_string()
}

fn markdown_heading(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let level = trimmed.bytes().take_while(|byte| *byte == b'#').count();
    if level == 0 {
        return None;
    }
    let heading = trimmed.get(level..)?.trim_start();
    (!heading.is_empty()).then_some((level, heading))
}

fn next_heading_at_or_above(lines: &[&str], start: usize, level: usize) -> usize {
    lines
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(index, line)| {
            let (candidate_level, _heading) = markdown_heading(line)?;
            (candidate_level <= level).then_some(index)
        })
        .unwrap_or(lines.len())
}

#[derive(Debug, Default)]
struct MarkerScan {
    bare: Vec<String>,
    malformed: Vec<String>,
}

fn scan_block_markers(lines: &[&str]) -> MarkerScan {
    let mut scan = MarkerScan::default();
    for line in lines {
        let candidate = line
            .trim_start()
            .trim_start_matches(['-', '*'])
            .trim_start();
        if candidate.starts_with(MARKER_PREFIX) {
            scan = combine_marker_scans(scan, scan_leading_marker(candidate));
        }
    }
    scan
}

fn scan_leading_marker(candidate: &str) -> MarkerScan {
    let marker_end = candidate
        .find(MARKER_SUFFIX)
        .map(|index| index.saturating_add(1))
        .unwrap_or(candidate.len());
    scan_markers(&candidate[..marker_end])
}

fn combine_marker_scans(mut left: MarkerScan, right: MarkerScan) -> MarkerScan {
    left.bare.extend(right.bare);
    left.malformed.extend(right.malformed);
    left.bare.sort();
    left.bare.dedup();
    left.malformed.sort();
    left.malformed.dedup();
    left
}

fn scan_markers(line: &str) -> MarkerScan {
    let mut scan = MarkerScan::default();
    let mut remainder = line;
    while let Some(prefix_index) = remainder.find(MARKER_PREFIX) {
        let body_start = prefix_index.saturating_add(MARKER_PREFIX.len());
        let Some(after_prefix) = remainder.get(body_start..) else {
            break;
        };
        let Some(suffix_index) = after_prefix.find(MARKER_SUFFIX) else {
            scan.malformed.push(after_prefix.to_string());
            break;
        };
        let body = &after_prefix[..suffix_index];
        let parts = body.split_whitespace().collect::<Vec<_>>();
        if parts.len() == 1 {
            let reference = parts[0]
                .split(MARKER_EXTENSION_SEPARATOR)
                .next()
                .unwrap_or(parts[0]);
            if stable_reference(reference) {
                scan.bare.push(reference.to_string());
            } else {
                scan.malformed.push(body.to_string());
            }
        }
        let consumed = suffix_index.saturating_add(1);
        remainder = after_prefix.get(consumed..).unwrap_or_default();
    }
    scan.bare.sort();
    scan.bare.dedup();
    scan
}

fn stable_reference(reference: &str) -> bool {
    !reference.is_empty()
        && reference != "..."
        && reference
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
}

fn marker_issues(source: &str, malformed: &[String]) -> Vec<super::types::LifecycleViewIssue> {
    malformed
        .iter()
        .map(|marker| super::types::LifecycleViewIssue {
            code: "lifecycle_view.marker.malformed".to_string(),
            message: format!("malformed lifecycle marker: {marker}"),
            source: source.to_string(),
            target: None,
        })
        .collect()
}
