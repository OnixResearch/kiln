let Contracts = import "../contracts.ncl" in
{
  source = fun runtime profile manifest wire_verdict auth_verdict supported tested licenses =>
    {
      schema = "basalt.ucan-v1-source-evidence.v1",
      protocol_profile = profile,
      runtime_revision = runtime,
      support_catalog_blake3 = Contracts.identity.support_catalog_blake3,
      source_manifest_blake3 = manifest,
      wire_archive = {
        evidence_blake3 = Contracts.identity.wire_evidence_blake3,
        spec_blake3 = Contracts.identity.wire_spec_blake3,
        verdict = wire_verdict,
      },
      authorization_archive = {
        evidence_blake3 = Contracts.identity.authorization_evidence_blake3,
        spec_blake3 = Contracts.identity.authorization_spec_blake3,
        verdict = auth_verdict,
      },
      profile_supported = supported,
      conformance_tested = tested,
      licenses = licenses,
    } | Contracts.SourceEvidence,
  mapping = fun profile entries non_claims =>
    {
      schema = "basalt.ucan-v1-command-mapping.v1",
      protocol_profile = profile,
      runtime_revision = Contracts.identity.runtime_revision,
      support_catalog_blake3 = Contracts.identity.support_catalog_blake3,
      entries = entries,
      non_claims = non_claims,
    } | Contracts.Mapping,
  identity = Contracts.identity,
  licenses = Contracts.licenses,
  entries = Contracts.entries,
  non_claims = Contracts.non_claims,
}
