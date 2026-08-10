let Contracts = import "../traceability-inventory-contracts.ncl" in
{
  schema_version = "lattice.traceability-debt-inventory.v1",
  hash_algorithm = "BLAKE3",
  expected_missing_count = 1,
  expected_missing_hash = "0000000000000000000000000000000000000000000000000000000000000000",
  reviewed = [{
    requirement_id = "lattice.example.scenario.invalid",
    classification = "exact_maintained_evidence",
    evidence = [{
      path = "../outside.rs",
      blake3 = "0000000000000000000000000000000000000000000000000000000000000000",
    }],
    rationale_code = "invalid.path",
  }],
  non_claims = [
    "inventory rows do not satisfy requirements or create Tracey evidence",
    "unassessed does not mean behavior or evidence is absent",
    "a valid inventory does not prove semantic correctness, complete traceability, release readiness, or production readiness",
  ],
} | Contracts.ReviewPolicy
