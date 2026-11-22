// r[impl cairn.semantic_review.metrics_regression_split.focused_helpers]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FixtureOutcome<'a> {
    valid: bool,
    diagnostics: &'a [serde_json::Value],
}

pub(super) fn fixture_result_map<'a>(
    fixture_coverage: &'a serde_json::Value,
    issues: &mut Vec<super::super::SemanticReviewIssue>,
) -> std::collections::BTreeMap<&'a str, FixtureOutcome<'a>> {
    let mut results = std::collections::BTreeMap::new();
    let Some(fixtures) = fixture_coverage
        .get("fixtures")
        .and_then(serde_json::Value::as_array)
    else {
        super::super::common::push_issue(
            issues,
            "missing_fixture_results",
            "fixtures",
            "fixture coverage must declare fixture results",
        );
        return results;
    };
    if fixtures.is_empty() {
        super::super::common::push_issue(
            issues,
            "empty_fixture_results",
            "fixtures",
            "fixture coverage must declare fixture results",
        );
        return results;
    }
    for (index, fixture) in fixtures.iter().enumerate() {
        collect_fixture_result(fixture, index, &mut results, issues);
    }
    results
}

fn collect_fixture_result<'a>(
    fixture: &'a serde_json::Value,
    index: usize,
    results: &mut std::collections::BTreeMap<&'a str, FixtureOutcome<'a>>,
    issues: &mut Vec<super::super::SemanticReviewIssue>,
) {
    let path = match fixture.get("path").and_then(serde_json::Value::as_str) {
        Some(path) if !path.trim().is_empty() => path,
        Some(_) | None => {
            super::super::common::push_issue(
                issues,
                "invalid_fixture_result",
                &format!("fixtures[{index}].path"),
                "fixture result must bind a non-empty path",
            );
            return;
        }
    };
    let Some(valid) = fixture.get("valid").and_then(serde_json::Value::as_bool) else {
        super::super::common::push_issue(
            issues,
            "invalid_fixture_result",
            &format!("fixtures[{index}].valid"),
            "fixture result must bind a boolean valid field",
        );
        return;
    };
    let Some(diagnostics) = fixture
        .get("diagnostics")
        .and_then(serde_json::Value::as_array)
    else {
        super::super::common::push_issue(
            issues,
            "invalid_fixture_result",
            &format!("fixtures[{index}].diagnostics"),
            "fixture result must bind diagnostics as an array",
        );
        return;
    };
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.as_str().is_none())
    {
        super::super::common::push_issue(
            issues,
            "invalid_fixture_result",
            &format!("fixtures[{index}].diagnostics"),
            "fixture diagnostics must be strings",
        );
        return;
    }
    if results
        .insert(path, FixtureOutcome { valid, diagnostics })
        .is_some()
    {
        super::super::common::push_issue(
            issues,
            "duplicate_fixture_result",
            &format!("fixtures[{index}].path"),
            "fixture result path must be unique",
        );
    }
}

pub(super) fn validate_fixture_pairs(
    registry: &serde_json::Value,
    fixture_results: &std::collections::BTreeMap<&str, FixtureOutcome<'_>>,
    issues: &mut Vec<super::super::SemanticReviewIssue>,
) {
    let Some(categories) = registry
        .get("categories")
        .and_then(serde_json::Value::as_array)
    else {
        return;
    };
    for (index, category) in categories.iter().enumerate() {
        let Some(diagnostic_code) = super::common::non_empty_string(category, "diagnostic_code")
        else {
            continue;
        };
        validate_positive_fixture(category, index, fixture_results, issues);
        validate_negative_fixture(category, index, diagnostic_code, fixture_results, issues);
    }
}

fn validate_positive_fixture(
    category: &serde_json::Value,
    index: usize,
    fixture_results: &std::collections::BTreeMap<&str, FixtureOutcome<'_>>,
    issues: &mut Vec<super::super::SemanticReviewIssue>,
) {
    let Some(fixture_ref) = super::common::non_empty_string(category, "positive_fixture") else {
        return;
    };
    match fixture_results.get(fixture_ref) {
        Some(outcome) if outcome.valid => {}
        Some(_) => super::super::common::push_issue(
            issues,
            "positive_fixture_failed",
            &format!("categories[{index}].positive_fixture"),
            "positive fixture must pass",
        ),
        None => super::super::common::push_issue(
            issues,
            "missing_fixture_result",
            &format!("categories[{index}].positive_fixture"),
            "positive fixture result is missing",
        ),
    }
}

fn validate_negative_fixture(
    category: &serde_json::Value,
    index: usize,
    diagnostic_code: &str,
    fixture_results: &std::collections::BTreeMap<&str, FixtureOutcome<'_>>,
    issues: &mut Vec<super::super::SemanticReviewIssue>,
) {
    let Some(fixture_ref) = super::common::non_empty_string(category, "negative_fixture") else {
        return;
    };
    match fixture_results.get(fixture_ref) {
        Some(outcome) if outcome.valid => super::super::common::push_issue(
            issues,
            "negative_fixture_passed",
            &format!("categories[{index}].negative_fixture"),
            "negative fixture must fail",
        ),
        Some(outcome) if has_diagnostic(outcome.diagnostics, diagnostic_code) => {}
        Some(_) => super::super::common::push_issue(
            issues,
            "negative_fixture_missing_diagnostic",
            &format!("categories[{index}].negative_fixture"),
            "negative fixture must emit the category diagnostic",
        ),
        None => super::super::common::push_issue(
            issues,
            "missing_fixture_result",
            &format!("categories[{index}].negative_fixture"),
            "negative fixture result is missing",
        ),
    }
}

fn has_diagnostic(diagnostics: &[serde_json::Value], diagnostic_code: &str) -> bool {
    diagnostics
        .iter()
        .filter_map(serde_json::Value::as_str)
        .any(|diagnostic| diagnostic == diagnostic_code)
}
