## Why

Issue #1 asks Kiln to design opt-in CI orchestration that checks an already proposed merge M against an exact base O and variants A and B. Today Kiln can acquire one exact Pijul channel state through a read-only adapter and bind gate results to one candidate set, but nothing binds a semantic checker result to a frozen four-version quartet, its property and its policy. Without that binding, a clean text merge, equal trees, passing tests or a replayed result could be mistaken for a semantic guarantee.

## What Changes

- Add the state-transition contract `docs/semantic-merge-orchestration.md` with an integration plan that reuses the existing source acquisition, Lattice workflow dispatch, Bounded Exec, gate and Valence evidence ports.
- Add a pure `kiln_core::semantic_merge` module:
  - an immutable four-role evaluation request (O, A, B, M) with exact source and tree identities, native provider observations, semantic subject, property, profile, policy revision, checker and toolchain identities, and named resource bounds;
  - explicit base and variant selection, with Pijul channel-state provenance and change containment, refusing ambiguous or unavailable lineage;
  - candidate admission only through a declared supplier and a read-only acquisition profile;
  - advisory and required modes and the full outcome lattice: verified, counterexample, unknown, unsupported, exhausted, cancelled, stale, tool-error;
  - role-tagged BLAKE3 bindings for attempts, logs, products, retries and caches, with diagnostics that name the failed binding;
  - rejection of late, superseded, replayed and stale results;
  - independence from other gates and a downstream revalidation obligation that grants no mutation authority.
- Add a `SemanticCheckerPort` to `kiln-provider-api`. Only a test fixture producer implements it.
- Add a Nickel-authored frozen quartet fixture with a deterministic JSON export, and an end-to-end fixture test: Pijul acquisition, checker request, result evidence and CI decision. Add the negative fixtures that the issue lists.
- Add a test proving that the Pijul source profile still refuses every forbidden capability.

## Impact

- **Consumers:** Kiln operators who opt into a semantic merge gate, and future checker producers (Octet #1, Noble #30) and evidence contracts (Valence #1).
- **Owners:** Kiln maintainers own the core module, the port and the contract document.
- **Durable contribution:** a fixture-proven orchestration contract that a production producer can implement without changing Kiln's gate meaning.
- **Affected:**
  - `crates/kiln-core` (new module);
  - `crates/kiln-provider-api` (new port module);
  - `crates/kiln-adapter-pijul` tests;
  - `fixtures/semantic-merge`;
  - `docs/semantic-merge-orchestration.md`;
  - `flake.nix` (one fixture-export check);
  - `Cargo.lock`, through cargo only, for a test-only dependency.

## Out of Scope

- No production semantic checker, receipt profile or required production gate. Those wait for Octet #1, Noble #30 and Valence #1 and for independently reviewed checker support.
- No candidate generation, and no apply, record, reset, push, merge or other mutation on the Pijul adapter.
- No merge, publish or status authority. Downstream mutation keeps its own race-safe protocol.
- No inferred Git-style merge base, and no Git ancestry lineage policy.
- No implementation of verified three-way merge semantics in Kiln.
- No web console surface.
