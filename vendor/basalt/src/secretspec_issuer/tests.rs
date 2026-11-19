// Issuer-key resolution fixture and rotation verification:
// r[verify ucan_token_flow.oidc.secretspec_fixtures]
use super::{
    IssuerKeyDeclaration, IssuerResolutionReason, RotationWindow, build_resolve_request,
    classify_resolution, validate_declaration, validate_rotation_window,
};
use crate::secretspec_issuer::{
    DEFAULT_MAX_RESOLVED_VALUE_BYTES, IssuerResolutionFailure, REQUIRED_ISSUER_NON_CLAIMS,
    ResolutionBoundaries, resolve_issuer_key,
};
use crate::secretspec_issuer_identity::{OIDC_EXCHANGE_SCOPE, SECRETSPEC_VERSION};

fn declaration() -> IssuerKeyDeclaration {
    IssuerKeyDeclaration::new("production", "oidc-signing-key", "env://")
}

fn ok_envelope() -> String {
    serde_json::json!({
        "ok": true,
        "response": {
            "schema_version": 2,
            "provider": "env://",
            "profile": "production",
            "scope": OIDC_EXCHANGE_SCOPE,
            "secrets": { "oidc-signing-key": { "value": "k", "as_path": false, "source": "provider" } },
            "missing_required": [],
            "missing_optional": []
        }
    })
    .to_string()
}

#[test]
fn declaration_is_valid_under_reviewed_scope() {
    let mut declaration = declaration();
    declaration.scope = OIDC_EXCHANGE_SCOPE.to_owned();

    assert!(validate_declaration(&declaration).is_empty());
}

#[test]
fn rejects_declaration_outside_reviewed_scope() {
    let mut declaration = declaration();
    declaration.scope = "other-scope".to_owned();

    let issues = validate_declaration(&declaration);
    assert!(!issues.is_empty());
    assert!(issues.iter().any(|issue| issue.code == "issuer.scope"));
}

#[test]
fn rejects_weakened_dependency_identity() {
    let mut declaration = declaration();
    declaration.secretspec_version = "0.99.0".to_owned();

    let issues = validate_declaration(&declaration);
    assert!(issues.iter().any(|issue| issue.code == "issuer.dependency"));
}

#[test]
fn build_resolve_request_is_deterministic() {
    let declaration = declaration();
    let first = build_resolve_request(&declaration, "/config/secretspec.toml");
    let second = build_resolve_request(&declaration, "/config/secretspec.toml");

    assert_eq!(first, second);
    assert!(first.contains("\"scope\":\"basalt-oidc-exchange\""));
}

#[test]
fn classifies_matching_key_as_loaded() {
    let result = classify_resolution(&declaration(), &ok_envelope(), 256);

    assert!(result.is_allowed());
    assert_eq!(result.reason, IssuerResolutionReason::Loaded);
}

#[test]
fn fails_closed_on_missing_required_key() {
    let mut envelope: serde_json::Value = serde_json::from_str(&ok_envelope()).unwrap();
    envelope["response"]["missing_required"] = serde_json::json!(["oidc-signing-key"]);
    envelope["response"]["secrets"] = serde_json::json!({});

    let result = classify_resolution(&declaration(), &envelope.to_string(), 256);

    assert!(!result.is_allowed());
    assert_eq!(result.reason, IssuerResolutionReason::Missing);
}

#[test]
fn fails_closed_on_missing_declared_key() {
    let mut envelope: serde_json::Value = serde_json::from_str(&ok_envelope()).unwrap();
    envelope["response"]["secrets"] = serde_json::json!({
        "other-key": { "value": "k", "as_path": false, "source": "provider" }
    });

    let result = classify_resolution(&declaration(), &envelope.to_string(), 256);

    assert!(!result.is_allowed());
    assert_eq!(result.reason, IssuerResolutionReason::Missing);
}

#[test]
fn fails_closed_on_wrong_scope() {
    let mut envelope: serde_json::Value = serde_json::from_str(&ok_envelope()).unwrap();
    envelope["response"]["scope"] = serde_json::json!("other-scope");

    let result = classify_resolution(&declaration(), &envelope.to_string(), 256);

    assert!(!result.is_allowed());
    assert_eq!(result.reason, IssuerResolutionReason::WrongScope);
}

#[test]
fn fails_closed_on_oversized_output() {
    let oversized = ok_envelope();
    let output_bytes = DEFAULT_MAX_RESOLVED_VALUE_BYTES + 1;

    let result = classify_resolution(&declaration(), &oversized, output_bytes);

    assert!(!result.is_allowed());
    assert_eq!(result.reason, IssuerResolutionReason::OversizedOutput);
}

#[test]
fn fails_closed_on_provider_failure_envelope() {
    let provider_failure = r#"{"ok":false,"error":{"kind":"provider","message":"unavailable"}}"#;
    let result = classify_resolution(&declaration(), provider_failure, 256);

    assert!(!result.is_allowed());
    assert_eq!(result.reason, IssuerResolutionReason::ProviderFailure);
}

#[test]
fn preserves_non_claims_on_every_result() {
    let loaded = classify_resolution(&declaration(), &ok_envelope(), 256);
    let rejected = classify_resolution(
        &declaration(),
        r#"{"ok":false,"error":{"kind":"provider","message":"unavailable"}}"#,
        256,
    );

    let expected = REQUIRED_ISSUER_NON_CLAIMS.map(str::to_owned).to_vec();
    assert_eq!(loaded.non_claims, expected);
    assert_eq!(rejected.non_claims, expected);
}

#[test]
fn declaration_reject_reports_deterministic_issues() {
    let mut declaration = declaration();
    declaration.scope = "other-scope".to_owned();

    let result = classify_resolution(&declaration, &ok_envelope(), 256);

    assert!(!result.is_allowed());
    assert_eq!(result.reason, IssuerResolutionReason::DeclarationRejected);
    assert!(
        result
            .issues
            .iter()
            .any(|issue| issue.code == "issuer.scope")
    );
}

#[test]
fn pinned_version_identity_is_stable() {
    assert_eq!(SECRETSPEC_VERSION, "0.17.0");
    assert!(!REQUIRED_ISSUER_NON_CLAIMS.is_empty());
}

#[test]
fn rotation_window_admits_public_verification_material() {
    let window = RotationWindow::new(
        "blake3:active",
        vec!["blake3:previous".to_owned()],
        "oidc-signing-key-2",
    );

    assert!(validate_rotation_window(&window).is_empty());
    assert!(window.admits("oidc-signing-key-2"));
    assert!(!window.admits("oidc-signing-key-1"));
}

#[test]
fn rotation_window_rejects_duplicate_active_digest() {
    let window = RotationWindow::new(
        "blake3:active",
        vec!["blake3:active".to_owned()],
        "oidc-signing-key-2",
    );

    let issues = validate_rotation_window(&window);
    assert!(
        issues
            .iter()
            .any(|issue| issue.code == "issuer.rotation.active-duplicate")
    );
}

#[test]
fn rotation_window_rejects_oversized_overlap() {
    let overlap = (0..=super::MAX_ROTATION_OVERLAP_KEYS)
        .map(|i| format!("digest-{i}"))
        .collect();
    let window = RotationWindow::new("blake3:active", overlap, "oidc-signing-key-2");

    let issues = validate_rotation_window(&window);
    assert!(
        issues
            .iter()
            .any(|issue| issue.code == "issuer.rotation.overlap-limit")
    );
}

#[test]
fn default_boundaries_keep_writes_and_caches_disabled() {
    let boundaries = ResolutionBoundaries::default();
    assert!(boundaries.is_safely_disabled());
}

#[test]
fn resolve_refuses_enabled_writes_or_cache() {
    let declaration = declaration();
    let boundaries = ResolutionBoundaries {
        disable_writes: false,
        ..ResolutionBoundaries::default()
    };

    let result = resolve_issuer_key(&declaration, "/no/config.toml", boundaries);

    assert_eq!(result, Err(IssuerResolutionFailure::WriteOrCacheEnabled));
}
