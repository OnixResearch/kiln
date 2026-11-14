pub const EXPORT_BUNDLE_SCHEMA_V1: &str = "octet.export-bundle.v1";

pub(super) const EXPORT_BUNDLE_REQUIRED_FIELDS: &[&str] = &[
    "schema",
    "bundle_hash",
    "objects",
    "name_pointers",
    "replay",
    "compiler_ir",
    "mutation_manifest",
    "caveats",
];
pub(super) const EXPORT_BUNDLE_REPLAY_REQUIRED_FIELDS: &[&str] = &["command", "args", "toolchain"];
pub(super) const EXPORT_BUNDLE_ISSUE_CAPACITY: usize = 16;
