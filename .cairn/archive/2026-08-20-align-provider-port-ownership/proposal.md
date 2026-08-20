# Align provider port ownership

## Why

Kiln declares application-owned provider ports in `kiln-provider-api`. Several concrete adapter crates also publish separate provider contracts.

The affected contracts include `JujutsuRepository`, `CobMutationTransport`, `RemoteTransport`, `RemoteLifecycleTransport`, and `BrokerService`. Their adapter functions combine provider calls with admission, policy, and lifecycle decisions.

These contracts bypass the neutral provider API. They also return `CoreResult` for provider and transport failures, which merges infrastructure failure with domain rejection.

## What Changes

- Move genuine provider capability contracts into the application-owned provider API.
- Keep deterministic admission, ambiguity, transition, and effect-plan decisions in `kiln-core`.
- Make concrete adapter crates implement application-owned ports instead of defining competing public ports.
- Keep provider SDK values, transport state, credentials, and infrastructure errors inside adapters and shells.
- Select concrete adapters at a visible composition root.
- Add positive and negative conformance tests for each migrated capability.

## Non-Goals

- One universal provider interface for unrelated capabilities.
- Provider SDK types in `kiln-core` or `kiln-provider-api`.
- Changing Kiln identity, provider-native identity, canonical bytes, or accepted outcome meaning.
- Treating successful adapter calls as proof of provider correctness or effect completion.

## Impact

- **Affected specs:** `provider-ports`
- **Affected core:** provider-neutral admission and effect planning
- **Affected application contracts:** `kiln-provider-api`
- **Affected adapters:** Jujutsu, Radicle broker, COB mutation, and remote execution
- **Testing:** contract, shell-order, error-translation, ambiguity, denial, timeout, cancellation, and provider-failure cases
