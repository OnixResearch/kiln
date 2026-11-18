## Context

The current verified-publication path is built from these pieces:

- `ImplementationAttempt` retains exactly one `CandidateHandoff`, one `CandidateVerification` and one `PublicationRecord`.
- `admit_publication_plan` binds them to one branch (`branch_for(work, attempt)`) and one pull request.
- `GithubPublicationClient::publish_candidate_branch` reads the candidate from the attempt/fence export `<attempt>-<fence>/candidate`, checks its tree, pushes the exact ref without overwriting, and reconciles. `ensure_pull_request` then reconciles or creates exactly one pull request for that head and base.
- `GithubLandingClient` is read-only. `land` refuses (`github_landing_unqualified_atomicity`). `recover_landed` accepts only an exact two-parent merge: base head first, candidate second, landed tree equal to the candidate tree.

The GitHub documentation read for this change (stacked pull requests, the stacks REST API and merging stacked pull requests, as of November 2026) says the following:

- A stack is a chain of pull requests in one repository. The bottom one targets the trunk, and each one above targets the branch of the one below.
- `POST /repos/{owner}/{repo}/stacks` creates a stack from two to 100 pull request numbers, bottom to top. Each base must equal the previous head. `GET /stacks?pull_request=N` finds the stack containing a pull request. `/add` appends to a stack and `/unstack` dissolves one. A repository without stacks returns `404`, and `409` signals concurrent modification.
- Stacks merge from the bottom up. Merging a pull request lands every unmerged pull request below it as one operation, ordered bottom-up in history; with the merge-commit method GitHub creates one merge commit per pull request. The pull requests left above are retargeted to the trunk and rebased automatically, which replaces their head commits. **Rebase stack** also replaces commits, and keeps approvals when the diff is unchanged.
- A stack enters the merge queue as one group, and ejecting one pull request removes every pull request above it. API merges of stacks must use the asynchronous merge API.

## Goals and Success Contract

An operator can publish one verified attempt as two or more exact layers, each with its own branch, pull request and verification, linked into one GitHub stack. A re-run never duplicates anything. A gate tells exactly which layers may land and why the others may not. No layer or stack is recorded as verified-integrated unless the landed history is exactly the verified layers.

## Decisions

### Decision: the top layer is the attempt's verified candidate; lower layers carry layer verifications

**Choice:** a stack decomposes the attempt's one verified candidate along its ancestry, so the attempt model keeps its single handoff, verification and publication record. The top layer reuses the attempt's handoff and `CandidateVerification`. Each lower layer has a `LayerVerification` that binds:

- the attempt's handoff identity, work/version and attempt/fence;
- the layer position;
- the exact candidate, tree and parent;
- the work's verification profile;
- a canonical `CiFact` for exactly that candidate.

The CI acceptance rules are extracted from `admit_candidate_verification` into one shared function, so both receipts apply identical rules.

**Rationale:** every layer is verified at its exact revision without inventing a second attempt or changing #46's receipt.

### Decision: route resolution before any effect

**Choice:** `resolve_route(requested, host_supports_stacks)` resolves the route before anything else happens:

- `PullRequest` is the default and resolves to `PullRequest`.
- `GithubStack` resolves to `GithubStack` only on a host with stacked pull requests, and to `PullRequest` otherwise.
- `OriginStack` is always refused with `implementation_stack_destination_unsupported`.

The adapter's probe (`GET /stacks?per_page=1`, where `404` means unsupported) supplies `host_supports_stacks`.

### Decision: exact plan chain

**Choice:** `admit_stack_plan` admits a plan only when all of the following hold:

- the plan has two to sixteen layers at contiguous positions;
- the bottom layer's parent is the frozen base, and every other layer's parent is the candidate of the layer below;
- candidates are distinct;
- the bottom layer's base branch is the trunk, and every other layer's base branch is the branch below;
- every branch equals `layer_branch(work, attempt, position)`;
- the top candidate and tree are the handoff's, and the top verification is the attempt's;
- every lower layer's verification identity is an admitted `LayerVerification` for that position.

The plan identity is BLAKE3 over every binding. Git ancestry cannot be checked in the pure core, so the adapter checks tree and `descendant_of(parent)` for every layer before its first push.

### Decision: bottom-up gate with no skipped layer

**Choice:** `stack_gate(states)` maps each layer's state (`Ready`, `Integrated`, `Refused(code)` or `Unknown(code)`) to a gate, walking from the bottom. A layer may land only if it and every layer below it are `Ready` or `Integrated`. Otherwise the lowest non-passing layer below decides: a refusal blocks with that layer and code, and an unknown state stays unknown with that layer and code. An `Integrated` layer above a non-integrated one is reported as unknown (`implementation_stack_out_of_order`), because GitHub merges bottom-up. The adapter derives each layer's state:

- recovered exact merge → `Integrated`;
- #48 `observe` success → `Ready`;
- a domain refusal → `Refused`;
- any other failure → `Unknown`.

### Decision: integration only for an exact prefix

**Choice:** `admit_stack_integration` accepts layers 1..=m (m ≥ 1) of an admitted plan with the retained pull requests. Each layer's merge commit must be commit-shaped, with:

- the planned candidate as second parent;
- the planned tree as the landed tree;
- as first parent, the frozen base (bottom layer) or the previous layer's merge commit.

It returns an identity and whether the stack is complete (m = n). `GithubLandingClient::recover_stack_landed` reads layers from the bottom and enforces the same mapping on the actual commits:

- a merged layer whose pull request head differs from the plan refuses with `github_stack_layer_head_changed`;
- a merged layer above an unmerged one is unknown;
- after the topmost recovered merge, the trunk head must descend from it.

**Rationale:** GitHub documents that the remaining layers are rebased after a partial merge and that **Rebase stack** rewrites commits. Neither produces the verified commits, so neither can yield a verified-integrated receipt. Whether a whole-stack merge preserves the original heads is a live-qualification question. Until then the strict mapping either holds or refuses.

### Decision: GitHub stack publication is reconcile-first

**Choice:** `GithubPublicationClient::publish_stack` runs in this order:

1. Probe support, and refuse before any effect if the repository has no stacks.
2. Check every layer's tree and ancestry.
3. Read every layer before the first mutation: look up its pull request by head branch alone, and read the branch of a layer that has none. A conflicting branch or pull request is refused here. This includes an upper pull request that GitHub retargeted to the trunk after a lower merge, which a head-and-base lookup would miss and then duplicate.
4. From the bottom up, push each missing branch at the exact candidate and create each missing pull request against the branch below. Each layer uses a per-layer `PublicationPlan` view with its candidate, tree, branch, base branch and text.
5. Read `GET /stacks?pull_request=<bottom>`. If no stack exists, create one with `POST /stacks`.

A stack whose numbers, order, head branches, head SHAs and trunk equal the plan is reused, and any other existing stack is refused (`github_stack_conflict`). A conflict, a lost response or an unreadable creation response is reconciled once and otherwise stays unknown. Kiln never adds to, removes from or dissolves a stack.

## Risks / Trade-offs

- The stacked merge mapping is unqualified on live GitHub. A whole-stack merge that rewrites heads is reported as refused or unknown, not integrated, until a live qualification shows otherwise.
- GitHub enforces its own bottom-up approval and check rules. Kiln does not rely on them; it applies its own per-layer gates and exact receipts.
- The route is not yet selectable from the operator CLI, and stack records are not yet persisted. The library and fixtures prove the behaviour; wiring remains on #89.

## Verification Strategy

Core unit tests cover:

- every route;
- layer verification acceptance and refusals;
- each plan binding, one at a time;
- the gate over refused, unknown, out-of-order and ready states;
- integration prefixes with each mapping refusal.

Adapter tests use the existing loopback fixture server and a local bare remote:

- a two-layer stack is published, then re-entered with no mutation;
- the unsupported probe refuses before any push;
- a conflicting stack is refused;
- a lost creation response is reconciled once;
- exact recovery of a two-layer merge;
- a rewritten upper head and an out-of-order merge are refused or unknown.

Clippy, scoped formatting, and the Cairn gates and validation also run.
