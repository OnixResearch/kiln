{
  schema_version = 2,
  id = "security-audit",
  display_name = "Security Audit",
  description = "Run Cairn, cargo-audit, and Octet checks and retain a Provenance summary receipt.",
  category = "audit",
  # Lattice classification.rs classifies every Shell node as Mutating; Kiln's RunbookRegistry classifies security-audit as read_only with write_scopes = [].
  mode = "mutating",
  params = [
    {
      name = "crate_root",
      type = { kind = "relative_path", max_bytes = 128 },
      required = false,
      default = ".",
      bindings = [{ node = "audit", shell_env = "LATTICE_PARAM_CRATE_ROOT" }],
    },
    {
      name = "audit_scope",
      type = { kind = "enum", values = ["all", "cairn", "cargo-audit", "octet"] },
      required = false,
      default = "all",
      bindings = [{ node = "audit", shell_env = "LATTICE_PARAM_AUDIT_SCOPE" }],
    },
    {
      name = "strict",
      type = { kind = "bool" },
      required = false,
      default = true,
      bindings = [{ node = "audit", shell_env = "LATTICE_PARAM_STRICT" }],
    },
  ],
  workflow = {
    path = "workflow.ncl",
    blake3 = "c8c3213107b3ea1593563e4a77906e4206b48d0744c64133eac93108527e9640",
  },
  orchestration = {
    path = "orchestration.ncl",
    blake3 = "58799aa96c8695899e58001fc82f3b25a6348826f7723f73de289e2f27eae6c9",
  },
  assets = [
    {
      path = "assets/run-security-audit.py",
      media_kind = "text/x-python",
      blake3 = "0c7a51d71e36c886a3afa6d01877536646af25051d7ba2d4dcc2a8c1e8df040a",
      required = true,
    },
  ],
  compatibility = {
    workflow_schema = 1,
    orchestration_schema = 1,
    node_kinds = ["shell"],
    runtime_kinds = ["shell"],
    feature_ids = ["lattice.workflow-template-bundle.v1"],
  },
  operator_notes = [
    "Lattice classifies Shell nodes conservatively as mutating (classification.rs); Kiln admits security-audit as read_only with empty write_scopes.",
    "Unavailable tools or offline advisory databases are reported explicitly as unavailable in the summary receipt.",
  ],
  non_claims = [
    "Bundle identity proves exact reviewed bytes only and grants no authority.",
    "Security audit summary receipts (grants_authority = false) do not prove absence of vulnerabilities or grant merge/release authority.",
  ],
}
