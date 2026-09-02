// Source guard: missing repository files are immediate harness failures and concrete path ownership
// stays visible in the test.
#![allow(
    unknown_lints,
    clippy::expect_used,
    non_trait_imports,
    no_unwrap,
    assertion_density,
    unbounded_collection_growth,
    allocation_in_loop,
    unbounded_loop,
    reason = "the test has no safe result when checked-in files are unavailable; an explicit entry limit bounds its source walk"
)]

use std::path::Path;

const MAXIMUM_SOURCE_ENTRIES: usize = 64;
const FORBIDDEN_SOURCE_MARKERS: &[&str] = &[
    "std::",
    "tokio::",
    "reqwest::",
    "redb::",
    "cap_std::",
    "std::process",
    "std::fs",
    "std::net",
    "std::env",
    "chrono::",
    "lattice::",
];
const FORBIDDEN_MANIFEST_MARKERS: &[&str] = &[
    "path = \"../..\"",
    "name = \"lattice\"",
    "tokio",
    "reqwest",
    "redb",
    "chrono",
    "cap-std",
];

// r[verify lattice.workflow_interchange.contract]
// r[verify lattice.workflow_interchange.contract.scenario.independent]
// r[verify lattice.workflow_interchange.contract.scenario.runtime]
#[test]
fn contract_source_contains_no_runtime_or_shell_dependency() {
    let manifest_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_root = manifest_root.join("src");
    assert!(manifest_root.join("LICENSE").is_file());
    let manifest = std::fs::read_to_string(manifest_root.join("Cargo.toml")).expect("contract manifest is readable");
    assert!(violations(&manifest, FORBIDDEN_MANIFEST_MARKERS).is_empty());

    let mut pending = vec![source_root];
    let mut observed_entries = 0_usize;
    while let Some(path) = pending.pop() {
        observed_entries = observed_entries.saturating_add(1);
        assert!(observed_entries <= MAXIMUM_SOURCE_ENTRIES);
        if path.is_dir() {
            for entry in std::fs::read_dir(path).expect("contract source directory is readable") {
                assert!(pending.len() < MAXIMUM_SOURCE_ENTRIES);
                pending.push(entry.expect("contract source entry is readable").path());
            }
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            let source = std::fs::read_to_string(&path).expect("contract source is readable");
            assert!(
                violations(&source, FORBIDDEN_SOURCE_MARKERS).is_empty(),
                "{} contains a forbidden shell or runtime marker",
                path.display()
            );
        }
    }
}

#[test]
fn source_guard_rejects_shell_and_runtime_markers() {
    let invalid = "use std::fs; use lattice::engine;";
    let found = violations(invalid, FORBIDDEN_SOURCE_MARKERS);

    assert!(found.contains(&"std::"));
    assert!(found.contains(&"std::fs"));
    assert!(found.contains(&"lattice::"));
}

fn violations<'a>(source: &str, markers: &'a [&'a str]) -> Vec<&'a str> {
    markers.iter().copied().filter(|marker| source.contains(marker)).collect()
}
