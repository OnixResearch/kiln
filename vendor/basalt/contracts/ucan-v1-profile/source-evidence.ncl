let Contracts = import "contracts.ncl" in
{
  schema = "basalt.ucan-v1-source-evidence.v1",
  protocol_profile = "ucan-wg-1.0.0",
  runtime_revision = Contracts.identity.runtime_revision,
  support_catalog_blake3 = Contracts.identity.support_catalog_blake3,
  source_manifest_blake3 = Contracts.identity.source_manifest_blake3,
  wire_archive = {
    evidence_blake3 = Contracts.identity.wire_evidence_blake3,
    spec_blake3 = Contracts.identity.wire_spec_blake3,
    verdict = "PASS",
  },
  authorization_archive = {
    evidence_blake3 = Contracts.identity.authorization_evidence_blake3,
    spec_blake3 = Contracts.identity.authorization_spec_blake3,
    verdict = "PASS",
  },
  profile_supported = true,
  conformance_tested = true,
  licenses = Contracts.licenses,
} | Contracts.SourceEvidence
