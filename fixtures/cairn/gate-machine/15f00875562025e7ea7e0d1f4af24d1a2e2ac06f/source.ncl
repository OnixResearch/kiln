let Source = {
  schema | String,
  repository | String,
  revision | String,
  executable_blake3 | String,
  policy_blake3 | String,
  pass_fixture | String,
  block_fixture | String,
  non_claims | Array String,
} in
({
  schema = "kiln.cairn-gate-machine-fixture-source.v1",
  repository = "ssh://git@github.com/OnixResearch/cairn.git",
  revision = "15f00875562025e7ea7e0d1f4af24d1a2e2ac06f",
  executable_blake3 = "9ade7248f9f48a0a83fc21365cc45550b09a048621ac925292ed974dbc4951d0",
  policy_blake3 = "be930a118f405d176ea56de5c72ba05b8061ab3a4f996113cde505710cce5c74",
  pass_fixture = "tasks-pass.json",
  block_fixture = "tasks-block.json",
  non_claims = [
    "machine fixtures record bounded local gate observations",
    "fixture capture does not prove candidate truth or executable availability",
    "fixture admission does not grant merge, publication, or release authority",
  ],
} | Source)
