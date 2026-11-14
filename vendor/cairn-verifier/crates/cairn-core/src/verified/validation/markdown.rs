pub(super) fn markdown_heading(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let marker_len = trimmed.chars().take_while(|value| *value == '#').count();
    if marker_len == 0 || !trimmed[marker_len..].starts_with(' ') {
        return None;
    }
    Some((marker_len, trimmed[marker_len..].trim()))
}

pub(super) fn next_heading_at_or_above(lines: &[&str], start: usize, max_level: usize) -> usize {
    lines[start..]
        .iter()
        .position(|line| markdown_heading(line).is_some_and(|(level, _heading)| level <= max_level))
        .map_or(lines.len(), |offset| start + offset)
}

pub(super) fn has_requirement_strength(lines: &[&str]) -> bool {
    lines.iter().any(|line| {
        let upper = line.to_ascii_uppercase();
        contains_word(&upper, "MUST")
            || contains_word(&upper, "SHALL")
            || contains_word(&upper, "SHOULD")
            || contains_word(&upper, "MAY")
    })
}

pub(super) fn has_scenario_clause(lines: &[&str], clause: &str) -> bool {
    lines.iter().any(|line| {
        let normalized = normalize_scenario_line(line);
        normalized.starts_with(clause)
    })
}

fn normalize_scenario_line(line: &str) -> String {
    line.trim_start()
        .trim_start_matches('-')
        .trim_start_matches('*')
        .trim_start()
        .trim_start_matches("**")
        .to_ascii_uppercase()
}

fn contains_word(text: &str, needle: &str) -> bool {
    text.split(|value: char| !value.is_ascii_alphanumeric())
        .any(|word| word == needle)
}
