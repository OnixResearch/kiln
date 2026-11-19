const ADMITTED_ADAPTER: &str = "https://seed.radicle.garden/z3kGDT73ja8rewvGLwJFLF5MTx1ih.git";
const REVIEWED_OBJECT: &str = "a0de79303f3f45c18a34fff535b690a85cb3cf59";
const EXPECTED_CORE_PACKAGE_COUNT: usize = 2;
const CARGO_MANIFEST: &str = include_str!("../Cargo.toml");
const CARGO_LOCK: &str = include_str!("../Cargo.lock");

#[test]
fn both_cores_use_one_exact_radicle_object() {
    for package in ["replay-ledger-core", "revocation-view-core"] {
        let expected =
            format!("{package} = {{ git = \"{ADMITTED_ADAPTER}\", rev = \"{REVIEWED_OBJECT}\", optional = true }}");
        assert!(CARGO_MANIFEST.contains(&expected), "missing exact source for {package}");
    }

    let locked_source = format!("git+{ADMITTED_ADAPTER}?rev={REVIEWED_OBJECT}#{REVIEWED_OBJECT}");
    assert_eq!(
        CARGO_LOCK.matches(&locked_source).count(),
        EXPECTED_CORE_PACKAGE_COUNT,
        "both lock entries must resolve the reviewed object"
    );
}

#[test]
fn fallback_sources_are_absent() {
    for prohibited in [
        "github.com/OnixResearch/durable-authority-state",
        "path = \"../durable-authority-state\"",
        "rev = \"main\"",
        "branch =",
        "tag =",
    ] {
        assert!(!CARGO_MANIFEST.contains(prohibited), "prohibited authority-state fallback is present: {prohibited}");
    }
}
