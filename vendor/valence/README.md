# Valence

Valence is the canonical evidence identity and validation layer for the Onix trust stack.

It defines Evidence IR, a serializable and typed format. Evidence IR gives artifacts stable BLAKE3 identities and preserves source spans and provenance.

It also keeps each claim within an explicit boundary.

Valence lets tools refer to the same source, receipt, sidecar, or binary. It rejects changes to identity, role, linkage, or claim boundaries.

Valence is an intermediate representation. It is not a programming language, compiler, code generator, or proof of language soundness.

> [!IMPORTANT]
> A passing result proves only the facts that the selected Valence profile checks. It does not prove evidence truth, subject correctness, safety, or release readiness.

## Why Valence exists

Evidence moves through several tools before a lifecycle or release decision:

```text
source → evidence artifact → receipt or sidecar → release bundle → lifecycle gate
          └──────────── stable identities and typed links ────────────┘
```

Without a shared identity layer, each handoff can reinterpret hashes, roles, spans, or claims.

Valence provides canonical forms and fail-closed checks. These checks keep each handoff explicit and reviewable.

| Project | Relationship to Valence |
|---|---|
| [Octet](https://github.com/OnixResearch/octet) | Produces source-derived evidence and owns source scans, proof rails, and release decisions for its surfaces |
| Valence | Owns Evidence IR identity, linkage, roles, provenance, and non-claim checks |
| [Trellis](https://github.com/OnixResearch/trellis) | Supplies verified-logic components and proof-artifact identity references |
| [Mantle](https://github.com/OnixResearch/mantle) | Records bounded Valence evidence and requirement identities in release bundles |
| [Cairn](https://github.com/OnixResearch/cairn) | Uses saved Valence reports as bounded lifecycle-policy input |

## Quick start

Show the CLI:

```bash
nix run .#valence -- --help
```

Run the workspace tests:

```bash
nix develop -c cargo test --workspace
```

The CLI uses explicit capability roots. A file locator must be relative to the root for its role.

```bash
nix run .#valence -- check \
  --artifact-root /srv/evidence \
  artifact.json
```

The CLI rejects these inputs before it opens a file:

- absolute locators
- parent traversal
- missing roots
- root and role mismatches

Read [CLI capability roots](docs/cli-capability-roots.md) for the full contract.

## Core model

An Evidence IR v1 `EvidenceArtifact` contains these fields:

| Field | Purpose |
|---|---|
| `schema` and `artifact_kind` | Select the envelope version and payload meaning |
| `stable_id` and `canonical_hash` | Bind typed content to a stable BLAKE3 identity |
| `source_span` | Preserve source identity, byte range, and coordinates |
| `verification_role` | Separate verified properties from recorded or boundary evidence |
| `payload` | Carry kind-specific data inside the hash boundary |
| `non_claims` | State what the artifact does not establish |

Verification roles are separate by design:

| Role | Meaning |
|---|---|
| `Property` | Can count as verified under the selected Valence and Cairn profile |
| `RecordedOnly` | Records identity or linkage and does not count as verification |
| `Boundary` | Records a claim or authority boundary and does not count as verification |

The control-plane envelope is closed. Unknown or duplicate control fields fail before hash and claim-policy checks.

The kind-specific payload remains open, but the canonical hash covers it fully.

Read the [artifact envelope](docs/evidence-artifact-envelope.md) and [span integrity](docs/evidence-span-integrity.md) specifications.

## Meaning of a passing result

| Valence can establish | Valence does not establish |
|---|---|
| Canonical identity and accepted BLAKE3 spelling | Evidence truth or requirement satisfaction |
| Schema, envelope, and metadata consistency | Behavioral correctness or semantic equivalence |
| Typed links, digest roles, spans, and provenance continuity | Verifier soundness, runtime authority, or release eligibility |
| Role separation and bounded role summaries | Exhaustive Rust coverage or general HIR, THIR, or MIR extraction |
| Required non-claims and rejection of selected overclaims | Whole-program safety, pure-cache eligibility, cross-machine determinism, or external compatibility |

Producers still own source analysis and artifact creation. Shells own file access, processes, byte measurement, publication, and operator policy.

Downstream consumers must apply their own freshness, authorization, lifecycle, and release rules.

## Components

| Component | Purpose |
|---|---|
| [`valence-core`](crates/valence-core/README.md) | MPL-2.0 functional core for deterministic validation over already-loaded values. Its README lists every public module. |
| `valence` | AGPL CLI shell for capability-confined reads, writes, measurements, validation, and reports. |
| [`valence-radicle`](crates/valence-radicle/README.md) | AGPL preview-first adapter for recorded-only evidence links plus canonical forge plans, bindings, and CI-admission receipts. |
| [`valence-radicle-native`](crates/valence-radicle-native/README.md) | Process-isolated AGPL shell for bounded built-in Radicle issue/patch metadata writes without contaminating Valence canonical JSON. |
| `contracts/` and `cairn-policy/` | Nickel-authored schemas and lifecycle compatibility inputs with positive and negative fixtures. |
| `docs/`, `fixtures/`, and `evidence/` | Profile specifications, frozen producer inputs, migration guidance, and retained evidence. |

The core covers these areas:

- Evidence IR envelopes, references, canonical identity, and role registries
- stack provenance, Preserves bridges, evidence chains, and source spans
- function addresses, Wasm components, ast-grep, Kani, Trellis, and proof profiles
- evidence indexes, capability plans, deterministic harnesses, and Hegel properties
- Radicle COB reduction and artifact-auth mapping over checked observations

## Use `valence-core`

The packages use reviewed Git revisions and are not on crates.io. Pin an exact public revision:

```toml
[dependencies.valence-core]
git = "https://github.com/OnixResearch/valence"
rev = "<reviewed-commit>"
package = "valence-core"
```

Rust imports this package as `valence_core`.

The core does not access files, environment state, processes, clocks, networks, or terminals.

A passing core result grants only the documented claim. It does not transfer producer, verifier, runtime, lifecycle, or release authority.

## CLI

```text
valence check --artifact-root <ROOT> <FILE>
valence explain --manifest-root <ROOT> --id <ID> --manifest <FILE>
valence stack-provenance build --artifact-root <ROOT> --octet-export <FILE> --binary-hash <B3> --output-root <ROOT> --output <FILE>
valence stack-provenance verify --manifest-root <ROOT> --manifest <FILE> --sidecar-root <ROOT> --sidecar <FILE>
valence stack-provenance verify-built-binary --binary-root <ROOT> --binary <FILE> --artifact-root <ROOT> --octet-export <FILE> --sidecar-root <ROOT> --sidecar <FILE> --manifest-root <ROOT> --manifest <FILE> [--json]
valence stack-provenance graph-report --artifact-root <ROOT> --chain <FILE> [--format json|markdown]
valence function-address validate --artifact-root <ROOT> --input <FILE> [--output-root <ROOT> [--bundle <FILE>] [--receipt <FILE>] [--graph <FILE>]]
valence function-address preserves --artifact-root <ROOT> --input <FILE>
valence formal-proof aeneas-chain validate --artifact-root <ROOT> --input <FILE>
valence evidence-chain validate --artifact-root <ROOT> --input <FILE>
valence requirement-reference validate --artifact-root <ROOT> --input <FILE> [--registry <FILE>] [--output-root <ROOT> --out <FILE>]
valence stack-context validate --artifact-root <ROOT> --input <FILE> [--output-root <ROOT> --out <FILE>]
valence wasm-component validate --artifact-root <ROOT> --input <FILE>
valence version
```

Exit status `0` means success. Status `1` means invalid evidence or a missing span. Status `2` means a usage or I/O failure.

Run `nix run .#valence -- --help` for the authoritative command summary.

### Radicle sidecars

`valence-radicle` records optional collaboration state. It does not change Evidence IR or Cairn source files.

```text
valence-radicle authorization ...
valence-radicle mutate ...                  # preview without writes
valence-radicle mutate ... --execute        # sign and update one local COB ref
valence-radicle snapshot ...
valence-radicle verify-binding ...
valence-radicle verify-snapshot ...
valence-radicle forge-plan ...               # deterministic, no writes
valence-radicle-native import ...            # preview unless --execute
valence-radicle forge-bind ...
valence-radicle verify-forge-binding ...
valence-radicle ci-admission ...             # external-guard input only
valence-radicle verify-ci-admission ...
```

The adapter does not perform these functions:

- run a Radicle node or fetch peers
- manage private keys
- change Cairn lifecycle files
- promote recorded evidence into proof

Read [Radicle COB evidence sidecars](docs/radicle-cob-evidence.md).

## Documentation by task

- **Identity and envelopes:** [artifact envelope](docs/evidence-artifact-envelope.md), [digest roles](docs/nominal-digest-roles.md), [span integrity](docs/evidence-span-integrity.md), [requirements](docs/content-bound-requirements.md), and [Preserves migration](docs/preserves-integrity-migration.md).
- **Evidence profiles:** [generic chains](docs/evidence-chain-profile.md), [function addresses](docs/function-address-evidence-profile.md), [build service](docs/build-service-evidence.md), [Preserves ingest](docs/function-address-preserves-ingest.md), [Wasm components](docs/wasm-component-evidence.md), [ast-grep spans](docs/ast-grep-evidence-spans.md), [Kani boundaries](docs/kani-evidence-boundaries.md), and [FSM receipts](docs/fsm-default-exhaustive.md).
- **Proof-oriented adapters:** [Aeneas/Lean chains](docs/aeneas-translation-chain.md), [Trellis evidence](docs/trellis-proof-evidence.md), and [proof-carrying IR lineage](docs/proof-carrying-ir-lineage.md).
- **Operations and policy:** [CLI roots](docs/cli-capability-roots.md), [function-address publication](docs/function-address-publication.md), [Radicle sidecars](docs/radicle-cob-evidence.md), [Radicle forge operations](docs/radicle-forge-operations.md), and [Cairn compatibility](docs/cairn-policy-compatibility.md).
- **Testing and pilots:** [testing harness](docs/testing-harness.md), [strict Rust admission](docs/octet-deterministic-core.md), and the rejected [build-graph pilot](docs/build-graph-pilot.md).

## Development and checks

The repository pins Rust nightly in `rust-toolchain.toml`.

```bash
nix develop
cargo fmt --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings

# Full Rust, Nickel, fixture, and Nix validation.
nix flake check -L
```

The [test harness guide](docs/testing-harness.md) documents fast, deep, Hegel, and nightly lanes.

Nickel contracts in `contracts/` provide static preflight checks. Rust owns runtime parsing, capability-confined I/O, and measured-byte identity.

### artifact-auth source transport

Cargo and Nix resolve `artifact-auth-core` and `artifact-auth-ed25519` from [`OnixResearch/onix-artifact`](https://github.com/OnixResearch/onix-artifact).

Both tools pin revision `e41340bec587b6d049b5cc518ec7db925dde84be`.

The [migration receipt](evidence/source/artifact-workspace-migration-v1.ncl) binds source, package graph, byte parity, rollback, and non-claims.

The prior [Radicle receipt](evidence/radicle/artifact-auth-cutover-v1.ncl) is historical evidence only. Valence does not use a fallback source.

## Project status

Valence is under active development. Documented schema and identity versions are compatibility surfaces. Other APIs can change.

`valence-core`, `valence-cli`, and `valence-radicle` set `publish = false`. They use reviewed Git revisions.

A registry release or broader compatibility promise requires a separate review.

## License

| Surface | License |
|---|---|
| `crates/valence-core/**` | MPL-2.0 |
| CLI, Radicle adapter, executable pilots, and repository documentation | AGPL-3.0-or-later |
| Dependencies and upstream-derived material | Their upstream terms |

Complete texts are in [`LICENSES/`](LICENSES/). Read [the licensing guide](docs/licensing.md) for the package map and boundaries.

License metadata is outside canonical Evidence IR identity unless a versioned schema includes it.

## References

### Onix stack

- [OnixResearch/octet](https://github.com/OnixResearch/octet) — source for extracted core modules and selected evidence surfaces.
- [OnixResearch/cairn](https://github.com/OnixResearch/cairn) — lifecycle engine that uses Valence evidence as bounded input.
- [OnixResearch/trellis](https://github.com/OnixResearch/trellis) — verified logic and proof-artifact identity references.
- [OnixResearch/mantle](https://github.com/OnixResearch/mantle) — build system that records Valence identities.

### Identity and data models

- [Unison: The Big Idea](https://www.unison-lang.org/docs/the-big-idea/) — content identity, metadata names, typed values, effects, and structured refactoring.
- [oxidecomputer/iddqd](https://github.com/oxidecomputer/iddqd) — derived-key maps and duplicate-key rejection.
- [oxidecomputer/newtype-uuid](https://github.com/oxidecomputer/newtype-uuid) — typed UUID wrappers. Valence identity remains BLAKE3.
- [oxidecomputer/daft](https://github.com/oxidecomputer/daft) — semantic differences as review evidence, not equivalence proof.
- [Nostr protocol and NIPs](https://github.com/nostr-protocol/nips) — external signed-event identity reference. Valence preserves Nostr protocol digests without promoting authentication into proof or authority.

### Attestation and proof

- [oxidecomputer/rats-corim](https://github.com/oxidecomputer/rats-corim), [oxidecomputer/vm-attest](https://github.com/oxidecomputer/vm-attest), [oxidecomputer/dice-util](https://github.com/oxidecomputer/dice-util), and [oxidecomputer/pki-playground](https://github.com/oxidecomputer/pki-playground) — attestation and certificate-chain references.
- [joweeba/tMIR](https://github.com/joweeba/tMIR) — proof-lineage and sidecar reference.
- [AeneasVerif/aeneas](https://github.com/AeneasVerif/aeneas) — Rust translation-chain reference.
- [AeneasVerif/eurydice](https://github.com/AeneasVerif/eurydice) — deferred Rust-to-C lineage reference.
- [AeneasVerif/scylla](https://github.com/AeneasVerif/scylla) — deferred C-to-Rust lineage reference.

These references do not transfer attestation, compiler, proof, or equivalence claims into Valence.

### Evaluated but not adopted

- [d-tietjen/build-graph](https://github.com/d-tietjen/build-graph) — rejected navigation pilot at revision `68a12dcac9b2475e27b6062f974b5f37321d3c4f`. Read the [pilot report](docs/build-graph-pilot.md).
