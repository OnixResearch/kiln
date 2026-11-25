{
  schema_version = 2,
  id = "license-ci-scaffold",
  display_name = "License & CI Scaffold",
  description = "Scaffold LICENSE and GitHub CI workflow files from pinned templates and pause at an approval gate before PR publication.",
  category = "scaffolding",
  mode = "mutating",
  params = [
    {
      name = "license_spdx",
      type = { kind = "enum", values = ["apache-2.0", "mit", "mit-or-apache-2.0"] },
      required = false,
      default = "mit-or-apache-2.0",
      bindings = [{ node = "scaffold", shell_env = "LATTICE_PARAM_LICENSE_SPDX" }],
    },
    {
      name = "ci_template",
      type = { kind = "enum", values = ["rust-cargo", "nix-flake", "rust-and-nix"] },
      required = false,
      default = "rust-and-nix",
      bindings = [{ node = "scaffold", shell_env = "LATTICE_PARAM_CI_TEMPLATE" }],
    },
    {
      name = "overwrite_existing",
      type = { kind = "bool" },
      required = false,
      default = false,
      bindings = [{ node = "scaffold", shell_env = "LATTICE_PARAM_OVERWRITE_EXISTING" }],
    },
  ],
  workflow = {
    path = "workflow.ncl",
    blake3 = "1ff2372e88ad772f890b6e9f583bc5d052e9e76da2f3b95236aec0a8ec262554",
  },
  orchestration = {
    path = "orchestration.ncl",
    blake3 = "1c986fb9128933c3d02bb07e62d51d02d52fae7a1001568ee61961daa34db0f6",
  },
  assets = [
    {
      path = "assets/LICENSE-APACHE-2.0.txt",
      media_kind = "text/plain",
      blake3 = "b063ecf42b9058f89cac7e6d9c6002d80f8f3d3fc121824cb8d19f5531fae897",
      required = true,
    },
    {
      path = "assets/LICENSE-MIT.txt",
      media_kind = "text/plain",
      blake3 = "597e8b2d9f87c66f56ad445b0ce74e829d4381021449441f50d26a7749006468",
      required = true,
    },
    {
      path = "assets/ci-nix-flake.yml",
      media_kind = "text/yaml",
      blake3 = "e5669073403cb3e7becfcfff8ffa952be033d5a0dec7270876fd860b80dbffc3",
      required = true,
    },
    {
      path = "assets/ci-rust-and-nix.yml",
      media_kind = "text/yaml",
      blake3 = "bb94d090927b488eae697ccb382e31cf5749ea1961fa868e114a131e619666fe",
      required = true,
    },
    {
      path = "assets/ci-rust-cargo.yml",
      media_kind = "text/yaml",
      blake3 = "53d9cafea0ad1a67cb84f4b881e760d04e0c9a660eca57f93ce49a94eb30c19e",
      required = true,
    },
    {
      path = "assets/run-license-ci-scaffold.py",
      media_kind = "text/x-python",
      blake3 = "7d3a118489ad63d1ab82b10e472994e05994f44da8a603cb6657d7823c133604",
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
    "Mutating bundle: requires an explicit Kiln RunbookGrant and approval before Kiln's publisher opens a PR.",
    "Lattice parameterized approval workflows must be bound via workflow_template::bind and save_draft rather than lattice run --input.",
  ],
  non_claims = [
    "Bundle identity proves exact reviewed bytes only and grants no execution or publication authority.",
    "Scaffolding LICENSE/CI files does not replace legal review or grant merge eligibility.",
  ],
}
