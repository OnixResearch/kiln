# Reviewed local CI entries for the operator-owned OnixResearch registry.
# Caller supplies explicit repository paths, source identities, installed tools,
# credentials and resource bounds through web-service.ncl/web-worker.ncl.
# These are committed-ref checks, not working-copy or full release gates.
let Project = {
  id | String,
  label | String,
  reference | String,
  nix_args | Array String,
}
in
let rust = fun project_id project_label flags =>
  {
    id = project_id,
    label = project_label,
    reference = "refs/heads/main",
    nix_args =
      [
        "develop",
        "path:.",
        "--no-write-lock-file",
        "--command",
        "cargo",
        "test",
        "--locked",
      ]
      @ flags,
  }
in
[
  rust "cap-root" "Cap Root - workspace tests" ["--workspace"],
  rust "bounded-exec" "Bounded Exec - workspace tests" ["--workspace"],
  rust "durable-file-publication" "Durable File Publication - all-target tests" ["--all-targets"],
  rust "content-identity-core" "Content Identity Core - all-target tests" ["--all-targets"],
  rust "schema-identity-core" "Schema Identity Core - conformance and all features" ["--workspace", "--all-targets", "--all-features"],
  rust "schema-migration-core" "Schema Migration Core - conformance and all features" ["--workspace", "--all-targets", "--all-features"],
  {
    id = "quantity-contracts",
    label = "Quantity Contracts - positive and negative matrix",
    reference = "refs/heads/main",
    nix_args = ["flake", "check", "path:.", "--no-write-lock-file", "-L"],
  },
  rust "valence" "Valence - workspace tests" ["--workspace"],
  rust "artifact" "Artifact - workspace tests" ["--workspace"],
  rust "bounded-http" "Bounded HTTP - all-feature conformance" ["--workspace", "--all-targets", "--all-features"],
  rust "durable-authority-state" "Durable Authority State - all-feature conformance" ["--workspace", "--all-targets", "--all-features"],
  rust "nickel-export" "Nickel Export - workspace tests" ["--workspace"],
  rust "nickel-eval" "Nickel Eval - all-target tests" ["--all-targets"],
  rust "transactional-reconciliation-core" "Transactional Reconciliation Core - all-feature conformance" ["--workspace", "--all-targets", "--all-features"],
] | Array Project
