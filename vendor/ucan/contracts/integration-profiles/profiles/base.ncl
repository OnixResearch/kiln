let MaximumProofDepth = 16 in
let MaximumProofCount = 64 in
let MaximumInputBytes = 65536 in
{
  schema = "ucan.integration-support-profile.v1",
  id = "ucan.controlled-ed25519-did-key",
  catalog_blake3 = "2bd91f2d23540781ded43e409130bb23dd77785ac641a33394f8514edd2c6f25",
  required_surfaces = [
    "ed25519-did-key-compact-token",
    "caller-supplied-key-resolution",
    "caller-supplied-proof-store",
    "caller-supplied-revocation",
    "caller-supplied-replay-admission",
    "holder-signed-invocation-admission",
    "verified-proof-traversal-orchestration",
    "opaque-caveat-policy-evaluation",
  ],
  admitted_preview_surfaces = [],
  did_methods = ["did:key"],
  signing_algorithms = ["EdDSA"],
  key_classes = ["Ed25519"],
  verification_limits = {
    id = "controlled-default-bounds",
    max_proof_depth = MaximumProofDepth,
    max_proof_count = MaximumProofCount,
    max_input_bytes = MaximumInputBytes,
  },
  integrations = {
    key_resolution = "caller-supplied",
    proof_loading = "caller-supplied",
    revocation = "caller-supplied",
    replay = "caller-supplied",
    verification_time = "explicit",
    caveat_policy = "caller-supplied",
    signer_custody = "caller-owned",
    backend_failure = "fail-closed",
  },
  fail_closed = true,
  non_claims = [
    "Profile conformance does not prove resolver truth.",
    "Profile conformance does not prove key custody or cryptographic correctness.",
    "Profile conformance does not prove revocation or replay freshness.",
    "Profile conformance does not prove backend durability.",
    "Profile conformance does not prove application authorization completeness.",
  ],
}
