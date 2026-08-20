let Contracts = import "../contracts.ncl" in
({
  source_revision = "220307a371e7bc4e3309447b09447d0fe6c4db5f",
  artifact_auth_revision = "e41340bec587b6d049b5cc518ec7db925dde84be",
  evidence_ir_schema = "valence.evidence-ir.v1",
  sidecar_schema = "valence.stack-provenance-sidecar.v1",
  receipt_schema = "valence.receipt-transcript.v1",
  role_registry = "valence-core::stack_role_registry@220307a371e7bc4e3309447b09447d0fe6c4db5f",
  span_registry = "valence-core::evidence_ir::EvidenceSpan@220307a371e7bc4e3309447b09447d0fe6c4db5f",
  license = "MPL-2.0",
  subject_kinds = ["attempt"],
  identity_roles = ["authority"],
  span_kinds = ["event"],
  required_non_claims = ["invalid role fixture"],
  bounds = {
    max_artifact_bytes = 1,
    max_facts = 1,
    max_roles = 1,
    max_text_bytes = 1,
    max_response_bytes = 1,
  },
}) | Contracts.ValenceProfile
