{
  schema_version = 2,
  id = "release-cut",
  display_name = "Release Cut",
  description = "Prepare a verified release candidate and require an approval gate before tag creation or release publication.",
  category = "release",
  mode = "mutating",
  params = [
    {
      name = "crate_manifest",
      type = { kind = "relative_path", max_bytes = 128 },
      required = false,
      default = "Cargo.toml",
      bindings = [{ node = "prepare-release", shell_env = "LATTICE_PARAM_CRATE_MANIFEST" }],
    },
    {
      name = "release_channel",
      type = { kind = "enum", values = ["alpha", "beta", "rc", "stable"] },
      required = false,
      default = "rc",
      bindings = [{ node = "prepare-release", shell_env = "LATTICE_PARAM_RELEASE_CHANNEL" }],
    },
    {
      name = "bump_level",
      type = { kind = "enum", values = ["patch", "minor", "major"] },
      required = false,
      default = "patch",
      bindings = [{ node = "prepare-release", shell_env = "LATTICE_PARAM_BUMP_LEVEL" }],
    },
    {
      name = "allow_dirty",
      type = { kind = "bool" },
      required = false,
      default = false,
      bindings = [{ node = "prepare-release", shell_env = "LATTICE_PARAM_ALLOW_DIRTY" }],
    },
  ],
  workflow = {
    path = "workflow.ncl",
    blake3 = "ca024fde4b9ca7bde6ded1079b30d10ab15d48cf740a6debb19c9bdf3f3b5ef4",
  },
  orchestration = {
    path = "orchestration.ncl",
    blake3 = "93a870be1e21b7b1e3cc132ba72dfd8bab6c8b9c77a9794bd44490bd5b4bc0bc",
  },
  assets = [
    {
      path = "assets/run-release-cut.py",
      media_kind = "text/x-python",
      blake3 = "6e48823d8aa6dcaff583655da87f6c91d758f891279065665c1ed9dd588be9fa",
      required = true,
    },
  ],
  compatibility = {
    workflow_schema = 1,
    orchestration_schema = 1,
    node_kinds = ["shell", "approval"],
    runtime_kinds = ["shell", "approval"],
    feature_ids = ["lattice.workflow-template-bundle.v1"],
  },
  operator_notes = [
    "Mutating bundle: requires an explicit Kiln RunbookGrant and approval before Kiln's publisher tags or pushes.",
    "Lattice parameterized approval workflows must be bound via workflow_template::bind and save_draft rather than lattice run --input.",
  ],
  non_claims = [
    "Bundle identity proves exact reviewed bytes only and grants no execution or release authority.",
    "Release candidate receipt grants no tag, push, or publication authority.",
  ],
}
