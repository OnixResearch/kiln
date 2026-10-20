## Context

The shipped GitHub shell resolves pinned SecretSpec references, authenticates original webhook bodies, reserves SQLite run/status state, dispatches through `LatticeAdapter<UnixWorkflowExchange>`, admits canonical outcomes, and publishes commit statuses. Iroh already provides a private local Unix proxy and authenticated worker bridge to that same exchange. The exact-source job helper fetches the admitted GitHub revision before invoking reviewed argv with a cleared environment. The missing deliverable is an operated and repeatable composition with real external observations, not new core CI semantics.

## Decisions

### Decision: Compose existing production interfaces

Use the ordinary GitHub and Iroh binaries and an exact reviewed Lattice runtime with shell-exit semantics. Bind service, native graph, saved workflow, shell operation, original job-profile bytes, source revision, and enrolled endpoint identities. Keep credentials and private keys outside the checkout and Nix store. Use typed Nickel for authored deployment inputs and export runtime JSON through existing contracts.

### Decision: Isolate the validation deployment

Use new private state roots and service identities, a dedicated validation ref, and a new webhook/HTTPS route. Preserve unrelated working-copy changes and existing CI/ingress/services. Use the existing authorized operator tool for bootstrap and a real SecretSpec provider for runtime resolution; do not install a fake resolver or put credentials in argv, logs, workflow environment, source, or evidence. Keep worker commands separated from controller status authority. Record actual credential scope limitations rather than claiming least privilege without evidence.

### Decision: Exercise real source and outcomes

The reviewed worker command must compile and execute a small repository-owned Rust behavior probe from the exact acquired revision. Positive input must pass its behavioral assertions; negative input must fail an assertion with a nonzero test exit. Retain the probe and exact source commits so the result can be reproduced. Do not use `/bin/true`, `/bin/false`, a marker-only job, fabricated payloads/signatures, local Git/API fixtures, or a Pending demonstration as completion evidence.

### Decision: Define the interruption boundary precisely

Interrupt the controller and Iroh bridges after durable remote execution acceptance while leaving the Lattice executor and its job running. Restart with the same protected keys, profile identities, SQLite database, native workflow routes, and Lattice store. Recover the same execution through the native recovery operation and require one admitted terminal outcome plus its real remote status. A killed Lattice executor is outside this recovery claim; its orphaned nonterminal work remains Unknown rather than being redispatched blindly.

## Risks / Trade-offs

- Live GitHub writes and ingress are operator effects, bounded to dedicated validation resources. A broad existing bootstrap credential is not evidence of a suitably scoped production token; any temporary exception must be explicit, private, and retired after the bounded exercise.
- A bridge disconnect proves ambiguity, not nonacceptance. Never retry an ordinary dispatch to obtain a convenient result.
- Failed or Unknown status publication cannot be declared recovered from the local outcome alone. Completion requires the corresponding exact Published intent and a matching real GitHub status.
- Workload isolation remains the operator's responsibility. This rail executes only its reviewed validation source; it does not admit arbitrary fork code or establish a security sandbox.
- Endpoint reachability proves only the selected two-host network path. Do not promote it into public-WAN, relay, long-term deployment, or release claims.
