let Source = {
  schema | String,
  repository | String,
  revision | String,
  contract | String,
  license | String,
  fixture | String,
  consumption | String,
  non_claims | Array String,
} in
({
  schema = "kiln.mantle-build-fixture-source.v1",
  repository = "ssh://git@github.com/OnixResearch/mantle.git",
  revision = "0bbab77e23ab95caa4e4184b9945cfaa5c8040e2",
  contract = "mantle-build-contract",
  license = "MIT OR Apache-2.0",
  fixture = "producer-fixtures-v1.json",
  consumption = "independent-wire-consumer",
  non_claims = [
    "producer fixture admission does not prove build correctness",
    "producer fixtures grant no process, store, cache, or release authority",
    "wire compatibility does not prove runtime availability",
  ],
} | Source)
