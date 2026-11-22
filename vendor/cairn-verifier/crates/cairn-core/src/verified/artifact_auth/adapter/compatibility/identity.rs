pub(super) fn classify_preimages(
    legacy: &str,
    standalone: Option<&str>,
    output: &mut Vec<super::CairnArtifactAuthCompatibilityClassification>,
) {
    let Some(standalone) = standalone else {
        super::push_classification(
            output,
            super::CairnArtifactAuthCompatibilityDimension::Preimage,
            "artifact_auth.compatibility.preimage.missing",
            Some(legacy),
            None,
            super::CairnArtifactAuthCompatibilityDisposition::Blocking,
        );
        return;
    };
    let is_pair_valid = super::valid_blake3(legacy) && super::valid_blake3(standalone);
    let (code, disposition) = if !is_pair_valid {
        (
            "artifact_auth.compatibility.preimage.malformed",
            super::CairnArtifactAuthCompatibilityDisposition::Blocking,
        )
    } else if legacy == standalone {
        (
            "artifact_auth.compatibility.preimage.domain_not_distinct",
            super::CairnArtifactAuthCompatibilityDisposition::Blocking,
        )
    } else {
        (
            "artifact_auth.compatibility.preimage.schema_mapped",
            super::CairnArtifactAuthCompatibilityDisposition::IntentionallyMapped,
        )
    };
    super::push_classification(
        output,
        super::CairnArtifactAuthCompatibilityDimension::Preimage,
        code,
        Some(legacy),
        Some(standalone),
        disposition,
    );
}

pub(super) fn classify_identities(
    standalone_preimage: Option<&str>,
    legacy: &crate::AuthenticatedEvidenceReport,
    standalone: &super::CairnArtifactAuthReport,
    output: &mut Vec<super::CairnArtifactAuthCompatibilityClassification>,
) {
    classify_equal_identity(
        "artifact_auth.compatibility.identity.subject",
        &legacy.measured_subject_hash,
        &standalone.measured_subject_hash,
        output,
    );
    classify_equal_identity(
        "artifact_auth.compatibility.identity.producer",
        &legacy.producer,
        &standalone.producer,
        output,
    );
    classify_equal_identity(
        "artifact_auth.compatibility.identity.key_label",
        &legacy.key_id,
        &standalone.key_id,
        output,
    );
    classify_key_encoding(standalone, output);
    classify_authentication_identity(legacy, standalone, output);
    classify_standalone_identity_set(standalone_preimage, standalone, output);
}

pub(super) fn classify_decisions(
    legacy: &crate::AuthenticatedEvidenceReport,
    standalone: &super::CairnArtifactAuthReport,
    output: &mut Vec<super::CairnArtifactAuthCompatibilityClassification>,
) {
    let is_report_pair_well_formed = legacy.passed == legacy.issues.is_empty()
        && standalone.decision_passed == standalone.issues.is_empty();
    let is_decision_equal = legacy.passed == standalone.decision_passed;
    let is_rejection_context_explained = legacy.passed
        || legacy
            .issues
            .iter()
            .any(|issue| shared_rejection_context(&issue.code));
    super::push_classification(
        output,
        super::CairnArtifactAuthCompatibilityDimension::Decision,
        "artifact_auth.compatibility.decision.outcome",
        Some(if legacy.passed { "passed" } else { "rejected" }),
        Some(if standalone.decision_passed {
            "passed"
        } else {
            "rejected"
        }),
        if is_report_pair_well_formed && is_decision_equal && is_rejection_context_explained {
            super::CairnArtifactAuthCompatibilityDisposition::Equal
        } else {
            super::CairnArtifactAuthCompatibilityDisposition::Blocking
        },
    );
}

fn shared_rejection_context(code: &str) -> bool {
    matches!(
        code,
        "authenticated_inputs.measured_digest_mismatch"
            | "authenticated_inputs.signature_parent_mismatch"
            | "authenticated_inputs.signature_parent_unlinked"
            | "authenticated_inputs.signer_revoked"
            | "authenticated_inputs.signature_purpose_unauthorized"
            | "authenticated_inputs.signature_purpose_mismatch"
    )
}

fn classify_equal_identity(
    code: &str,
    legacy: &str,
    standalone: &str,
    output: &mut Vec<super::CairnArtifactAuthCompatibilityClassification>,
) {
    let disposition = if legacy == standalone {
        super::CairnArtifactAuthCompatibilityDisposition::Equal
    } else {
        super::CairnArtifactAuthCompatibilityDisposition::Blocking
    };
    super::push_classification(
        output,
        super::CairnArtifactAuthCompatibilityDimension::Identity,
        code,
        Some(legacy),
        Some(standalone),
        disposition,
    );
}

fn classify_key_encoding(
    standalone: &super::CairnArtifactAuthReport,
    output: &mut Vec<super::CairnArtifactAuthCompatibilityClassification>,
) {
    let legacy = &standalone.legacy_public_key_blake3;
    let raw = &standalone.standalone_public_key_blake3;
    let is_valid_and_distinct =
        super::valid_blake3(legacy) && super::valid_blake3(raw) && legacy != raw;
    super::push_classification(
        output,
        super::CairnArtifactAuthCompatibilityDimension::Identity,
        "artifact_auth.compatibility.identity.public_key_encoding_mapped",
        Some(legacy),
        Some(raw),
        if is_valid_and_distinct {
            super::CairnArtifactAuthCompatibilityDisposition::IntentionallyMapped
        } else {
            super::CairnArtifactAuthCompatibilityDisposition::Blocking
        },
    );
}

fn classify_authentication_identity(
    legacy: &crate::AuthenticatedEvidenceReport,
    standalone: &super::CairnArtifactAuthReport,
    output: &mut Vec<super::CairnArtifactAuthCompatibilityClassification>,
) {
    let is_authentication_identity_valid =
        match (&legacy.authentication_hash, &standalone.authentication_hash) {
            (Some(left), Some(right)) => {
                super::valid_blake3(left) && super::valid_blake3(right) && left != right
            }
            (None, None) => !legacy.passed && !standalone.decision_passed,
            _ => false,
        };
    super::push_classification(
        output,
        super::CairnArtifactAuthCompatibilityDimension::Identity,
        "artifact_auth.compatibility.identity.decision_schema_mapped",
        legacy.authentication_hash.as_deref(),
        standalone.authentication_hash.as_deref(),
        if is_authentication_identity_valid {
            super::CairnArtifactAuthCompatibilityDisposition::IntentionallyMapped
        } else {
            super::CairnArtifactAuthCompatibilityDisposition::Blocking
        },
    );
}

fn classify_standalone_identity_set(
    standalone_preimage: Option<&str>,
    standalone: &super::CairnArtifactAuthReport,
    output: &mut Vec<super::CairnArtifactAuthCompatibilityClassification>,
) {
    let is_statement_match = standalone_preimage
        .zip(standalone.standalone_statement_blake3.as_deref())
        .is_some_and(|(expected, actual)| expected == actual && super::valid_blake3(actual));
    let is_decision_identity_set_valid = super::valid_blake3(&standalone.standalone_policy_blake3)
        && super::valid_blake3(&standalone.standalone_scope_blake3)
        && super::valid_blake3(&standalone.standalone_decision_blake3);
    let is_verified_key_set_valid = if standalone.decision_passed {
        standalone.standalone_verified_key_blake3
            == vec![standalone.standalone_public_key_blake3.clone()]
    } else {
        standalone
            .standalone_verified_key_blake3
            .iter()
            .all(|identity| super::valid_blake3(identity))
    };
    super::push_classification(
        output,
        super::CairnArtifactAuthCompatibilityDimension::Identity,
        "artifact_auth.compatibility.identity.standalone_set",
        None,
        standalone.standalone_statement_blake3.as_deref(),
        if is_statement_match && is_decision_identity_set_valid && is_verified_key_set_valid {
            super::CairnArtifactAuthCompatibilityDisposition::IntentionallyMapped
        } else {
            super::CairnArtifactAuthCompatibilityDisposition::Blocking
        },
    );
}
