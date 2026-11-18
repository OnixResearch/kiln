## Context

`kiln_core::projection::resolve` produces an `AdmittedPartialView`, and `require_complete` produces a `CompleteProjection` for a build or write operation; neither grants authority. Manifest entries carry destinations, kinds and native or generated origins; native origins name a provider object such as a Git blob. `worker::source::materialize_exact` already writes a full Git tree with exact modes from a bare backing repository.

## Goals and Success Contract

The same manifest and profile always produce the same input identity and a byte- and mode-equivalent tree. Two attempts share one immutable input but write only to their own overlays. Missing, corrupt or truncated content, unsafe or unsupported entries and pre-existing unbound directories never yield an admitted input or overlay, and a failure midway leaves nothing that a retry would adopt.

## Decisions

### Decision: a pure plan bound to the manifest, profile and attempt

**Choice:** `plan` accepts only a `CompleteProjection` and a `kiln.materializer-profile.v1` profile naming the materializer kind, its exact version and the platform it writes to. The input identity is BLAKE3 over the manifest identity, its content identity and the profile; the overlay identity is BLAKE3 over the input identity, the attempt identity and a non-zero fence.

**Rationale:** separating reusable input identity from attempt-specific overlay identity is the issue's isolation requirement; binding both to exact identities makes a mismatch detectable.

### Decision: refuse what this profile cannot represent

**Choice:** eager-local v1 refuses symbolic links, generated entries, executable entries on a platform without executable bits, a platform whose case sensitivity differs from the manifest target and destinations longer than the platform bound.

**Rationale:** explicit refusal is required until links and generators are qualified; silently degrading would change the materialized tree.

### Decision: verify, prepare privately, publish by rename, bind separately

**Choice:** the shell checks that the store is a private directory, reads each blob from the exact repository, recomputes its Git object identity, writes entries with create-new into a fresh preparing directory, makes the input read-only, renames it into place and only then writes the binding. Reuse requires a matching binding and re-verifies every entry. An overlay is a full writable copy of a verified input with its own binding.

**Rationale:** a copy, never a hard link, keeps overlay writes away from shared inputs; a binding written after the rename means an interrupted preparation is never adopted.

### Decision: one workspace pin shared by dispatch and the retained overlay

**Choice:**

- `WorkspacePin` (`kiln.materialization.workspace-pin.v1`) holds the admitted repository identity, the exact source revision, the definition and manifest identities, the materializer profile identity, the input and overlay identities, the attempt and the fence. `MaterializationPlan::pin` derives it from the manifest and profile the plan was built from.
- `DispatchWorkspace.projection` optionally carries the pin. Admission requires that the pin's attempt, fence and source equal the attempt and the plan's frozen base; that its overlay is the identity of its input, attempt and fence; and that the workspace identity is that overlay. The pin is appended to the plan identity only when present, so existing plans keep their identity.
- The worker overlay binding moves to `kiln.materialization.binding.v2` and records the pin. `prepare_overlay` takes the dispatch pin, refuses one that differs from `plan.pin`, and resumes only a directory whose binding records exactly that pin. A v1 overlay binding is refused rather than adopted.

**Rationale:** dispatch and the worker then agree on one exact source, definition, manifest, profile and overlay generation. A changed input, a new attempt or fence, or a substituted manifest yields a different pin and a fresh overlay, never a silently reused one.

## Risks / Trade-offs

- Re-verifying a reused input costs a full read; it is what keeps a damaged cache from being admitted.
- Inputs are not fsynced; a crash can leave a published input whose bytes are damaged, which re-verification then refuses.

## Verification Strategy

Pure tests cover plan identities, each refusal, pin derivation and pin consistency. Dispatch tests cover an admitted pinned plan, each pin mismatch, and an identity that changes with the pin and is unchanged without one. Shell tests over a temporary Git repository cover repeated materialization, overlay isolation, missing and corrupt objects, damaged reuse, pre-existing unbound directories, overlay binding mismatch, a dispatch pin that differs from the plan and interrupted preparation. Then Clippy, formatting and Cairn gates.
