# r[impl kiln.nix_eval_jobs_onix.profile]
# r[impl kiln.nix_eval_jobs_onix.cohort]
# Human-authored selection of the closed, separately provisioned Onix cohort.
let Stock = (import "nix-eval-jobs-profile.ncl").profile in
let Fixed = {
  provider_profile_id = "onix-nix-eval-v1",
  provider_role = "discovery",
  upstream_source = "https://github.com/NixOS/nix-eval-jobs",
  upstream_revision = "9f1553afabd95f9f59951f5c946e86005af7196a",
  upstream_license = "GPL-3.0",
  adapter_version = "2.36.4",
  source_version = "0.1.6",
  nix_source = "https://github.com/onixcomputer/nix",
  nix_revision = "f93f28ad9dce007baae13aa461c16abf656d1171",
  nix_version = "2.36.0",
  nixpkgs_revision = "34ca302a9572963c02e385c056be37c85ff51b77",
  platform = "x86_64-linux",
  cohort_manifest_schema = Stock.cohort_manifest_schema,
  bounded_exec_source = Stock.bounded_exec_source,
  bounded_exec_revision = Stock.bounded_exec_revision,
  bounded_exec_license = Stock.bounded_exec_license,
  capabilities = Stock.capabilities,
  outcomes = Stock.outcomes,
  executable_path = "/nix/store/69k3iijnj5s8417anq7ncgc9f006gc1n-nix-eval-jobs-2.36.4/bin/nix-eval-jobs",
  executable_identity = "4b4e221bfe3645da5f9d89198f211e07f63e4ea5f4fbd6064d6311bfa5090361",
  cohort_manifest_path = "/nix/store/bnigspvq8r3pxwy2cvkzaa6cgarbh1mi-kiln-nix-eval-jobs-onix-cohort.json",
  cohort_manifest_identity = "1e7df147057fde6b439541427a51c2d4ba70f6a42ee15d30021d120e9dda8263",
  evaluator_kind = "onix-nix",
  host_abi = "nix_wasm_init_v1",
  features = ["flakes", "nix-command", "wasm-builtin"],
  plugin_path = "/nix/store/di3jrnpmrc4lff4581vxmprmcsgswd0r-nickel-plugin-0.1.0/nickel_plugin.wasm",
  plugin_identity = "6bbf0180617b4d87746416eb8d58d09dea1076312935038b4d92706e5c7626f6",
  plugin_source = "https://github.com/onixcomputer/onix-wasm",
  plugin_revision = "54bf09574c42a21f9f38dc7e01bc44440e6fbc07",
  fixture_upstream = "https://github.com/onixcomputer/onix-modules",
  fixture_revision = "69a131d71a509f9c57646b71113f75f39ab637b1",
  source_path = "/nix/store/yxjjwvja5c8qfz10b6qh14bjd2bwrpww-kiln-onix-discovery-jobset-v1",
  source_identity = "dbb21080c65c18ce318cf95106f8b112edb1c29219700f846d0721c77d5abbfb",
  root_selector = "jobs.x86_64-linux",
  policy = Stock.policy,
  limits = Stock.limits,
  required_non_claims = [
    "discovery observation does not prove Nix or Nickel evaluation correctness",
    "source identity does not prove source trust",
    "task mapping does not prove complete dependency semantics",
    "cache advisory does not prove cache truth or output presence",
    "discovery observation does not prove build execution or output correctness",
    "exact Onix fixture compatibility does not prove general stock Nix compatibility",
    "discovery observation does not prove reproducibility",
    "discovery observation does not establish release readiness",
    "local process execution does not prove hostile-code confinement",
    "source-defined Wasm handshake does not prove a versioned public host ABI",
    "owner-approved package review source does not establish a production release",
  ],
} in
let Absolute = fun value =>
  std.is_string value && std.string.is_match "^/" value
in
let ProfileContract = std.contract.from_predicate (fun value =>
  std.is_record value
  && std.record.has_field "gc_root_base" value
  && std.record.has_field "home_base" value
  && Absolute value.gc_root_base
  && Absolute value.home_base
  && (std.record.remove "gc_root_base" (std.record.remove "home_base" value)) == Fixed
) in
{
  Profile = ProfileContract,
  profile | ProfileContract = Fixed & {
    gc_root_base = "/var/lib/kiln/onix-nix-eval-roots",
    home_base = "/var/lib/kiln/onix-nix-eval-home",
  },
}
