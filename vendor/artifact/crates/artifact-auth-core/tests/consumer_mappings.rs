//! Positive and negative checks for typed consumer mapping projections.

#![allow(clippy::expect_used)]

use std::collections::BTreeSet;

use serde::Deserialize;

const MAPPING_SCHEMA_V1: &str = "artifact-auth.consumer-mapping.v1";
const EXPECTED_CONSUMER_COUNT: usize = 4;

#[derive(Debug, Clone, Deserialize)]
struct ConsumerMapping {
    schema: String,
    consumer: String,
    source_revision: String,
    shared: Vec<String>,
    extensions: Vec<String>,
    retained_authority: Vec<String>,
}

struct ExpectedMapping<'a> {
    consumer: &'a str,
    source_revision: &'a str,
    retained_authority: &'a [&'a str],
}

fn mapping_issues(mapping: &ConsumerMapping, expected: &ExpectedMapping<'_>) -> Vec<String> {
    let mut issues = Vec::new();
    if mapping.schema != MAPPING_SCHEMA_V1 {
        issues.push("mapping.schema".to_string());
    }
    if mapping.consumer != expected.consumer {
        issues.push("mapping.consumer".to_string());
    }
    if mapping.source_revision != expected.source_revision {
        issues.push("mapping.source_revision".to_string());
    }
    if mapping.shared.is_empty() {
        issues.push("mapping.shared".to_string());
    }
    if mapping.retained_authority
        != expected
            .retained_authority
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>()
    {
        issues.push("mapping.retained_authority".to_string());
    }
    for (field, values) in [
        ("mapping.shared_duplicate", &mapping.shared),
        ("mapping.extensions_duplicate", &mapping.extensions),
        (
            "mapping.retained_authority_duplicate",
            &mapping.retained_authority,
        ),
    ] {
        let unique = values.iter().collect::<BTreeSet<_>>();
        if unique.len() != values.len() {
            issues.push(field.to_string());
        }
    }
    issues
}

fn fixtures() -> Vec<(ConsumerMapping, ExpectedMapping<'static>)> {
    vec![
        (
            serde_json::from_slice(include_bytes!("../../../fixtures/consumers/cairn.json"))
                .expect("typed Cairn mapping"),
            ExpectedMapping {
                consumer: "cairn",
                source_revision: "cc4639be24475d80844b091cdec605eb2fef9ff0",
                retained_authority: &[
                    "filesystem-measurement",
                    "receipt-canonicalization",
                    "lifecycle-policy",
                    "readiness",
                    "json",
                    "process-exit",
                ],
            },
        ),
        (
            serde_json::from_slice(include_bytes!("../../../fixtures/consumers/mantle.json"))
                .expect("typed Mantle mapping"),
            ExpectedMapping {
                consumer: "mantle",
                source_revision: "26c6880c08cd730c9e9420f92ea91a87212e492d",
                retained_authority: &[
                    "repository-authorization",
                    "registry-routing",
                    "credentials",
                    "signing",
                    "oci",
                    "build",
                    "release",
                ],
            },
        ),
        (
            serde_json::from_slice(include_bytes!("../../../fixtures/consumers/molten.json"))
                .expect("typed Molten mapping"),
            ExpectedMapping {
                consumer: "molten",
                source_revision: "70590459a218dc8e66948ab6f305a7c54142b710",
                retained_authority: &[
                    "key-generation",
                    "signing",
                    "key-storage",
                    "capability-authority",
                    "federation-membership",
                    "preserves",
                    "iroh",
                ],
            },
        ),
        (
            serde_json::from_slice(include_bytes!("../../../fixtures/consumers/valence.json"))
                .expect("typed Valence mapping"),
            ExpectedMapping {
                consumer: "valence",
                source_revision: "8eccac35fcd8aae8ef64b3d1021e7f1a53656180",
                retained_authority: &[
                    "radicle-verification",
                    "actor-authorization",
                    "evidence-ir",
                    "cob-reduction",
                    "git-storage",
                    "ssh-agent",
                    "lifecycle-handoff",
                ],
            },
        ),
    ]
}

// r[verify artifact_auth.conformance.consumers]
#[test]
fn typed_consumer_mappings_preserve_required_authority() {
    let fixtures = fixtures();
    assert_eq!(fixtures.len(), EXPECTED_CONSUMER_COUNT);
    for (mapping, expected) in fixtures {
        assert!(
            mapping_issues(&mapping, &expected).is_empty(),
            "{} mapping lost information: {:?}",
            expected.consumer,
            mapping_issues(&mapping, &expected)
        );
    }
}

#[test]
fn dropping_one_required_authority_is_rejected() {
    let (mut mapping, expected) = fixtures().remove(0);
    mapping.retained_authority.pop();
    assert_eq!(
        mapping_issues(&mapping, &expected),
        vec!["mapping.retained_authority".to_string()]
    );
}
