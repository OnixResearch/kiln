## Phase 1: Token model and FCIS boundary

- [x] Define the full `ucan-core` claim model — issuer/audience fields, proof references, capability sets, temporal bounds, and the top-level token-claim composite — and integrate the existing capability-document surface into that model. [covers=library.token-round-trip library.fcis-boundary library.core-no-std]
- [x] Add structural validation for the new core claim model, including required-field failures, malformed-claim rejection, and explicit wildcard-segment alignment tests for whole-segment vs embedded-wildcard cases. [covers=library.token-round-trip library.fcis-boundary library.core-no-std library.delegation-chain]
- [x] Add named semantic wrapper types at the `ucan-core`/`ucan` seam for decoded claim roles and in the `ucan` public API for shell-only concepts, with compact-token string wrappers explicitly owned by `ucan` rather than `ucan-core`; add positive and negative tests so issuer DID, audience DID, compact token strings, proof collections, capability collections, and temporal bounds are not exposed primarily as raw primitives and malformed inputs fail deterministically. [covers=library.token-round-trip library.fcis-boundary]
- [x] Add stage-typed parse/validate/verify result types plus shell-side error/orchestration types so invalid token-workflow states are not representable and clocks, proof material, and key resolution stay explicit. [covers=library.token-round-trip library.fcis-boundary library.delegation-chain]
- [x] Add `scripts/check-ucan-core-no-std.sh` as the focused `ucan-core` no-std compile rail; it MUST run `nix develop -c cargo check -p ucan-core --lib --no-default-features`, compile a temporary `#![no_std]` smoke library that imports and uses the new token-claim validation API, and run a source audit that rejects `std`, JSON, compact-token codec, filesystem, network, or clock use in `crates/ucan-core`. [covers=library.core-no-std library.fcis-boundary]
- [x] Run `./scripts/check-ucan-core-no-std.sh` after Phase 1 changes, staging any new files with `git add -N <path>` before Crane-based checks. [covers=library.core-no-std library.fcis-boundary]

## Phase 2: Compact-token usability

- [x] Add shell-owned compact-token encode/decode support and keep any base64url/JSON dependencies out of `ucan-core`. [covers=library.token-round-trip library.fcis-boundary]
- [x] Add positive encode/decode tests proving issued tokens use `header.payload.signature` shape and exact initial claim keys `iss`, `aud`, `att`, `prf`, `exp`, and `nbf`. [covers=library.token-round-trip]
- [x] Add consumer-facing builder and parser entry points in `ucan` for issuing and decoding `did:key` / Ed25519 compact tokens through ergonomic public APIs. [covers=library.token-round-trip library.did-key-ed25519]
- [x] Make verification time, proof material, and key-resolution context explicit public verifier inputs, with positive tests for caller-supplied inputs and negative tests for missing or mismatched inputs. [covers=library.token-round-trip library.fcis-boundary library.delegation-chain]
- [x] Add public verifier entry points for `did:key` / Ed25519 compact tokens using those explicit inputs, and assert that the verified issuer DID equals the signing-key-derived `did:key`. [covers=library.token-round-trip library.did-key-ed25519]
- [x] Add a public verified-token result type and tests proving it exposes issuer, audience, capability set, proof list, and effective time window after successful verification. [covers=library.token-round-trip]
- [x] Add public parser/verifier negative tests for malformed compact-token structure, invalid base64url/JSON encoding, missing required claims, structured error classification, and no-panic behavior. [covers=library.token-round-trip]
- [x] Add builder-style token/proof test helpers so integration and property tests declare only the preconditions they need instead of inheriting general fixtures. [covers=library.token-round-trip library.delegation-chain]
- [x] Add a public API inventory in `docs/public-api.md` that classifies each existing primitive-heavy helper as retained supporting API or deprecated API. [covers=library.token-round-trip library.fcis-boundary]
- [x] Apply the chosen primitive-helper API treatment in code by deprecating, narrowing visibility, or moving helper-only surfaces behind supporting modules, then verify README examples compile using only the token-oriented workflow for the documented happy path. [covers=library.token-round-trip library.fcis-boundary library.consumer-onboarding]
- [x] Add README coverage for the architecture boundary and end-to-end issue/parse/verify flow, with onboarding ordered as architecture snapshot first, end-to-end flow second, and proof-chain deep dive third. [covers=library.consumer-onboarding]
- [x] Add `examples/issue_and_verify.rs` as the runnable end-to-end issue/parse/verify example target. [covers=library.consumer-onboarding library.token-round-trip]
- [x] Document supported DID methods, signing algorithms, proof-loading model, and notable out-of-scope UCAN features in the README support-limits section. [covers=library.consumer-onboarding]

## Phase 3: Delegation-chain verification

- [x] Implement shell-level proof traversal and issuer/audience linkage checks across parent tokens with explicit caller-supplied proof material. [covers=library.delegation-chain]
- [x] Add positive verification coverage for a valid parent/child proof chain where the parent audience equals the child issuer and end-to-end authorization succeeds. [covers=library.delegation-chain]
- [x] Implement structured self-delegation rejection in shell authorization so tokens whose issuer and audience are the same cannot verify as authorized. [covers=library.delegation-chain]
- [x] Implement temporal narrowing for proof chains through `vendor/verified-logic`-backed token-time helpers and add positive coverage for valid narrower-child delegation windows. [covers=library.delegation-chain]
- [x] Implement capability attenuation for proof chains through `vendor/verified-logic`-backed capability helpers and add positive coverage for valid narrower-child capability sets. [covers=library.delegation-chain]
- [x] Return and test effective delegation constraints after successful proof-chain verification, including the narrowed capability set and effective time window. [covers=library.delegation-chain]
- [x] Add explicit negative coverage for `r[library.delegation-chain.self-delegation-is-rejected]` so a directly verified token whose issuer and audience are the same fails with a structured authorization error. [covers=library.delegation-chain]
- [x] Add explicit negative coverage for a proof chain containing a self-delegating parent or ancestor token, asserting the same structured self-delegation error. [covers=library.delegation-chain]
- [x] Add negative tests for broken proof links, malformed compact tokens, signature mismatch, expired parents, and over-broad child capabilities, asserting proof-chain errors for broken/missing/wrong links, signature-related errors for signature mismatch, attenuation errors for over-broad capabilities, and temporal errors for expired or widened time windows. [covers=library.token-round-trip library.did-key-ed25519 library.delegation-chain]
- [x] Extend README coverage with the stabilized proof-chain flow and effective-delegation result shape. [covers=library.consumer-onboarding library.delegation-chain]
- [x] Add `examples/proof_chain.rs` as the runnable proof-chain example target. [covers=library.consumer-onboarding library.delegation-chain]

## Phase 4: Verification and release bar

- [x] Add `scripts/check-ucan-bridge.sh`; it MUST fail if attenuation, token-time, signature semantics, or wildcard-segment enforcement stop routing through `vendor/verified-logic`, or if `ucan-core` picks up shell-only concerns such as `std`, JSON, or compact-token codec dependencies. [covers=library.core-no-std library.fcis-boundary]
- [x] Add `scripts/check-doc-onboarding.sh`; it MUST verify README onboarding order is architecture snapshot first, end-to-end flow second, and proof-chain deep dive third, and it MUST verify the presence of supported DID methods, signing algorithms, proof-loading model, and out-of-scope UCAN feature statements. [covers=library.consumer-onboarding]
- [x] Add `scripts/check-readme-examples.sh`; it MUST fail when the README happy-path code no longer builds against the public token-oriented API. [covers=library.consumer-onboarding]
- [x] Run `nix develop -c cargo test --workspace --all-targets` and keep evidence that public-entry-point tests pass. [covers=library.token-round-trip library.delegation-chain]
- [x] Run `nix develop -c cargo run --example issue_and_verify` and record it as evidence for `r[library.consumer-onboarding.fresh-consumer-follows-the-documented-flow]`. [covers=library.consumer-onboarding]
- [x] Run `nix develop -c cargo run --example proof_chain` and record it as proof-chain onboarding evidence. [covers=library.consumer-onboarding library.delegation-chain]
- [x] Run `nix develop -c ./scripts/check-doc-onboarding.sh` and record it as evidence for `r[library.consumer-onboarding.docs-state-current-support-limits]`. [covers=library.consumer-onboarding]
- [x] Run `nix develop -c ./scripts/check-readme-examples.sh` and record it as README happy-path build evidence. [covers=library.consumer-onboarding]
- [x] Run `nix develop -c ./scripts/check-ucan-core-no-std.sh` and `nix develop -c ./scripts/check-ucan-bridge.sh` under the repo's Nix-provided Rust toolchain. [covers=library.core-no-std library.fcis-boundary]
- [x] Run `nix develop -c cargo generate-lockfile` after dependency changes and keep the tracked `Cargo.lock` current. [repo-guardrail]
- [x] Verify `Cargo.lock` is tracked with `git ls-files --error-unmatch Cargo.lock`. [repo-guardrail]
- [x] Audit dependency sources with `! grep -R "git+ssh\|ssh://" Cargo.toml crates/ucan-core/Cargo.toml Cargo.lock` so new dependencies stay CI-safe. [repo-guardrail]
- [x] Before Crane-based verification, stage each new file with `git add -N <path>`. [repo-guardrail]
- [x] Run `nix flake check` as the final Crane-based verification, relying on the existing scoped `cargo fmt`/`cargo clippy` checks for `ucan` and `ucan-core`. [covers=library.token-round-trip library.consumer-onboarding library.delegation-chain library.core-no-std library.fcis-boundary]
- [x] Verify `.gitignore` preserves the `vendor/verified-logic/src/result.rs` unignore with `grep -qx '!vendor/verified-logic/src/result.rs' .gitignore`. [covers=library.fcis-boundary]
