let NonEmptyString = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let HexDigest = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
) in
let Revision = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^[0-9a-f]{40}$" value
) in
let ProviderIdentity = {
  schema_version | std.contract.Equal "ucan-aeneas-provider-identity/v1",
  octet | {
    revision | Revision,
    package | NonEmptyString,
    closure_schema | std.contract.Equal "octet-aeneas-lean-provider-closure/v1",
    profile_id | std.contract.Equal "octet-aeneas-lean-0.1.0-bc2eb6f",
    profile_blake3 | HexDigest,
    adequacy_policy_id | std.contract.Equal "octet.aeneas_lean_proof_rail.theorem_adequacy",
    adequacy_policy_blake3 | HexDigest,
    compatibility_matrix_id | std.contract.Equal "octet-aeneas-lean-compatibility-0.1.0-bc2eb6f-cb50ff16",
    compatibility_matrix_blake3 | HexDigest,
    compatibility_schema | std.contract.Equal "octet-aeneas-lean-compatibility-matrix/v1",
    adapter_schema | std.contract.Equal "octet-aeneas-lean-valence-adapter/v1",
    adapter_blake3 | HexDigest,
    aeneas_revision | Revision,
    charon_revision | Revision,
    charon_pin_blake3 | HexDigest,
    aeneas_binary_blake3 | HexDigest,
    charon_binary_blake3 | HexDigest,
    lean_binary_blake3 | HexDigest,
    lake_binary_blake3 | HexDigest,
    leantar_binary_blake3 | HexDigest,
    network_sandbox_blake3 | HexDigest,
  },
  valence | {
    revision | Revision,
    profile | std.contract.Equal "aeneas-lean/v1",
    chain_schema | std.contract.Equal "valence.formal-proof.aeneas-lean-chain.v1",
    status_for_blocked_pilot | std.contract.Equal "not-applicable-no-property-chain",
  },
} in
{
  schema_version = "ucan-aeneas-provider-identity/v1",
  octet = {
    revision = "ef18352bf579bb7c6eb39070d917e3d005385a61",
    package = "octet#octet-aeneas-lean-provider-closure",
    closure_schema = "octet-aeneas-lean-provider-closure/v1",
    profile_id = "octet-aeneas-lean-0.1.0-bc2eb6f",
    profile_blake3 = "cd11b49d1acede2c6a91fa26f0b7a6b4b28db4007b4601a65b3269706a0ce625",
    adequacy_policy_id = "octet.aeneas_lean_proof_rail.theorem_adequacy",
    adequacy_policy_blake3 = "f19f3cc971b6802083d721daf72e77b9d8e44d7cccce300cb52d3a35f7e9ef54",
    compatibility_matrix_id = "octet-aeneas-lean-compatibility-0.1.0-bc2eb6f-cb50ff16",
    compatibility_matrix_blake3 = "403656d75de41b955cd2493ef289ba17308da65a3d214babc1ea606aeae248f5",
    compatibility_schema = "octet-aeneas-lean-compatibility-matrix/v1",
    adapter_schema = "octet-aeneas-lean-valence-adapter/v1",
    adapter_blake3 = "19a10c8d3e4ce29b4a058bd2202ff52fc42cee7508f04312840d0bfd398b00a8",
    aeneas_revision = "bc2eb6f941369fa3c695e30598b5758b11e4f0de",
    charon_revision = "cb50ff16b9f1066b8a97dc06da704de2da2fa41c",
    charon_pin_blake3 = "c7eb373f69fb075192a96fe193c9e60b58bca251bbeb9885e1d9fee08a0b13ba",
    aeneas_binary_blake3 = "428ce05d0f32c3bc3eceb24c1ead80b0dc742bfda22cba9c4b91b6a7f40a3819",
    charon_binary_blake3 = "64ee129c4553574e1686d87560af0699dba9010c998dd68fe4a6b25f6b2e64b5",
    lean_binary_blake3 = "e9e24ff8e9efd3e7f7a692d2411e0bb39316b626aeda1c3f1bce21bf21daea5d",
    lake_binary_blake3 = "9c0b74bac6d9436481da3be2f158f9abfb76b940a18aa723d2c493b29ac9f2b5",
    leantar_binary_blake3 = "b9f350bec0f839bb2e6157fdac2c8cc877f26a961ce81b16e5bcc97957d257fe",
    network_sandbox_blake3 = "c87d8bcc11f0c5d61e2598dbc08b801d7e1220753ba47b543525835bc1288b6b",
  },
  valence = {
    revision = "8eccac35fcd8aae8ef64b3d1021e7f1a53656180",
    profile = "aeneas-lean/v1",
    chain_schema = "valence.formal-proof.aeneas-lean-chain.v1",
    status_for_blocked_pilot = "not-applicable-no-property-chain",
  },
} | ProviderIdentity
