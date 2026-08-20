let Source = {
  schema | String,
  repository | String,
  revision | String,
  crate | String,
  license | String,
  fixture | String,
  non_claims | Array String,
} in
({
  schema = "kiln.lattice-workflow-fixture-source.v1",
  repository = "ssh://git@github.com/OnixResearch/lattice.git",
  revision = "e3148ec59fa12081c270d9800a060d94214b9405",
  crate = "lattice-workflow-contract",
  license = "MIT",
  fixture = "producer-fixtures-v1.json",
  non_claims = [
    "fixture identity does not prove workflow execution",
    "producer fixtures do not grant Lattice availability or authority",
    "transport recovery fixtures do not prove durable lookup or non-execution",
  ],
} | Source)
