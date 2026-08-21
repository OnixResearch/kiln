let Source = {
  schema | String,
  repository | String,
  revision | String,
  crate | String,
  license | String,
  fixture | String,
  contract | String,
  non_claims | Array String,
} in
({
  schema = "kiln.cairn-gate-fixture-source.v1",
  repository = "ssh://git@github.com/OnixResearch/cairn.git",
  revision = "15f00875562025e7ea7e0d1f4af24d1a2e2ac06f",
  crate = "cairn-gate-contract",
  license = "MIT OR Apache-2.0",
  fixture = "producer-fixtures-v1.json",
  contract = "contract.ncl",
  non_claims = [
    "fixture identity does not prove lifecycle gate correctness",
    "producer fixtures do not grant executable, filesystem, or policy authority",
    "fixture admission does not authorize merge, publication, or release execution",
  ],
} | Source)
