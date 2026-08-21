# r[impl kiln.cairn_gate.profile] r[impl kiln.cairn_gate.nonclaims]
let Positive = std.contract.from_predicate (fun value =>
  std.is_number value && value > 0
) in
let AbsolutePath = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^/" value
) in
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let Profile = {
  source | Exact "ssh://git@github.com/OnixResearch/cairn.git",
  revision | Exact "15f00875562025e7ea7e0d1f4af24d1a2e2ac06f",
  contract_license | Exact "MIT OR Apache-2.0",
  cli_license | Exact "AGPL-3.0-or-later",
  bounded_exec_source | Exact "https://git.onix.computer/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git",
  bounded_exec_revision | Exact "29dac88ecded94457572db3fdfaaaab95fa91525",
  bounded_exec_license | Exact "AGPL-3.0-or-later",
  bounded_exec_reuse_basis | Exact "OnixResearch owner-authorized source reuse",
  request_schema | Exact "cairn.gate-request.v1",
  result_schema | Exact "cairn.gate-result.v1",
  machine_schema | Exact "cairn.cli-machine.v1",
  diagnostic_schema | Exact "cairn.cli-machine-diagnostic.v1",
  policy_schema | Exact "cairn.policy-schema.v1",
  gate_receipt_schema | Exact "cairn.gate-receipt.v1",
  command | Exact "gate",
  policy_name | Exact "kiln-default",
  platform | Exact "x86_64-linux",
  supported_stages | Exact ["proposal", "design", "tasks"],
  rerun_policy | Exact "never",
  provider_profile_id | Exact "cairn-gate-v1",
  executable_path | Exact "/nix/store/bvz6kl88qkfi15n8ml1xr7p22syykggd-cairn-0.1.0/bin/cairn",
  executable_identity | Exact "9ade7248f9f48a0a83fc21365cc45550b09a048621ac925292ed974dbc4951d0",
  repository_root | AbsolutePath,
  policy_path | AbsolutePath,
  policy_identity | Exact "be930a118f405d176ea56de5c72ba05b8061ab3a4f996113cde505710cce5c74",
  exit_statuses | {
    success | Exact 0,
    invalid_arguments | Exact 2,
    not_found | Exact 3,
    policy_error | Exact 4,
    blocked | Exact 5,
    internal_error | Exact 70,
  },
  limits | {
    max_changes | Positive,
    max_edges | Positive,
    max_evidence_identities | Positive,
    max_findings | Positive,
    max_request_bytes | Positive,
    max_result_bytes | Positive,
    max_stderr_bytes | Positive,
    max_diagnostic_bytes | Positive,
    max_executable_bytes | Positive,
    max_in_flight | Positive,
    stdin_max_bytes | Positive,
    timeout_milliseconds | Positive,
    poll_interval_milliseconds | Positive,
    teardown_timeout_milliseconds | Positive,
  },
  machine_non_claims | Array String,
  native_gate_non_claims | Array String,
  result_non_claims | Array String,
} in
let MaxChanges = 64 in
let MaxEdges = 256 in
let MaxEvidenceIdentities = 256 in
let MaxFindings = 256 in
let MaxRequestBytes = 1048576 in
let MaxResultBytes = 16777216 in
let MaxStderrBytes = 1048576 in
let MaxDiagnosticBytes = 4096 in
let MaxExecutableBytes = 268435456 in
let MaxInFlight = 64 in
let StdinMaxBytes = 1 in
let TimeoutMilliseconds = 30000 in
let PollIntervalMilliseconds = 10 in
let TeardownTimeoutMilliseconds = 1000 in
{
  profile | Profile = {
    source = "ssh://git@github.com/OnixResearch/cairn.git",
    revision = "15f00875562025e7ea7e0d1f4af24d1a2e2ac06f",
    contract_license = "MIT OR Apache-2.0",
    cli_license = "AGPL-3.0-or-later",
    bounded_exec_source = "https://git.onix.computer/z2CpqLFpdP36fZXYUK5ZNWxMibpCo.git",
    bounded_exec_revision = "29dac88ecded94457572db3fdfaaaab95fa91525",
    bounded_exec_license = "AGPL-3.0-or-later",
    bounded_exec_reuse_basis = "OnixResearch owner-authorized source reuse",
    request_schema = "cairn.gate-request.v1",
    result_schema = "cairn.gate-result.v1",
    machine_schema = "cairn.cli-machine.v1",
    diagnostic_schema = "cairn.cli-machine-diagnostic.v1",
    policy_schema = "cairn.policy-schema.v1",
    gate_receipt_schema = "cairn.gate-receipt.v1",
    command = "gate",
    policy_name = "kiln-default",
    platform = "x86_64-linux",
    supported_stages = ["proposal", "design", "tasks"],
    rerun_policy = "never",
    provider_profile_id = "cairn-gate-v1",
    executable_path = "/nix/store/bvz6kl88qkfi15n8ml1xr7p22syykggd-cairn-0.1.0/bin/cairn",
    executable_identity = "9ade7248f9f48a0a83fc21365cc45550b09a048621ac925292ed974dbc4951d0",
    repository_root = "/var/lib/kiln/repository",
    policy_path = "/etc/kiln/cairn-policy.json",
    policy_identity = "be930a118f405d176ea56de5c72ba05b8061ab3a4f996113cde505710cce5c74",
    exit_statuses = {
      success = 0,
      invalid_arguments = 2,
      not_found = 3,
      policy_error = 4,
      blocked = 5,
      internal_error = 70,
    },
    limits = {
      max_changes = MaxChanges,
      max_edges = MaxEdges,
      max_evidence_identities = MaxEvidenceIdentities,
      max_findings = MaxFindings,
      max_request_bytes = MaxRequestBytes,
      max_result_bytes = MaxResultBytes,
      max_stderr_bytes = MaxStderrBytes,
      max_diagnostic_bytes = MaxDiagnosticBytes,
      max_executable_bytes = MaxExecutableBytes,
      max_in_flight = MaxInFlight,
      stdin_max_bytes = StdinMaxBytes,
      timeout_milliseconds = TimeoutMilliseconds,
      poll_interval_milliseconds = PollIntervalMilliseconds,
      teardown_timeout_milliseconds = TeardownTimeoutMilliseconds,
    },
    machine_non_claims = [
      "a valid machine envelope proves only that Cairn reached the controlled output boundary and serialized one typed outcome",
      "a machine envelope does not prove nested evidence truth or command correctness",
      "a machine envelope does not prove mutation safety, process reliability, or host integrity",
      "a machine envelope does not prove lifecycle readiness or release eligibility",
      "no envelope is guaranteed after host termination, process kill, memory exhaustion, pre-boundary panic, broken stdout, or failure before Cairn starts",
    ],
    native_gate_non_claims = [
      "workflow state proves only the selected policy graph and observed structural artifact facts",
      "workflow instructions do not prove artifact truth, review quality, or implementation correctness",
      "workflow state does not satisfy proposal, design, tasks, sync, archive, evidence, or release gates",
      "workflow commands are read-only and do not create, edit, delete, or approve lifecycle artifacts",
    ],
    result_non_claims = [
      "gate binding does not prove candidate facts describe the evaluated filesystem",
      "gate binding does not prove source correctness",
      "gate binding does not prove test completeness",
      "gate binding does not prove process reliability",
      "gate binding does not authorize merge or publication",
      "gate binding does not execute a release",
      "gate binding does not prove deployment success",
      "gate binding does not establish future applicability",
    ],
  },
}
