#![allow(
    unknown_lints,
    clippy::expect_used,
    assertion_density,
    non_trait_imports,
    no_unwrap,
    unbounded_collection_growth,
    allocation_in_loop,
    unbounded_loop,
    reason = "the source guard walks a checked-in tree under an explicit entry bound and fails when repository files disappear"
)]

use std::path::Path;

const MAXIMUM_SOURCE_ENTRIES: usize = 96;
const FORBIDDEN_SOURCE_MARKERS: &[&str] = &["std::process", "std::env", "lattice::", "Command::new"];
const FORBIDDEN_MANIFEST_MARKERS: &[&str] = &["path = \"../..\"", "name = \"lattice\""];

// r[verify lattice.workflow_interchange.transport_handler]
#[test]
fn transport_source_does_not_import_the_lattice_application_or_process_shell() {
    let manifest_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = std::fs::read_to_string(manifest_root.join("Cargo.toml")).expect("transport manifest is readable");
    assert!(violations(&manifest, FORBIDDEN_MANIFEST_MARKERS).is_empty());

    let mut pending = vec![manifest_root.join("src")];
    let mut observed_entries = 0_usize;
    while let Some(path) = pending.pop() {
        observed_entries = observed_entries.saturating_add(1);
        assert!(observed_entries <= MAXIMUM_SOURCE_ENTRIES);
        if path.is_dir() {
            for entry in std::fs::read_dir(path).expect("transport source directory is readable") {
                assert!(pending.len() < MAXIMUM_SOURCE_ENTRIES);
                pending.push(entry.expect("transport source entry is readable").path());
            }
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            let source = std::fs::read_to_string(&path).expect("transport source is readable");
            assert!(
                violations(&source, FORBIDDEN_SOURCE_MARKERS).is_empty(),
                "{} contains a forbidden application or process marker",
                path.display()
            );
        }
    }
}

#[test]
fn source_guard_rejects_application_and_process_markers() {
    let invalid = "use lattice::engine; std::process::Command::new(\"hidden\");";
    let found = violations(invalid, FORBIDDEN_SOURCE_MARKERS);
    assert!(found.contains(&"lattice::"));
    assert!(found.contains(&"std::process"));
    assert!(found.contains(&"Command::new"));
}

fn violations<'a>(source: &str, markers: &'a [&'a str]) -> Vec<&'a str> {
    markers.iter().copied().filter(|marker| source.contains(marker)).collect()
}
