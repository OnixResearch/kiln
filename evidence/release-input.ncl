let NonEmpty = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let Identity = {
  path | NonEmpty,
  blake3 | NonEmpty,
} in
{
  schema = "kiln.local-release-evidence.v1",
  source = {
    commit = "36984e367eafd44c22d2b4f4f47121deb51eece3",
    change = "oyzuupzrwuysrvtonpurwqykoozkzrpq",
    archive_blake3 = "ce60d4c323a3966eecd7a93fda32c55c0442c434a2216eaa9fc002fd12ff28f2",
  },
  identities | Array Identity = [
    { path = "Cargo.lock", blake3 = "blake3-yHm9IUm8ELl1wMCqhXLmhctPaHfCSqoKmdWSszTZEHc=" },
    { path = "flake.lock", blake3 = "blake3-BtxUHihCZ+vfPoJ1OoBthWCD0Ri2MIYU2gd0FrdEwIg=" },
    { path = "rust-toolchain.toml", blake3 = "blake3-bfPbiokPYLm7Mr5THHNK0zOnlOij/41aZFQlXnG34Jw=" },
    { path = "config", blake3 = "blake3-DxsGYcNh8v6aK05tUE5JszO3lSBd6s1IiaxpuhSeD+c=" },
    { path = "cairn-policy/generated/cairn-policy.json", blake3 = "blake3-axYEl7hCX4DQTnXtJoTiOTQAZDzBeiA2trsrmjHcPak=" },
  ],
  checks = {
    workspace_tests = { verdict = "PASS", tests = 78 },
    strict_clippy = "PASS",
    no_std_wasm = "PASS",
    nickel_positive_and_negative = "PASS",
    dependency_boundaries = "PASS",
    runtime_independence = "PASS",
    nix_flake_check_x86_64_linux = "PASS",
    cairn_validation = "PASS",
    cairn_task_gates = "PASS",
    traceability = "PASS",
  },
  profiles = {
    provider_contract = "kiln-provider-v1",
    radicle_broker = "radicle-ci-broker-v1",
    remote_execution = "build.bazel.remote.execution.v2",
    cobs = import "../config/cob-profiles.ncl",
    stack = import "../config/stack-profiles.ncl",
    hosting_candidate = import "publication-candidate.ncl",
    hosting_publication = import "publication-receipt.ncl",
  },
  references = {
    cairn_cob = "837d730799faffe6a088cfd6dc49e08c7e0f1822",
    pueue = "b3fe223ef0a3c51d95d5c6128700b79442c54af7",
    details = "docs/reference-pins.md",
  },
  authority = {
    rid_assigned = true,
    radicle_publication_authorized = true,
    remote_publication_performed = true,
  },
  non_claims = [
    "local tests do not prove external provider correctness",
    "interchange projections do not grant canonical state authority",
    "publication does not prove indefinite availability or seed trust",
    "Cairn gates prove lifecycle structure and do not prove whole-system correctness",
  ],
}
