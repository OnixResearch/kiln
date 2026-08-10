let NonEmptyString = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in

let NonNegativeNumber = std.contract.from_predicate (fun value =>
  std.is_number value && value >= 0
) in

let Blake3Hex = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
) in

let Classification = std.contract.from_predicate (fun value =>
  std.is_string value && (
    value == "exact_maintained_evidence"
    || value == "partial_evidence_only"
    || value == "no_exact_evidence_identified"
  )
) in

let RequirementId = std.contract.from_predicate (fun value =>
  std.is_string value
  && std.string.is_match "^[A-Za-z0-9][A-Za-z0-9._-]*[A-Za-z0-9]$" value
  && !std.string.is_match "\\.\\." value
) in

let SafeRelativePath = std.contract.from_predicate (fun value =>
  std.is_string value
  && std.string.is_match "^[A-Za-z0-9][A-Za-z0-9._/-]*$" value
  && !std.string.is_match "(^|/)\\.?\\.(/|$)" value
) in

let RationaleCode = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^[a-z0-9][a-z0-9.-]*$" value
) in

let EvidenceReference = {
  path | SafeRelativePath,
  blake3 | Blake3Hex,
} in

let ReviewDecision = {
  requirement_id | RequirementId,
  classification | Classification,
  evidence | Array EvidenceReference,
  rationale_code | RationaleCode,
} in

{
  ReviewPolicy = {
    schema_version | NonEmptyString,
    hash_algorithm | NonEmptyString,
    expected_missing_count | NonNegativeNumber,
    expected_missing_hash | Blake3Hex,
    reviewed | Array ReviewDecision,
    non_claims | Array NonEmptyString,
  },
}
