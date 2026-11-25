{
  schema_version = 2,
  id = "flake-update",
  display_name = "Flake Update",
  description = "Update flake inputs, run verification checks, and pause at an approval gate before Kiln publication.",
  category = "maintenance",
  mode = "mutating",
  params = [
    {
      name = "flake_dir",
      type = { kind = "relative_path", max_bytes = 128 },
      required = false,
      default = ".",
      bindings = [{ node = "update-flake", shell_env = "LATTICE_PARAM_FLAKE_DIR" }],
    },
    {
      name = "update_scope",
      type = { kind = "enum", values = ["all", "nixpkgs", "cairn", "toolchain"] },
      required = false,
      default = "all",
      bindings = [{ node = "update-flake", shell_env = "LATTICE_PARAM_UPDATE_SCOPE" }],
    },
    {
      name = "run_check",
      type = { kind = "bool" },
      required = false,
      default = true,
      bindings = [{ node = "update-flake", shell_env = "LATTICE_PARAM_RUN_CHECK" }],
    },
  ],
  workflow = {
    path = "workflow.ncl",
    blake3 = "c3fc0eaea96fd679f457c6c89950b0d9c24441b43213be75d4da9ed8357735ce",
  },
  orchestration = {
    path = "orchestration.ncl",
    blake3 = "2ec3071b77cedbbd66f9f5cdce316db8a69de501b6e187f178c80d0ecc200ea3",
  },
  assets = [
    {
      path = "assets/run-flake-update.py",
      media_kind = "text/x-python",
      blake3 = "c282c8d89783c97744bb2ef24a163a6b19aca015042a23b33be83d2e1b2e73f8",
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
    "Bundle identity proves exact reviewed bytes only and grants no execution or merge authority.",
    "Worker execution holds no GitHub push or PR credentials.",
  ],
}
