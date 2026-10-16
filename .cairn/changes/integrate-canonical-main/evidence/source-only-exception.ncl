{
  schema = "kiln.source-publication-exception.v1",
  status = "operator-authorized-source-only",
  owner = "Kiln maintainers",
  authority = {
    operator_selection = "Authorize source-only exception",
    scope = "Publish both history-preserving main refs only after all other integrated checks pass; preserve the failed traceability result and make no production-readiness claim"
  },
  candidate = "d11c5592ff990594d3ad53e8dd80240f6ad8e927",
  verified_source_manifest_blake3 = "e41eaf62f46bdbaf47a0e8890c26c059551515bf1f695f9716b1a090aec00d2c",
  verified_source_archive_blake3 = "5faa879fa3c23087c37819e802bd6fbd5d5bccdf54a896c7b775cffd840b442a",
  failed_gate = {
    command = "cairn traceability coverage",
    exit_code = 1,
    report = "traceability-coverage.json",
    requirements = 625,
    referenced = 622,
    missing = [
      "kiln.aspen_host.production_evidence",
      "kiln.aspen_host.production_evidence.accepted",
      "kiln.aspen_host.production_evidence.rejected"
    ],
    dangling = []
  },
  reason = "Public source integration preserves the accepted production-evidence obligations but does not supply their current positive/negative qualification",
  recurrence_decision = "One exact checked source candidate and its evidence-only descendants; not a reusable gate waiver",
  revisit_trigger = "Before any production eligibility claim, deployment authorization, or publication of changed executable/configuration source",
  required_followup_owner = "Kiln maintainers must qualify the existing production-evidence obligations separately before making a production claim",
  capability_non_claims = [
    "No production eligibility",
    "No new runtime or deployment authority",
    "No live Onix shadow or delta acceptance",
    "No global durability, exactly-once delivery, provider truth, CI correctness, sandboxing, or availability claim",
    "The default traceability gate remains failing; no policy or marker change turns this exception into PASS"
  ]
}
