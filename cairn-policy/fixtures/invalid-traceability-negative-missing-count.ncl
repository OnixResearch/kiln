let Contracts = import "../traceability-inventory-contracts.ncl" in
{
  schema_version = "lattice.traceability-debt-inventory.v1",
  hash_algorithm = "BLAKE3",
  expected_missing_count = -1,
  expected_missing_hash = "86a0d721f0469382362733937cb24599c20b620b51b53628bce4215c77a19f87",
  reviewed = [],
  non_claims = [
    "inventory rows do not satisfy requirements or create Tracey evidence",
  ],
} | Contracts.ReviewPolicy
