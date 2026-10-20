# Verification: Local CI Composition E2E

## Source

Working copy change `klqwnxkszuyl`, parent `f748acd`. The working-copy commit
identity is not fixed while the change is uncommitted. The final local-suite Nix
check built source snapshot `/nix/store/3hpq9s1qbxk17gfx8rhkkmrmb7136fkc-source`
and produced `/nix/store/a64n6hw9lk2307cvk9q2f4s330wzfhs8-kiln-github-protocol-fixtures-0.1.0`.
The recorded runs used the workspace sources in this change, including
`crates/kiln-adapter-github/tests/local_ci_e2e.rs`.

## Environment

The Nix development shell did not realize locally within the 300-second
command deadline on this workstation. The recorded Rust runs therefore used
the available Rust 1.88.0 store toolchain directly, with explicit CMake,
pkg-config, OpenSSL, Zlib, and Rust sysroot inputs. The Nix package check
itself completed successfully through the configured remote builder
`ssh-ng://root@aspen1.local`. The repository's interactive
`nix develop path:$PWD` shell was not reproduced locally in this session. The
broad `nix flake check` was not run; the changed-surface
`github-protocol-fixtures` check and the workspace Rust gates were exercised.

## Commands and results

| Command | Result |
|---|---|
| `cargo test --locked --offline -p kiln-adapter-github --features protocol-fixtures --test local_ci_e2e -- --nocapture --test-threads=1` | PASS: 17 tests |
| `cargo test --locked --offline -p kiln-adapter-github --features protocol-fixtures --tests` | PASS: 65 tests across 5 suites |
| `cargo clippy --locked --offline -p kiln-adapter-github --features protocol-fixtures --all-targets -- -D warnings` | PASS |
| `cargo clippy --locked --offline --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --locked --offline --workspace` | PASS: 419 tests across 74 suites, 2 ignored |
| `cairn validate --root . --policy cairn-policy/generated/cairn-policy.json` | PASS: no change issues, no issues |
| `cairn traceability coverage --root . --policy cairn-policy/generated/cairn-policy.json --json` | PASS: 646 of 646 requirements referenced, no dangling or missing markers |
| `cairn policy export --source cairn-policy/default.ncl --output cairn-policy/generated/cairn-policy.json --check` | PASS: policy fresh |
| `nix build path:$PWD#checks.x86_64-linux.github-protocol-fixtures --no-link -L --offline` | PASS: 43 library, 5 job-helper, and 17 local-composition tests; 0 doctests |

## DNS teardown stabilization

The pre-existing
`status::dns_tests::stalled_resolver_subprocess_deadline_kills_and_reaps_its_process_group`
test now waits up to two seconds for the leader and descendant to disappear
instead of sampling once. It still asserts that both processes stop and that
the direct child is reaped. Twenty consecutive local runs passed.

## Retained failure

An earlier Nix attempt for the expanded suite failed in the pre-existing
`status::dns_tests::stalled_resolver_subprocess_deadline_kills_and_reaps_its_process_group`
assertion (`!process_is_running(&descendant)`). The retry and the final
expanded-suite Nix run passed that test. The earlier failure is retained here
and is not relabeled as a pass; it is not caused by the local-composition
additions.

## Exercised composition

The focused tests ran the real GitHub webhook shell over loopback TCP with
HMAC-SHA256 admission, the real SQLite replay store, the real Lattice Unix
transport client, and the real `kiln-github-job` binary over a temporary bare
Git repository. The in-process cases constructed the real `GithubService`; the
process cases launched the real `kiln-adapter-github` binary with the
`protocol-fixtures`-only loopback status endpoint. Only the Lattice executor
and GitHub status endpoint were test-owned peers.

The seventeen cases were:

- a passing exact-source command with one success publication;
- a genuine nonzero command with one failure publication;
- an equal delivery with the same run identity and no second job;
- an in-process restart with `RecoverDispatch` recovery and no normal redispatch;
- the real binary publishing pending and success projections through the
  fixture status server;
- killing and restarting the real binary with `RecoverDispatch` recovery and
  no second job;
- the real binary producing a failure projection for a nonzero command;
- the real binary reusing one run for an equal duplicate delivery;
- the real binary rejecting an invalid signature before any run, job, or
  status effect;
- the real binary reconciling an ambiguous status POST through read-only exact
  lookup without a second POST;
- the real binary rejecting duplicate critical headers, an oversized body, and
  an unsupported method before any effect;
- the real binary rejecting a changed profile against existing replay state
  without rewriting the original run;
- the real binary admitting a same-repository pull request at its exact head
  revision, denying a fork pull request, and ignoring a closed pull request;
- the real binary returning capacity under stalled connections, timing out
  partial requests, and completing a later valid webhook;
- the real binary leaving a conflicting terminal status `Unknown` rather than
  `Published`.

## Non-claims

This evidence proves only the local composition and replay behavior described
above. The Lattice and status peers are fixtures. The restart cases restart the
Kiln shell against the same fixture peer; they do not restart a real Lattice
executor. The process-level status endpoint is a loopback HTTP fixture selected
only in `protocol-fixtures` builds. The evidence does not claim live GitHub mutation, source trust,
workflow correctness, host confinement, distributed exactly-once execution,
deployment success, or release eligibility.

## Aspen host boundary

The standalone real Aspen host rail was run separately:

```console
cargo test --locked --offline --manifest-path runtime/kiln-aspen-host/Cargo.toml   --features test-fixture --test hosted
```

Result: 13 passed in 419.66 seconds. This exercises the real Aspen native
service and real extension process against a synthetic Lattice peer; it does
not exercise a real Lattice executor. The host's Cargo lock binds the
separately reviewed immutable Kiln revision `69afc2697f4330005797256135ae81b2722a125c`,
so this rail exercises the standalone host cohort, not the local working-copy
GitHub adapter changes.

GitHub does not select the Aspen runtime. `docs/operator.md` keeps the reviewed
GitHub Lattice path independent of `--runtime aspen`, and the Aspen host
composes the Radicle trigger profile. A GitHub-to-Aspen composition would
require a new product path and operator profile, not a test-only addition. This
change therefore does not claim that composition.

## Live GitHub smoke

Genuine combined exercise completed on 2026-10-19 against the authorized
`OnixResearch/kiln` repository.

### First attempt retained

A first genuine delivery at revision `d22e961970c53e75c331bb89d69e5bfdc5fe52f5`
was admitted by the controller and executed by the worker, but the worker job
failed before compilation because the recorded cargo vendor store path
`/nix/store/ri1q6k2qnd0icg0f7z8n98gcsf93zx81-cargo-vendor-dir` had been
garbage-collected. The worker exit was 101 and the canonical result was
`DomainFailure`. The failed delivery, worker log, and failure status are
retained in [pass-vendor-missing.json](live-github/pass-vendor-missing.json)
and [worker-vendor-missing.log](live-github/worker-vendor-missing.log). This
failure is not relabeled as a pass.

### Repaired cohort

The current workspace vendor directory was regenerated and added to the worker
store as `/nix/store/4mgsydfhxaf78w585npl82km9w21cyvy-vendor-current`. The
worker job profile was updated to that exact path, producing a new cohort:

| Fact | Value |
|---|---|
| Job profile identity | `da772a711d69e99f18ec50aefe0e0653a77fa795ef975b2d214227e7d84d8037` |
| Lattice workflow revision | `b3:730c253342a4db488264333cfd84a0665779950d4efce77f1b2d778c8d3ae847` |
| Lattice graph identity | `b3:a2db54f4689bb07f3a5ce141ae5e3f12a39c576b9cb230d7ecd0f54a498bb892` |
| Controller profile identity | `3336551ceeb95dadc825a60ad4a54a3528bc6968cf61c85eccc0c7abeefdd934` |
| Controller host | `aspen1` (`100.100.103.95`) |
| Worker host | `britton-desktop` (`100.110.43.11`) |
| Ingress | operator-owned Cloudflare quick tunnel to loopback `127.0.0.1:8787` |

The previous controller database is retained as
`github-shell-pre-vendor-fix.sqlite`; the repaired cohort used a fresh
`github-shell.sqlite` because the service profile identity changed.

### Genuine passing result

- Hook: `700577233`
- Delivery: `3849253937345495040`
- Response: `202`, accepted, not duplicate
- Source revision: `4904c836d68262aecb1de8aa9fbb0a43a1228e2f`
- Run identity: `b6348e588429f32757bcb65d395fca569c147edaa98e31b370339ab08b56370d`
- Worker probe exit: `0`
- Canonical outcome: `Success`
- Pending GitHub status: `56738475142`
- Terminal GitHub status: `56738521022`, state `success`
- Remote status URL: `https://api.github.com/repos/OnixResearch/kiln/statuses/4904c836d68262aecb1de8aa9fbb0a43a1228e2f`

The live path was: GitHub push webhook through the operator Cloudflare tunnel
to the controller GitHub adapter; controller Iroh proxy to the separate Iroh
worker; worker Lattice route to the exact-source job; job checkout of the
pushed commit; reviewed probe compilation and execution; canonical Kiln
`Success`; and a matching GitHub commit status. Evidence is retained in
[live-github/pass.json](live-github/pass.json),
[live-github/worker-live-pass.log](live-github/worker-live-pass.log), and
[live-github/webhook.json](live-github/webhook.json).

The hook, temporary status/source tokens, and live services were retired after
the exercise. The validation branch remains the exact source record.

### Claim boundary

This proves one operator-authorized repository, branch, controller, and
separate worker over the private Tailscale path plus a temporary HTTPS ingress.
It does not prove public-WAN or relay availability, host confinement, exactly-
once distributed effects, persistent deployment, token renewal, rate-limit
behavior, or release eligibility.
