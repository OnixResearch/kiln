//! Recorded-cohort conformance for the repository-owned Basalt import.
//!
//! Kiln imports Basalt over its single vendored UCAN snapshot through
//! `scripts/import-pinned-source.sh`. The records and the rewritten manifest are
//! compiled into this test, so a drift between the recorded revision, archive
//! identity and literal manifest rewrites and the vendored bytes fails here
//! instead of surfacing as an unresolved path at build time.

use serde_json::Value;

const BASALT_SOURCE: &str = include_str!("../../../vendor/basalt.source.json");
const UCAN_SOURCE: &str = include_str!("../../../vendor/ucan.source.json");
const BASALT_MANIFEST: &str = include_str!("../../../vendor/basalt/Cargo.toml");

/// The Basalt revision Kiln imports; Lattice vendors the same revision.
const BASALT_COHORT_REVISION: &str = "fac0d7f1fd7ac936ca4d3f251f3287666e93d4bb";
/// The one UCAN revision Basalt vendors and Kiln serves.
const UCAN_COHORT_REVISION: &str = "fca62a6ed6895689ff18112b497bc7a986516c20";
const IMPORTER: &str = "scripts/import-pinned-source.sh";
const BLAKE3_HEX_BYTES: usize = 32;

fn parse(source: &str) -> Value {
    serde_json::from_str(source).unwrap_or_else(|error| unreachable!("{error}"))
}

fn text<'a>(record: &'a Value, field: &str) -> &'a str {
    record
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_else(|| unreachable!("record field {field}"))
}

#[test]
// r[verify kiln.implementation.authority.cohort]
// r[verify kiln.implementation.authority.cohort.scenario.import]
fn recorded_import_resolves_basalt_to_the_single_vendored_ucan_cohort() {
    let basalt = parse(BASALT_SOURCE);
    let ucan = parse(UCAN_SOURCE);

    assert_eq!(text(&basalt, "source_commit"), BASALT_COHORT_REVISION);
    assert_eq!(text(&ucan, "source_commit"), UCAN_COHORT_REVISION);
    // Basalt compiles in the UCAN revision its own snapshot names, so the
    // single Kiln UCAN snapshot must be that revision.
    assert_eq!(
        basalt::nostr_identity::UCAN_SOURCE_REVISION,
        UCAN_COHORT_REVISION
    );
    assert_eq!(text(&basalt, "importer"), IMPORTER);
    assert_eq!(text(&ucan, "importer"), IMPORTER);
    for record in [&basalt, &ucan] {
        assert_eq!(
            text(record, "archive_blake3").len(),
            BLAKE3_HEX_BYTES * 2,
            "{} archive identity",
            text(record, "source_repository")
        );
    }

    let rewrites = basalt
        .get("manifest_rewrites")
        .and_then(Value::as_array)
        .unwrap_or_else(|| unreachable!("basalt manifest rewrites"));
    assert_eq!(rewrites.len(), 3);
    for rewrite in rewrites {
        let path = text(rewrite, "path");
        let from = text(rewrite, "from");
        let to = text(rewrite, "to");
        let occurrences = rewrite
            .get("occurrences")
            .and_then(Value::as_u64)
            .unwrap_or_else(|| unreachable!("rewrite occurrences"));
        assert_eq!(path, "Cargo.toml");
        assert!(
            from.contains("vendor/ucan") && to.contains("../ucan"),
            "{from} -> {to}"
        );
        assert!(
            !BASALT_MANIFEST.contains(from),
            "vendored basalt manifest still carries the omitted nested snapshot: {from}"
        );
        assert_eq!(
            u64::try_from(BASALT_MANIFEST.matches(to).count())
                .unwrap_or_else(|error| unreachable!("{error}")),
            occurrences,
            "rewrite {from} -> {to}"
        );
    }
}
