pub(super) fn classify_issues(
    legacy: &crate::AuthenticatedEvidenceReport,
    standalone: &super::CairnArtifactAuthReport,
    output: &mut Vec<super::CairnArtifactAuthCompatibilityClassification>,
) {
    if legacy.issues.is_empty() && standalone.issues.is_empty() {
        super::push_classification(
            output,
            super::CairnArtifactAuthCompatibilityDimension::Issue,
            "artifact_auth.compatibility.issue.none",
            None,
            None,
            super::CairnArtifactAuthCompatibilityDisposition::Equal,
        );
        return;
    }
    let standalone_codes = standalone
        .issues
        .iter()
        .map(|issue| issue.code.as_str())
        .collect::<Vec<_>>();
    let mut matched_standalone = Vec::new();
    classify_legacy_issues(legacy, &standalone_codes, &mut matched_standalone, output);
    classify_standalone_issues(standalone, &standalone_codes, &matched_standalone, output);
}

fn classify_legacy_issues<'a>(
    legacy: &'a crate::AuthenticatedEvidenceReport,
    standalone_codes: &[&'a str],
    matched_standalone: &mut Vec<&'a str>,
    output: &mut Vec<super::CairnArtifactAuthCompatibilityClassification>,
) {
    for issue in &legacy.issues {
        if let Some(mapped) = mapped_standalone_issue(&issue.code) {
            let is_matched = standalone_codes.contains(&mapped);
            if is_matched {
                matched_standalone.push(mapped);
            }
            push_issue_classification(
                output,
                &issue.code,
                is_matched.then_some(mapped),
                if is_matched {
                    super::CairnArtifactAuthCompatibilityDisposition::IntentionallyMapped
                } else {
                    super::CairnArtifactAuthCompatibilityDisposition::Blocking
                },
            );
        } else {
            let is_retained = consumer_retained_issue(&issue.code);
            push_issue_classification(
                output,
                &issue.code,
                None,
                if is_retained {
                    super::CairnArtifactAuthCompatibilityDisposition::IntentionallyMapped
                } else {
                    super::CairnArtifactAuthCompatibilityDisposition::Blocking
                },
            );
        }
    }
}

fn classify_standalone_issues(
    standalone: &super::CairnArtifactAuthReport,
    standalone_codes: &[&str],
    matched_standalone: &[&str],
    output: &mut Vec<super::CairnArtifactAuthCompatibilityClassification>,
) {
    for issue in &standalone.issues {
        if matched_standalone.contains(&issue.code.as_str()) {
            continue;
        }
        let is_enriched = standalone_enriched_issue(&issue.code, standalone_codes);
        push_issue_classification(
            output,
            "artifact_auth.compatibility.issue.no_legacy_equivalent",
            Some(&issue.code),
            if is_enriched {
                super::CairnArtifactAuthCompatibilityDisposition::IntentionallyMapped
            } else {
                super::CairnArtifactAuthCompatibilityDisposition::Blocking
            },
        );
    }
}

fn mapped_standalone_issue(legacy: &str) -> Option<&'static str> {
    match legacy {
        "authenticated_inputs.signature_unverified" => Some("auth.signature_unverified"),
        "authenticated_inputs.signer_revoked" => Some("auth.key_not_current"),
        "authenticated_inputs.signature_purpose_unauthorized"
        | "authenticated_inputs.signature_purpose_mismatch" => Some("auth.purpose_unauthorized"),
        _ => None,
    }
}

fn consumer_retained_issue(code: &str) -> bool {
    matches!(
        code,
        "authenticated_inputs.measured_digest_mismatch"
            | "authenticated_inputs.signature_parent_mismatch"
            | "authenticated_inputs.signature_parent_unlinked"
    )
}

fn standalone_enriched_issue(code: &str, all_codes: &[&str]) -> bool {
    match code {
        "auth.cryptographic_failure_present" => all_codes.contains(&"auth.signature_unverified"),
        "auth.distinct_key_threshold_unsatisfied" => {
            all_codes.contains(&"auth.signature_unverified")
                || all_codes.contains(&"auth.key_not_current")
                || all_codes.contains(&"auth.purpose_unauthorized")
        }
        _ => false,
    }
}

fn push_issue_classification(
    output: &mut Vec<super::CairnArtifactAuthCompatibilityClassification>,
    legacy: &str,
    standalone: Option<&str>,
    disposition: super::CairnArtifactAuthCompatibilityDisposition,
) {
    super::push_classification(
        output,
        super::CairnArtifactAuthCompatibilityDimension::Issue,
        "artifact_auth.compatibility.issue.mapping",
        Some(legacy),
        standalone,
        disposition,
    );
}
