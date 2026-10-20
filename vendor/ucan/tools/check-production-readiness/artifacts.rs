use std::fs;
use std::path::Path;

use ucan::integration_profile::GENERATED_DOC_END;
use ucan::integration_profile::GENERATED_DOC_START;
use ucan::integration_profile::SupportCatalog;
use ucan::integration_profile::render_operator_support_section;
use ucan::integration_profile::render_support_catalog_json;
use ucan::integration_profile::render_support_vocabulary;

use crate::error::ShellError;

const REPORT_PATH: &str = "docs/production-readiness.json";
const VOCABULARY_PATH: &str = "contracts/integration-profiles/generated/support-vocabulary.ncl";
const OPERATOR_DOC_PATH: &str = "docs/production-readiness.md";
const BOUNDED_DOC_PATHS: &[&str] = &["README.md", "docs/public-api.md", "docs/verified-boundaries.md"];
const REQUIRED_READINESS_REFERENCE: &str = "production-readiness";
const FORBIDDEN_CLAIMS: &[&str] = &[
    "UCAN provides built-in remote DID resolution",
    "UCAN provides built-in proof transport",
    "UCAN provides durable replay persistence",
    "UCAN is production-supported",
];

pub fn generate(root: &Path, catalog: &SupportCatalog) -> Result<(), ShellError> {
    let report = map_render(render_support_catalog_json(catalog))?;
    let vocabulary = map_render(render_support_vocabulary(catalog))?;
    let generated_section = map_render(render_operator_support_section(catalog))?;
    fs::write(root.join(REPORT_PATH), report)?;
    fs::write(root.join(VOCABULARY_PATH), vocabulary)?;
    let operator_path = root.join(OPERATOR_DOC_PATH);
    let operator_doc = fs::read_to_string(&operator_path)?;
    fs::write(operator_path, replace_generated_section(&operator_doc, &generated_section)?)?;
    Ok(())
}

pub fn check(root: &Path, catalog: &SupportCatalog) -> Result<(), ShellError> {
    check_exact(&root.join(REPORT_PATH), &map_render(render_support_catalog_json(catalog))?)?;
    check_exact(&root.join(VOCABULARY_PATH), &map_render(render_support_vocabulary(catalog))?)?;
    let expected_section = map_render(render_operator_support_section(catalog))?;
    let operator_doc = fs::read_to_string(root.join(OPERATOR_DOC_PATH))?;
    let actual_section = extract_generated_section(&operator_doc)?;
    if actual_section != expected_section {
        return Err(ShellError::new(
            "operator-doc-drift",
            "generated production-readiness section differs from the Rust catalog",
        ));
    }
    check_bounded_docs(root)
}

fn map_render<T>(result: Result<T, ucan::integration_profile::ReadinessIssues>) -> Result<T, ShellError> {
    result.map_err(|error| {
        let messages = error
            .issues
            .iter()
            .map(|entry| format!("{}:{}:{}", entry.code, entry.path, entry.message))
            .collect::<Vec<_>>()
            .join("; ");
        ShellError::new("catalog", messages)
    })
}

fn check_exact(path: &Path, expected: &str) -> Result<(), ShellError> {
    let actual = fs::read_to_string(path)?;
    if actual == expected {
        return Ok(());
    }
    Err(ShellError::new(
        "generated-artifact-drift",
        format!("{} is stale; run the readiness generator", path.display()),
    ))
}

fn replace_generated_section(document: &str, generated: &str) -> Result<String, ShellError> {
    match marker_range(document) {
        Ok(Some((start, end))) => {
            let mut rendered = String::with_capacity(document.len() + generated.len());
            rendered.push_str(&document[..start]);
            rendered.push_str(generated);
            rendered.push_str(&document[end..]);
            Ok(rendered)
        }
        Ok(None) => Ok(format!("{}\n\n{generated}\n", document.trim_end())),
        Err(error) => Err(error),
    }
}

fn extract_generated_section(document: &str) -> Result<&str, ShellError> {
    let Some((start, end)) = marker_range(document)? else {
        return Err(ShellError::new(
            "operator-doc-markers",
            "production-readiness document has no generated catalog section",
        ));
    };
    Ok(&document[start..end])
}

fn marker_range(document: &str) -> Result<Option<(usize, usize)>, ShellError> {
    let start = document.find(GENERATED_DOC_START);
    let end = document.find(GENERATED_DOC_END);
    match (start, end) {
        (None, None) => Ok(None),
        (Some(start_index), Some(end_index)) if start_index <= end_index => {
            Ok(Some((start_index, end_index + GENERATED_DOC_END.len())))
        }
        _ => Err(ShellError::new(
            "operator-doc-markers",
            "generated catalog markers are missing, duplicated, or reversed",
        )),
    }
}

fn check_bounded_docs(root: &Path) -> Result<(), ShellError> {
    for relative in BOUNDED_DOC_PATHS {
        let path = root.join(relative);
        let document = fs::read_to_string(&path)?;
        if !document.contains(REQUIRED_READINESS_REFERENCE) {
            return Err(ShellError::new(
                "readiness-reference",
                format!("{} does not link the readiness boundary", path.display()),
            ));
        }
        if let Some(claim) = FORBIDDEN_CLAIMS.iter().find(|claim| document.contains(**claim)) {
            return Err(ShellError::new(
                "documentation-overclaim",
                format!("{} contains forbidden claim {claim:?}", path.display()),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_section_replacement_is_exact_and_idempotent() {
        let original = "# Readiness\n";
        let generated =
            "<!-- BEGIN GENERATED UCAN SUPPORT CATALOG -->\nvalue\n<!-- END GENERATED UCAN SUPPORT CATALOG -->";
        let first = replace_generated_section(original, generated).unwrap_or_default();
        let second = replace_generated_section(&first, generated).unwrap_or_default();
        assert_eq!(first, second);
        assert!(matches!(extract_generated_section(&first), Ok(value) if value == generated));
    }

    #[test]
    fn malformed_markers_fail_closed() {
        let malformed = format!("{GENERATED_DOC_END}\n{GENERATED_DOC_START}");
        assert!(extract_generated_section(&malformed).is_err());
    }

    #[test]
    fn paths_are_repository_relative() {
        for path in BOUNDED_DOC_PATHS.iter().chain([REPORT_PATH, VOCABULARY_PATH, OPERATOR_DOC_PATH].iter()) {
            assert!(!std::path::PathBuf::from(path).is_absolute());
        }
    }
}
