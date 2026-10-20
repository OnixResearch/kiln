# Verified bridge coverage inventory

This inventory is the guard-readable source of truth for UCAN verified bridge coverage. It separates live verifier/issuer routing from public helper APIs and intentionally unrouted vendored modules. It deliberately omits vendored `source_repo` metadata and does not include token values, secrets, private keys, credentials, or compact UCAN strings.

## Coverage classes

- `live-path`: UCAN shell code calls the bridge while issuing, parsing, constructing, or verifying live token facts. The claim is limited to the finite data-only predicate named by the bridge.

- `public-helper`: exported for advanced callers that provide explicit finite facts, but not a live verifier guarantee by itself.

- `candidate`: exact executable bridge exists and the shell already supplies matching finite facts; this sweep found no remaining candidates.

- `documented-boundary`: intentionally not routed or exposed because no exact executable bridge exists or routing would overclaim.

## Public verified exports

| Export | Class | Evidence |
| --- | --- | --- |
| `TokenEnvelopeInput` | `live-path` | `src/token.rs` reference |
| `TokenTimeWindow` | `live-path` | `src/token.rs` reference |
| `TokenValidationFailure` | `live-path` | `src/token.rs` reference |
| `VerifiedCapability` | `live-path` | `src/token.rs` reference |
| `VerifiedCaveatEvaluationFact` | `live-path` | `src/token.rs` reference |
| `VerifiedCaveatEvaluationTranscript` | `live-path` | `src/token.rs` reference |
| `VerifiedCaveatPreservation` | `live-path` | `src/token.rs` reference |
| `VerifiedChainEntry` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `VerifiedDidKey` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `VerifiedInvocationChainRequest` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `VerifiedParsedCapabilityListShape` | `live-path` | `src/token.rs` reference |
| `VerifiedParsedCapabilityShape` | `live-path` | `src/token.rs` reference |
| `VerifiedParsedCaveatListShape` | `live-path` | `src/token.rs` reference |
| `VerifiedParsedCaveatPayloadFact` | `live-path` | `src/token.rs` reference |
| `VerifiedParsedCaveatPayloadList` | `live-path` | `src/token.rs` reference |
| `VerifiedParsedCaveatShape` | `live-path` | `src/token.rs` reference |
| `VerifiedParsedClaimsPresence` | `live-path` | `src/token.rs` reference |
| `VerifiedParsedDidClaim` | `live-path` | `src/token.rs` reference |
| `VerifiedParsedDidClaims` | `live-path` | `src/token.rs` reference |
| `VerifiedParsedProofReferenceEntryShape` | `live-path` | `src/token.rs` reference |
| `VerifiedParsedProofReferenceListShape` | `live-path` | `src/token.rs` reference |
| `VerifiedParsedTemporalOrder` | `live-path` | `src/token.rs` reference |
| `VerifiedPrimitiveError` | `live-path` | `src/token.rs` reference |
| `VerifiedProofReferenceCollection` | `live-path` | `src/token.rs` reference |
| `VerifiedProofReferenceDigest` | `live-path` | `src/token.rs` reference |
| `VerifiedProofStoreResultFact` | `live-path` | `src/token.rs` reference |
| `VerifiedProofTraversalBudget` | `live-path` | `src/token.rs` reference |
| `VerifiedProofTraversalEntry` | `live-path` | `src/token.rs` reference |
| `VerifiedProofTraversalTarget` | `live-path` | `src/token.rs` reference |
| `VerifiedProofTraversalTranscript` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `VerifiedResult` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `VerifiedRevocationCheckFact` | `live-path` | `src/token.rs` reference |
| `VerifiedWebOfTrustAttestation` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `VerifiedWebOfTrustAuthorization` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `VerifiedWebOfTrustCaveat` | `live-path` | `src/token.rs` reference |
| `VerifiedWebOfTrustCaveatContext` | `live-path` | `src/token.rs` reference |
| `VerifiedWebOfTrustRevocation` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `VerifiedWebOfTrustUcanAuthorizationRequest` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `attenuation_chain_is_valid` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `backend_transcript_accepts` | `live-path` | `src/token.rs` reference |
| `backend_transcript_first_failed_operation` | `live-path` | `src/token.rs` reference |
| `capability_set_authorizes` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `caveat_evaluation_first_failed_index` | `live-path` | `src/token.rs` reference |
| `caveat_evaluation_transcript_accepts` | `live-path` | `src/token.rs` reference |
| `caveat_preservation_is_valid` | `live-path` | `src/token.rs` reference |
| `compose_time_window_chain` | `live-path` | `src/token.rs` reference |
| `decode_did_key` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `derive_ed25519_public_key` | `live-path` | `src/token.rs` reference; uses fixed arrays after vendored length admission. |
| `encode_ed25519_did_key` | `live-path` | `src/token.rs` reference |
| `invocation_chain_is_authorized` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `invocation_request_is_authorized` | `public-helper` | `src/lib.rs` public export only; structural invoker/audience equality is not holder possession. |
| `parsed_capability_list_shape_is_valid` | `live-path` | `src/token.rs` reference |
| `parsed_caveat_list_shape_is_valid` | `live-path` | `src/token.rs` reference |
| `parsed_caveat_payloads_are_valid` | `live-path` | `src/token.rs` reference |
| `parsed_claims_presence_is_valid` | `live-path` | `src/token.rs` reference |
| `parsed_did_claims_are_valid` | `live-path` | `src/token.rs` reference |
| `parsed_proof_reference_list_shape_is_valid` | `live-path` | `src/token.rs` reference |
| `parsed_temporal_order_is_valid` | `live-path` | `src/token.rs` reference |
| `proof_depth_within_budget` | `live-path` | `src/token.rs` reference; routes shell proof-hop depth conversion and overflow rejection through `verified_logic::ucan_delegation_depth::check_proof_hop_depth_budget`. |
| `proof_link_is_valid` | `live-path` | `src/token.rs` reference |
| `proof_reference_collection_is_valid` | `live-path` | `src/token.rs` reference |
| `proof_reference_digest_is_valid` | `live-path` | `src/token.rs` reference |
| `proof_store_result_is_valid` | `live-path` | `src/token.rs` reference |
| `proof_traversal_transcript_is_valid` | `live-path` | `src/token.rs` proof-chain transcript emission and public export |
| `receipt_execution_is_valid` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `revocation_check_is_valid` | `live-path` | `src/token.rs` reference |
| `revocation_reference_is_listed` | `live-path` | `src/token.rs` reference |
| `sign_ed25519` | `live-path` | `src/token.rs` reference; calls `ed25519-dalek` with fixed arrays after vendored length admission. |
| `signer_algorithm_is_valid` | `live-path` | `src/token.rs` reference before signer identity/key checks; routes exact `EdDSA` byte admission through `verified_logic::ucan_header::supported_algorithm_is_valid`. |
| `structural_chain_is_valid` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `structural_chain_not_revoked_is_valid` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `token_needs_refresh` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `token_time_valid` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `token_time_valid_at` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `typed_caveat_ascii_decimal_payload_is_valid` | `live-path` | `src/token.rs` reference before typed time-bound payload numeric parsing |
| `typed_caveat_is_satisfied` | `live-path` | `src/token.rs` reference |
| `typed_caveat_rate_limit_payload_shape_is_valid` | `live-path` | `src/token.rs` reference before typed rate-limit payload UTF-8/u64 parsing |
| `typed_caveat_set_is_satisfied` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `validate_token_envelope` | `live-path` | `src/token.rs` reference |
| `verify_ed25519_signature` | `live-path` | `src/token.rs` reference; verifies borrowed message and signature slices without bearer-sized `Vec` copies. |
| `web_of_trust_attestation_is_active_at` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `web_of_trust_attestation_payload_bytes` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `web_of_trust_attestation_signature_is_valid` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `web_of_trust_principal_digest` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `web_of_trust_principal_digest_is_valid` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `web_of_trust_revocation_is_effective_at` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `web_of_trust_revocation_matches_attestation` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `web_of_trust_revocation_payload_bytes` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `web_of_trust_revocation_signature_is_valid` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `web_of_trust_threshold_accepts` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |
| `web_of_trust_ucan_authorization` | `public-helper` | `src/lib.rs` public export only / advanced caller helper |

## Vendored UCAN / WOT modules

| Vendored module | Class | Notes |
| --- | --- | --- |
| `ucan_ability` | `live-path` | Imported by `src/verified.rs`; associated ability containment facts are routed by live attenuation/proof-link checks, and `VerifiedCapability::{has_wildcard_segment,is_concrete}` now route wildcard/concrete admission through this module. |
| `ucan_backend_transcript` | `live-path` | Imported by `src/verified.rs`; live key-resolution/proof-load/revocation/replay backend facts route through transcript aggregation helpers. |
| `ucan_base64url` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `ucan_capability_merge` | `public-helper` | Imported by `src/verified.rs` for explicit caller-supplied WOT helper APIs, not a live compact-token verifier guarantee by itself. |
| `ucan_caveat` | `live-path` | Imported by `src/verified.rs`; live typed caveat payload-shape admission and satisfaction route through this module for the finite supported time/rate vocabulary. |
| `ucan_caveat_evaluation` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `ucan_caveat_preservation` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `ucan_chain` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `ucan_cid` | `documented-boundary` | Vendored module is proof/spec/model-oriented or lacks exact executable UCAN shell bridge; routing now would overclaim. |
| `ucan_claims_presence` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `ucan_compact` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `ucan_delegation_depth` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `ucan_did` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `ucan_header` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `ucan_invocation` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `ucan_json` | `live-path` | Imported by `src/verified.rs`; live decoded header/payload admission routes first/last significant-byte discovery and object-envelope checks through this module before serde parsing. |
| `ucan_parsed_capability_shape` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `ucan_parsed_caveat_payload` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `ucan_parsed_caveat_shape` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `ucan_parsed_did_claim` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `ucan_principal_link` | `documented-boundary` | Vendored module is proof/spec/model-oriented or lacks exact executable UCAN shell bridge; routing now would overclaim. |
| `ucan_proof_reference` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `ucan_proof_reference_collection` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `ucan_proof_reference_list_shape` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `ucan_proof_store_result` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `ucan_proof_traversal` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `ucan_revocation` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `ucan_revocation_check` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `ucan_signature` | `live-path` | Imported by `src/verified.rs`; Ed25519 byte-length admission routes through `secret_key_len_is_valid`, `public_key_len_is_valid`, and `signature_len_is_valid`; associated signing/verification bridge facts are routed by live token issuance/verification paths where applicable. |
| `ucan_temporal` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `ucan_temporal_order` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `ucan_token` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `ucan_verification_stage` | `live-path` | Imported by `src/verified.rs`; associated bridge facts are routed by live token issuance/verification or verifier transcript paths where applicable. |
| `web_of_trust_attestation` | `public-helper` | Imported by `src/verified.rs` for explicit caller-supplied WOT helper APIs, not a live compact-token verifier guarantee by itself. |
| `web_of_trust_chain` | `documented-boundary` | Vendored module is proof/spec/model-oriented or lacks exact executable UCAN shell bridge; routing now would overclaim. |
| `web_of_trust_policy` | `public-helper` | Imported by `src/verified.rs` for explicit caller-supplied WOT helper APIs, not a live compact-token verifier guarantee by itself. |
| `web_of_trust_principal` | `public-helper` | Imported by `src/verified.rs` for explicit caller-supplied WOT helper APIs, not a live compact-token verifier guarantee by itself. |
| `web_of_trust_resolution` | `documented-boundary` | Vendored module is proof/spec/model-oriented or lacks exact executable UCAN shell bridge; routing now would overclaim. |
| `web_of_trust_revocation` | `public-helper` | Imported by `src/verified.rs` for explicit caller-supplied WOT helper APIs, not a live compact-token verifier guarantee by itself. |
| `web_of_trust_ucan_authorizer` | `public-helper` | Imported by `src/verified.rs` for explicit caller-supplied WOT helper APIs, not a live compact-token verifier guarantee by itself. |

## Candidate outcome

No `candidate` entries remain after this sweep. The only unrouted vendored UCAN-adjacent modules identified here are documented boundaries: `ucan_cid`, `ucan_principal_link`, `web_of_trust_chain`, and `web_of_trust_resolution`. In particular, `ucan_cid` and `ucan_principal_link` do not provide an exact executable UCAN shell bridge for the current BLAKE3 proof-reference and principal-link runtime surfaces, so exposing them now would overclaim hash, CID, DID, or identity semantics.

## Non-claims

These classifications do not verify parser correctness, base64url decoded-byte correctness, serde JSON semantics, duplicate-key handling, canonicalization, BLAKE3 or Ed25519 internals, resolver/proof-store/revocation backend truth or freshness, clock truth, DID-method semantics, CID semantics, principal-link semantics, arbitrary caveat-policy correctness, storage/network behavior, replay protection, persistence, token provenance, or runtime orchestration outside the finite facts passed to vendored predicates.
