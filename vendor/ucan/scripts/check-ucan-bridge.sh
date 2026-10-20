#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

require_pattern() {
  local pattern=$1
  local path=$2
  if ! grep -RInE "$pattern" "$path" >/dev/null; then
    echo "missing required bridge pattern '$pattern' in $path" >&2
    return 1
  fi
}

reject_pattern() {
  local pattern=$1
  local path=$2
  if grep -RInE "$pattern" "$path"; then
    echo "forbidden local semantic implementation matched '$pattern' in $path" >&2
    return 1
  fi
}

require_pattern 'use verified_logic::ucan_ability|VerifiedCapability::from_document|\.contains\(|ability_has_glob_segment|ability_is_concrete' src
require_pattern 'use verified_logic::ucan_capability_merge|capability_set_authorizes|check_authorized_by' src
require_pattern 'use verified_logic::ucan_temporal|compose_time_window_chain' src
require_pattern 'use verified_logic::ucan_signature|verify_ed25519_signature|sign_ed25519|secret_key_len_is_valid|public_key_len_is_valid|signature_len_is_valid' src
require_pattern 'use verified_logic::ucan_token|validate_token_envelope' src
require_pattern 'use verified_logic::ucan_compact|compact_token_split_is_valid|compact_token_envelope_is_valid|compact_split_matches_parts|compact_envelope_matches_parts|VerifiedCompactTokenEnvelope' src
require_pattern 'use verified_logic::ucan_base64url|base64url_segment_is_valid|base64url_no_pad_segment_is_valid|VerifiedBase64UrlSegment' src
require_pattern 'decode_part|base64url_segment_is_valid|failed verified base64url-no-pad structural admission' src/token.rs
require_pattern 'use verified_logic::ucan_json|json_object_bytes_are_valid|verified_json_object_bytes|json_object_significant_indexes|VerifiedJsonObjectBytes' src
require_pattern 'decode_json_part|validate_decoded_json_object_bytes|verified_json_object_bytes|failed verified decoded JSON object structural admission' src/token.rs
require_pattern 'use verified_logic::ucan_header|parsed_header_is_valid|parsed_header_admission_is_valid|signer_algorithm_is_valid|supported_algorithm_is_valid|VerifiedParsedHeader' src
require_pattern 'validate_signer_algorithm|signer_algorithm_is_valid|UnsupportedAlgorithm' src/token.rs
require_pattern 'use verified_logic::ucan_claims_presence|parsed_claims_presence_is_valid|VerifiedParsedClaimsPresence|ParsedClaimsPresence' src
require_pattern 'use verified_logic::ucan_temporal_order|parsed_temporal_order_is_valid|VerifiedParsedTemporalOrder|ParsedTemporalOrder' src
require_pattern 'use verified_logic::ucan_parsed_did_claim|parsed_did_claims_are_valid|VerifiedParsedDidClaims|ParsedDidClaimsFact' src
require_pattern 'use verified_logic::ucan_proof_reference_list_shape|parsed_proof_reference_list_shape_is_valid|VerifiedParsedProofReferenceListShape|ParsedProofReferenceEntryShape' src
require_pattern 'use verified_logic::ucan_proof_reference_collection|proof_reference_collection_is_valid|VerifiedProofReferenceCollection|ProofReferenceCollectionFact' src
require_pattern 'use verified_logic::ucan_parsed_capability_shape|parsed_capability_list_shape_is_valid|VerifiedParsedCapabilityListShape|ParsedCapabilityShapeFact' src
require_pattern 'use verified_logic::ucan_parsed_caveat_shape|parsed_caveat_list_shape_is_valid|VerifiedParsedCaveatListShape|ParsedCaveatShapeFact' src
require_pattern 'use verified_logic::ucan_parsed_caveat_payload|parsed_caveat_payloads_are_valid|VerifiedParsedCaveatPayloadList|ParsedCaveatPayloadFact' src
require_pattern 'use verified_logic::ucan_proof_store_result|proof_store_result_is_valid|VerifiedProofStoreResultFact|ProofStoreResultFact' src
require_pattern 'use verified_logic::ucan_revocation_check|revocation_check_is_valid|VerifiedRevocationCheckFact|RevocationCheckFact' src
require_pattern 'signing_input_byte_provenance_is_valid|VerifiedSigningInputByteProvenance|compact_signing_input' src
require_pattern 'validate_header|parsed_header_is_valid|VerifiedParsedHeader|UnsupportedHeader' src/token.rs
require_pattern 'validate_parsed_claims_presence|parsed_claims_presence_is_valid|VerifiedParsedClaimsPresence|MissingProofs|proofs_present|MissingExpiration|MissingNotBefore' src/token.rs
require_pattern 'validate_parsed_did_claims|parsed_did_claims_are_valid|VerifiedParsedDidClaims|InvalidDidClaims|non_empty|trimmed' src/token.rs
require_pattern 'validate_parsed_temporal_order|parsed_temporal_order_is_valid|VerifiedParsedTemporalOrder|InvalidTemporalOrder|not_before_seconds|expires_at_seconds' src/token.rs
require_pattern 'decoded_proof_references|parsed_proof_reference_list_shape_is_valid|VerifiedParsedProofReferenceEntryShape|InvalidProofReferenceListShape' src/token.rs
require_pattern 'validate_proof_reference_collection|proof_reference_collection_is_valid|InvalidProofReferenceCollection|ProofCountExceeded' src/token.rs
require_pattern 'validate_parsed_capability_list_shape|parsed_capability_list_shape_is_valid|InvalidCapabilityListShape|resource_present|ability_present' src/token.rs
require_pattern 'validate_parsed_caveat_list_shape|parsed_caveat_list_shape_is_valid|InvalidCaveatListShape|payload_present' src/token.rs
require_pattern 'decoded_caveat_inputs|parsed_caveat_payloads_are_valid|VerifiedParsedCaveatPayloadFact|InvalidCaveatPayloadAdmission|decoded_non_empty' src/token.rs
require_pattern 'resolve_proof|proof_store_result_is_valid|VerifiedProofStoreResultFact|InvalidProofStoreResultTranscript' src/token.rs
require_pattern 'reject_revoked_token|revocation_check_is_valid|VerifiedRevocationCheckFact|InvalidRevocationCheckTranscript' src/token.rs
require_pattern 'validate_signing_input_byte_provenance|signing_input_byte_provenance_is_valid|InvalidSigningInputProvenance' src/token.rs
require_pattern 'verify_successful_verification_stage_flow|verification_stage_transcript_outcome|InvalidVerificationStageFlow|VerifiedVerificationStage::Accepted' src/token.rs
require_pattern 'use verified_logic::ucan_proof_reference|proof_reference_digest_is_valid|proof_reference_admits_digest|proof_reference_digest_from_bytes|VerifiedProofReferenceDigest' src
require_pattern 'proof_reference_digest_is_valid|proof_reference_for_token|blake3::hash|VerifiedProofReferenceDigest' src/token.rs
require_pattern 'use verified_logic::ucan_verification_stage|verification_stage_transcript_outcome|stage_transcript_accepts|first_failed_stage|VerifiedVerificationStageTranscript' src
require_pattern 'use verified_logic::ucan_backend_transcript|backend_transcript_accepts|backend_transcript_first_failed_operation|VerifiedBackendTranscript' src
require_pattern 'record_backend_operation|verify_backend_transcript_accepts|VerifiedBackendOperation::(KeyResolution|ProofLoad|Revocation)' src/token.rs
require_pattern 'use verified_logic::ucan_chain|proof_link_is_valid|structural_chain_is_valid|check_chain_structural_valid' src
require_pattern 'use verified_logic::ucan_delegation_depth|proof_depth_within_budget|check_proof_hop_depth_budget' src
require_pattern 'use verified_logic::ucan_revocation|revocation_reference_is_listed|structural_chain_not_revoked_is_valid|check_chain_structural_valid_not_revoked' src
require_pattern 'use verified_logic::ucan_invocation|invocation_request_is_authorized|invocation_chain_is_authorized|receipt_execution_is_valid|check_invocation_chain_authorized|check_receipt_valid' src
require_pattern 'use verified_logic::ucan_proof_traversal|proof_traversal_transcript_is_valid|traversal_transcript_valid' src
require_pattern 'proof_traversal_dag_transcript_is_valid|VerifiedProofTraversalDagTranscript|dag_traversal_transcript_valid' src/verified.rs
require_pattern 'verify_live_dag_proof_transcript|LiveProofTraversalTranscript|proof_traversal_dag_transcript_is_valid|parent_indices' src/token.rs
require_pattern 'use verified_logic::web_of_trust_principal|web_of_trust_principal_digest|web_of_trust_principal_digest_is_valid|principal_id_from_bytes|principal_id_from_digest_bytes' src
require_pattern 'use verified_logic::ucan_caveat|typed_caveat_is_satisfied|typed_caveat_set_is_satisfied|typed_caveat_ascii_decimal_payload_is_valid|typed_caveat_rate_limit_payload_shape_is_valid|ascii_decimal_payload_is_valid|rate_limit_payload_shape_is_valid|caveat_satisfied|caveat_set_satisfied' src
require_pattern 'use verified_logic::ucan_caveat_preservation|caveat_preservation_is_valid|parent_caveat_ids_preserved|VerifiedCaveatPreservation' src
require_pattern 'use verified_logic::ucan_caveat_evaluation|caveat_evaluation_transcript_accepts|first_failed_caveat_index|VerifiedCaveatEvaluationTranscript' src
require_pattern 'parent_caveats_are_preserved|caveat_identifier_set|caveat_preservation_is_valid|VerifiedCaveatPreservation' src/token.rs
require_pattern 'evaluate_capability_caveats|caveat_evaluation_transcript_accepts|caveat_evaluation_first_failed_index|VerifiedCaveatEvaluationFact' src/token.rs
require_pattern 'VerifiedTypedCaveatPolicySet|TypedCaveatContext|parse_time_bound_caveat|parse_rate_limit_caveat|typed_caveat_is_satisfied|typed_caveat_ascii_decimal_payload_is_valid|typed_caveat_rate_limit_payload_shape_is_valid' src/token.rs
require_pattern 'use verified_logic::web_of_trust_attestation|web_of_trust_attestation_payload_bytes|web_of_trust_attestation_signature_is_valid|web_of_trust_attestation_is_active_at|attestation_payload_bytes|attestation_signature_valid|attestation_active_at' src
require_pattern 'use verified_logic::web_of_trust_revocation|web_of_trust_revocation_payload_bytes|web_of_trust_revocation_signature_is_valid|web_of_trust_revocation_matches_attestation|web_of_trust_revocation_is_effective_at|revocation_payload_bytes|revocation_signature_valid|revocation_matches|revocation_effective_at' src
require_pattern 'use verified_logic::web_of_trust_policy|web_of_trust_threshold_accepts|threshold_accepts' src
require_pattern 'use verified_logic::web_of_trust_ucan_authorizer|web_of_trust_ucan_authorization' src
require_pattern 'use verified_logic::ucan_did|encode_ed25519_did_key|decode_did_key' src
require_pattern '\.is_concrete\(\)' src/token.rs
require_pattern '\.has_wildcard_segment\(\)' src/verified.rs

reject_pattern 'fn .*attenuat.*\{|fn .*temporal.*\{|fn .*signature.*\{' crates/ucan-core/src
reject_pattern 'fn json_significant_byte_indexes|fn is_json_whitespace_byte|bytes\.iter\(\)\.position\(\|byte\| !is_json_whitespace_byte\(\*byte\)\)|bytes\.iter\(\)\.rposition\(\|byte\| !is_json_whitespace_byte\(\*byte\)\)' src/token.rs
reject_pattern 'self\.ability_segments\.iter\(\)\.any\(|segment\.as_slice\(\) == b"\*"|WILDCARD_ABILITY_SEGMENT' src/verified.rs
reject_pattern 'actual == ucan_signature::SECRET_KEY_LEN|actual == ucan_signature::PUBLIC_KEY_LEN|actual == ucan_signature::SIGNATURE_LEN' src/verified.rs
reject_pattern 'parent\.audience\(\)\.as_str\(\) *!= *child_issuer\.as_str\(\)|child_issuer\.as_str\(\) *!= *parent\.audience\(\)\.as_str\(\)' src/token.rs
reject_pattern 'self\.as_slice\(\)\.iter\(\)\.any\(\|candidate\| candidate == reference\)|candidate == reference\)\)|iter\(\)\.any\(\|existing\| existing == &reference\)' src/token.rs
reject_pattern 'invoker: verified\.issuer\(\)\.clone\(\)' src/token.rs
reject_pattern 'if depth > limits\.max_proof_depth\(\)|depth > limits\.max_proof_depth\(\)|chain_len - 1 <= max_depth' src/token.rs
reject_pattern 'checked_add\(1\)|let Some\(chain_len\)|check_depth_budget\(chain_len, max_depth\)' src/verified.rs
reject_pattern 'fn parse_ascii_u64|payload\.iter\(\)\.all\(u8::is_ascii_digit\)|window_seconds\.contains\(' src/token.rs
reject_pattern 'OnceLock|lazy_static|thread_local!|std::env|reqwest|ureq|hyper|process-global|global resolver|global proof|default resolver|default proof store|filesystem fallback|network fallback' src

python3 - <<'PY'
from pathlib import Path
import re
import sys

root = Path.cwd()
coverage = (root / 'docs/verified-bridge-coverage.md').read_text()
lib = (root / 'src/lib.rs').read_text()
exports = re.findall(r'pub use verified::([A-Za-z0-9_]+);', lib)
valid_classes = {'live-path', 'public-helper', 'candidate', 'documented-boundary'}
missing = []
for name in exports:
    row = re.search(rf'^\| `{re.escape(name)}` \| `([^`]+)` \|', coverage, re.MULTILINE)
    if row is None:
        missing.append(f'missing export coverage row: {name}')
    elif row.group(1) not in valid_classes:
        missing.append(f'invalid export coverage class for {name}: {row.group(1)}')

modules = sorted(
    p.stem for pattern in ('ucan*.rs', 'web_of_trust*.rs')
    for p in (root / 'vendor/verified-logic/src').glob(pattern)
)
for module in modules:
    row = re.search(rf'^\| `{re.escape(module)}` \| `([^`]+)` \|', coverage, re.MULTILINE)
    if row is None:
        missing.append(f'missing vendored module coverage row: {module}')
    elif row.group(1) not in valid_classes:
        missing.append(f'invalid vendored module coverage class for {module}: {row.group(1)}')

for boundary in ('ucan_cid', 'ucan_principal_link'):
    row = re.search(rf'^\| `{boundary}` \| `([^`]+)` \|', coverage, re.MULTILINE)
    if row is None or row.group(1) != 'documented-boundary':
        missing.append(f'{boundary} must remain documented-boundary until exact executable bridge exists')

if 'source_repo=' in coverage:
    missing.append('coverage inventory must not preserve vendored source_repo metadata')

if re.search(r'^\| `[^`]+` \| `candidate` \|', coverage, re.MULTILINE):
    missing.append('coverage inventory must not retain candidate rows after the sweep')

if missing:
    for item in missing:
        print(item, file=sys.stderr)
    sys.exit(1)
PY

"$repo_root/scripts/check-ucan-core-no-std.sh"
