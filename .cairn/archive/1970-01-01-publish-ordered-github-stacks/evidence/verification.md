# Ordered stack publication verification

Issue: <https://github.com/OnixResearch/kiln/issues/89>.

## Delivery and ownership

The immediate result is an opt-in, off-by-default route that publishes one
verified implementation attempt as an ordered GitHub stack. Single pull-request
publication is unchanged and stays the default. The durable capability has two
parts:

- `kiln_core::implementation::stack`: route resolution, lower-layer
  verification on the CI rules shared with #46, stack plan admission, retained
  publication admission, the bottom-up layer gate and exact-prefix integration.
- `kiln-adapter-github`: `GithubPublicationClient::supports_stacks` and
  `publish_stack`, and the read-only `GithubLandingClient::recover_stack_landed`,
  `observe_stack` and `layer_state`.

Kiln implementation and GitHub adapter maintainers own it. Consumers such as
#50, #62, #83 and #84 can select the route once a change is already split into
ordered layers. No stored state, default or operator command changed.

## Exercised checks

Commands ran from this checkout through `nix develop path:$PWD -c`.

| Check | Observed result |
| --- | --- |
| `cairn gate proposal\|design\|tasks publish-ordered-github-stacks ...` | PASS before implementation, and PASS again after the tasks were ticked |
| `cargo test --locked -p onix-kiln-core --lib` | 363 passed, including the 6 stack tests and the shared CI-rule refactor of `admit_candidate_verification` |
| `cargo test --locked -p kiln-adapter-github --lib stack` | 6 passed |
| `cargo test --locked -p kiln-adapter-github --lib` | 103 passed, including the existing single pull-request publication and landing tests |
| `cargo clippy --locked -p onix-kiln-core -p kiln-adapter-github --all-targets -- -D warnings` | PASS |
| Scoped `rustfmt --check --edition 2024` | PASS for the 5 touched Rust files and their 3 new test modules |
| `cairn sync publish-ordered-github-stacks ... --execute` | Added `.cairn/specs/implementation-stack/spec.md`; nothing was archived |
| `cairn validate --root . --policy cairn-policy/generated/cairn-policy.json` | Valid, 0 issues |
| `cairn traceability coverage ... --json` | 1,896 requirements, 1,894 referenced. It still exits 1 with the same two missing GitHub anchors and 71 unrelated dangling IDs. All 9 new stack IDs are referenced |

The tests pair positive and negative cases:

- **Route:** single pull request by default and on hosts without stacks; a
  GitHub stack only where supported; Origin refused on every host.
- **Layer verification:** an exact lower layer is admitted. A wrong schema,
  position, candidate, parent, profile or fence is refused, as are a failed run,
  a run for another revision and a failing check; another run ID changes the
  identity.
- **Plan:** 16 single-binding mutations are refused, among them swapped or
  skipped positions, broken parent and branch chains, an unverified top, a
  missing or mismatched lower verification, one layer, an attempt that is not
  verified and an unsupported terminal policy. Seventeen layers are refused.
- **Gate:** a refused or unknown lower layer holds every layer above it; an
  integrated layer above one that has not landed is unknown. Malformed state
  lists are refused.
- **Retained publication and integration:** one pull request per layer in the
  plan's repository, with six publication mutations refused. Only an exact
  bottom prefix is admitted; eight other prefixes and a foreign pull request
  are refused.
- **Publication fixture:** against a local bare remote and a loopback GitHub
  server, a two-layer stack pushes both exact branches, creates the upper pull
  request against the lower branch and links both with one `POST /stacks`. A
  re-run issues only `GET` requests and returns the same publication.
- **Publication refusals:** a `404` probe refuses after one request. A
  candidate that skips its parent, a layer with another tree, a foreign commit
  on the upper branch and an upper pull request retargeted to the trunk (found
  by its head branch alone) are refused before any push or `POST`. A stack
  holding another member is refused with reads only. A lost or conflicting
  creation response is reconciled with exactly one read and is otherwise
  `Unknown`.
- **Merge recovery:** both layers, or only the bottom layer under an advanced
  trunk, are recovered with reads only. A rebased upper head, an upper merge
  onto the frozen base, an out-of-order merge and a diverged trunk are refused
  or `Unknown`.

## Boundaries

- No GitHub stack was published, merged or recovered live. GitHub's stack
  documentation shows `X-GitHub-Api-Version: 2026-03-10`, while the client pins
  `2022-11-28`. Whether a whole-stack merge keeps the original heads and the
  two-parent mapping is a live-qualification question; until then, such a merge
  is refused or `Unknown`.
- The route is not selectable from the operator CLI, and stack records are not
  persisted on the attempt.
- Stack recovery does not reread merged layers' checks or approvals.
- Kiln never merges, rebases, retargets, adds to, removes from or dissolves a
  stack.
- The full workspace tests, Octet, release-readiness and flake checks were not
  run.
