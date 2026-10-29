# Add isolated Patchgate review evidence

## Why

Kiln can acquire Pijul snapshots but cannot consume Patchgate's native policy review, dependency decisions, conflict-resolution evidence, or isolated Nix verification. Patchgate remains the owner of those decisions and source application. The immediate consumer is a Kiln operator running review of an exact Pijul candidate through a reviewed workflow.

## What Changes

- Add a provider-neutral review-evidence contract and narrow review port, separate from lifecycle gates and source mutation.
- Add optional `kiln-adapter-patchgate` support for Patchgate source revision `97c1216fe13000d9dcbb97f70ddd5423180eea2c`, report schema 1, and isolated selected review only.
- Bind requests and results to an exact Kiln candidate, provider profile, effect, native main/inbox states, selected changes and trusted configuration.
- Execute the pinned operator-supplied program with explicit environment, process-group teardown, cancellation, deadlines and bounded output.
- Provide an operator CLI and typed Nickel configuration so a reviewed Lattice shell job can produce and retain exact review evidence.
- Preserve original native report bytes, SHA-256 policy/provenance facts and model-identity limitations under Kiln BLAKE3 identities.
- Add deterministic positive/negative conformance plus an opt-in real Patchgate/Pijul rail that verifies the source repository stays unchanged.

## Impact

- **Core**: pure review facts and applicability admission; no provider imports or new merge authority.
- **Providers**: a review-only collaboration observation port, not Cairn's proposal/design/tasks gate.
- **Shell**: a separate adapter and executable; normal builds do not fetch Patchgate or require access to its private source repository.
- **Ownership**: Kiln maintainers own the adapter and repeatability rail; Patchgate maintainers own the native protocol and review semantics; operators own executable provenance, credentials, model endpoints and isolation.
- **Testing**: core/adapter/conformance tests, actual installed CLI smoke, real Pijul isolated acceptance/rejection/escalation, format, Clippy, no_std, Nickel and native Cairn gates.

## Non-claims

Review evidence does not authorize acceptance, establish model-weight identity, prove source correctness, promise a final reusable merged snapshot, or establish deployment/release readiness. No acceptance, revert, signing, publication, or remote mutation is added.
