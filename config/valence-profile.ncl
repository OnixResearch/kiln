let Contracts = import "contracts.ncl" in
let MaxArtifactBytes = 32768 in
let MaxFacts = 64 in
let MaxRoles = 16 in
let MaxTextBytes = 256 in
let MaxResponseBytes = 65536 in
({
  source_revision = "220307a371e7bc4e3309447b09447d0fe6c4db5f",
  artifact_auth_revision = "e41340bec587b6d049b5cc518ec7db925dde84be",
  evidence_ir_schema = "valence.evidence-ir.v1",
  sidecar_schema = "valence.stack-provenance-sidecar.v1",
  receipt_schema = "valence.receipt-transcript.v1",
  role_registry = "valence-core::stack_role_registry@220307a371e7bc4e3309447b09447d0fe6c4db5f",
  span_registry = "valence-core::evidence_ir::EvidenceSpan@220307a371e7bc4e3309447b09447d0fe6c4db5f",
  license = "MPL-2.0",
  subject_kinds = ["candidate", "pipeline", "run", "attempt", "product", "event", "receipt"],
  identity_roles = ["producer", "checker", "builder", "signer", "provider", "worker", "linker", "consumer"],
  span_kinds = ["source", "receipt", "event"],
  required_non_claims = [
    "does not claim behavioral correctness, semantic equivalence, or whole-program verification",
    "Kiln evidence records identity and linkage only; it does not establish CI correctness or release eligibility",
    "provider observations remain bounded external facts and do not prove provider correctness",
  ],
  bounds = {
    max_artifact_bytes = MaxArtifactBytes,
    max_facts = MaxFacts,
    max_roles = MaxRoles,
    max_text_bytes = MaxTextBytes,
    max_response_bytes = MaxResponseBytes,
  },
}) | Contracts.ValenceProfile
