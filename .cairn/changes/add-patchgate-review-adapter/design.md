# Design: isolated Patchgate review evidence

## Context

Kiln's existing Pijul source profile is the beta.11 library cohort. Patchgate uses a separately patched `pijul-core` producer and already exposes selected isolated review with schema-versioned JSON. The existing Kiln lifecycle GatePort accepts proposal/design/tasks targets and is not a code-review contract.

## Goal and Success Contract

An operator can run an exact selected Pijul review through a bounded Kiln adapter, retain the complete native report, and consume a typed review observation bound to one exact request. The source channels, work files and audit state remain unchanged. Normal Kiln builds and tests do not acquire Patchgate or call a model. Real producer compatibility is exercised separately with an explicitly supplied installed executable.

False completion includes accepting a process exit as a verdict, inventing unavailable evidence, trusting a different initial main state, or treating a simulated acceptance as a source mutation capability.

## Decisions

### Decision: Separate review evidence from lifecycle gates

Add pure provider-neutral review plans/observations and a narrow review port under the collaboration role. Keep lifecycle targets, CI state transitions and merge authorization unchanged. The adapter exposes only isolated observation; no generic command, accept, reject, revert, signer or watch operation is available.

### Decision: Bind exact request and native evidence

A review plan binds the candidate set and canonical root, pipeline revision, policy identity, effect, idempotency key and provider profile. The native request binds the repository, exact initial main/inbox channel states, trusted configuration and explicit full change hashes. Every authorized prerequisite must be explicit in the bound candidate; automatic dependency expansion is disabled. Unrelated and duplicate changes fail admission.

The consumer verifies schema version, `dry_run: true`, `authorizes_acceptance: false`, exact initial channel/configuration bindings and result completeness. Native records retain their own reviewed main states: simulated earlier acceptances can legitimately change later states. A single initial-state claim must not replace those per-record facts. All native bytes remain available under a BLAKE3 content identity; native SHA-256 and NAR identities remain unchanged.

Fresh accepted, rejected and escalated decisions require matching newly reported records and decided items. Historical terminal state alone cannot turn an earlier operational stop into a completed new review. The pinned producer can omit request digests on short model reviews; retain the actual empty provenance vector without inventing evidence or claiming unavailable prompt/schema digests.

### Decision: Preserve decisions and operational uncertainty

Accepted, rejected, escalated, blocked and waiting remain distinct review results, not CI execution success. Process failure, malformed output, output overflow, timeout, cancellation and unknown execution cannot become approval. Partial or interrupted reports cannot claim complete review. No automatic retry or cached observation is substituted for a fresh request. Reconciliation is admission of previously retained exact evidence, not redispatch.

### Decision: Keep an explicit bounded shell

The profile pins the supported source revision/report contract, an operator-supplied absolute executable path and BLAKE3 identity, repository root, explicit environment and named request/result/executable/process limits. The shell clears inherited environment, uses fixed argv with explicit selected hashes, null stdin and process-group teardown, and supports cancellation. Executable provenance and transitive runtime closure remain operator responsibilities; a declared revision is not proof of backend weights or source origin.

### Decision: Keep private acquisition optional

Patchgate source `https://github.com/brittonr/patchgate` revision `97c1216fe13000d9dcbb97f70ddd5423180eea2c` is an external runtime dependency, not a workspace-relative or default Cargo dependency. An installed operator-supplied package contains its own patched Pijul. The old Pijul source adapter is not used to open this producer's database. Typed Nickel builds runtime JSON without providing fabricated production identities.

## Risks / Trade-offs

- Dry-run can call a model and create private temporary keys or Nix store results. It is source-nonmutating, not globally effect-free or a sandbox.
- Patchgate's model revision is caller-declared and `revision_verified` is false. Review evidence does not strengthen that claim.
- A dry-run report does not expose a generally reusable final source snapshot and never authorizes acceptance. CI-gated landing requires a separate reviewed contract and is excluded.
- Source JSON can contain sensitive messages/paths. Retained evidence is operator-controlled; CLI output is evidence, not public telemetry.

## Validation

Keep positive/negative pure and adapter tests together, including exact applicability, candidate/configuration/state substitution, missing or duplicate decisions, malformed JSON, unsupported versions/capabilities, process failures, deadline/cancellation and retained-evidence replay. Exercise the actual operator CLI over real Pijul changes for deterministic documentation acceptance, secret rejection and protected-path escalation while independently comparing source channels/audit files before and after. Run the repository's focused and broad gates; retain observed results and explicit environmental limits.

## Claim Boundary

Kiln maintainers own adapter repeatability. Patchgate owns review and native merge policy. Operators own deployment, credentials, model endpoints, Nix policy and executable provenance. Passing checks prove only the declared local request/result relation and tested producer cohort, not source correctness, unattended model safety, acceptance authority or release eligibility.
