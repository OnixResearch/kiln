# Kiln 0.1.0

> Draft release body. Do not publish until
> [launch-checklist.md](launch-checklist.md) is green and the release owner has
> replaced the qualification section with the tagged revision's results.

Kiln is a provider-neutral control plane for continuous integration. A pure core
turns commands into events and requested effects; adapters for Git, Jujutsu,
Pijul, Radicle, GitHub, Gitea, Lattice and Nix perform them. Every result binds
to an exact revision, unknown outcomes stay explicit, and recorded state can be
replayed.

## Try it

Kiln 0.1.0 is source-only; the first build compiles the workspace from source,
and no binary is published.

Clone the read-only HTTPS projection without a Radicle node:

```console
GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=protocol.version GIT_CONFIG_VALUE_0=0 \
  jj git clone https://seed.radicle.garden/z2wsvXm5S2sJGvuV1k5JHiwi1PbKE.git kiln
cd kiln
```

Select and verify the intended exact revision; `main` is a moving reference.
The read-only GitHub projection needs only Git:
`git clone https://github.com/OnixResearch/kiln.git`.

```console
nix develop path:$PWD
cargo test --locked --workspace
cargo run --locked -p kiln-cli -- demo
```

Expected output:

```json
{"revision":1,"runs":{"demo-run":{"active_attempt":"demo-attempt","outcome":"Pending"}},"attempts":{"demo-attempt":{"run_id":"demo-run","provider_profile":"standalone-build-v1","outcome":"Pending","external_reference":null,"products":[],"metrics":[]}},"holds":[],"seen_commands":["demo-start"]}
```

Preview the local dashboard:

```console
cargo run --locked -p kiln-dashboard --example preview -- /tmp/kiln-dashboard-preview
```

The preview writes a synthetic offline dashboard, not CI evidence.

## Highlights

- Pure core with two transitions and replayable state. (workspace tests)
- Exact-revision and exact-candidate results; `Unknown` stays explicit.
  (workspace tests)
- GitHub exact-source jobs with matching statuses; pass, fail and recovery.
  (exercised live once)
- Radicle source-to-build CI with exact-revision status and tag triggers.
  (exercised live on one same-host topology and across two owned hosts)
- Reviewed Lattice workflows with Iroh transport. (exercised live with GitHub)
- Nix job discovery and exact local derivation builds. (exercised locally)
- Single-tenant HTTPS workspace with logs, reruns, triggers and schedules.
  (exercised locally)
- Change review with revision-bound approvals and required-CI merge automation.
  (exercised locally)
- Bounded observation over HTTPS or iroh, plus a local dashboard.
  (fixture-tested and exercised locally)
- Optional `kiln-hosted` with per-job Firecracker and Cloud Hypervisor profiles.
  (exercised locally; `hostile_isolation_claim:false`)

## How it works

```text
state + command -> events + requested effects
state + event   -> new state
```

Kiln owns CI meaning and delegates external effects to replaceable adapters. A
shell commits accepted events and outbox records together, then dispatches
effects only after provider capability admission. Replay folds events through
`evolve` and returns no provider effects.

## What ships

| Program | Role |
| --- | --- |
| `kiln` | Run the Kiln local operator CLI |
| `kiln-adapter-github` | Run the Kiln GitHub webhook and recovery service |
| `kiln-implementer` | Drain authenticated GitHub issues into admitted implementation work |
| `kiln-lattice-dispatch` | Dispatch one reserved implementation attempt to an admitted Lattice request |
| `kiln-cairn-intake` | Admit complete Cairn implementation-ready snapshots into the durable Kiln work ledger |
| `kiln-adapter-gitea` | Run the Kiln Gitea/Forgejo webhook and recovery service |
| `kiln-adapter-iroh` | Run the authenticated Iroh native workflow worker or Unix proxy |
| `kiln-patchgate-review` | Run isolated Patchgate review and retain exact non-authorizing evidence |
| `kiln-github-job` | Run a pinned exact-source GitHub CI job |
| `kiln-gitea-job` | Run a pinned exact-source Gitea/Forgejo CI job |
| `kiln-radicle-ci` | Run source-bound Radicle CI and original-operation recovery |
| `kiln-dashboard` | Render the local operator dashboard over a canonical Kiln journal |
| `kiln-hosted` | Run the explicitly admitted Iroh-connected hosted CI composition |
| `kiln-web-service` | Run the authenticated source-bound single-tenant CI web service |
| `kiln-nix-cache` | Render a bounded read-only Nix binary-cache projection |
| `kiln-dashboard-refresh` | Collect bounded canonical CI histories and publish one dashboard report |
| `kiln-dashboard-publish` | Render and atomically deliver an explicitly configured dashboard catalog |

## Release qualification

This section records preparation, not the release. It was exercised on
2026-11-12 on one `x86_64-linux` host in the development shell, at source
revision `428d03fcfb02478ed79d38453fb44f82c6a2bdd5` plus the launch
preparation changes. Replace it with the tagged revision's results before
publishing.

| Check | Outcome |
| --- | --- |
| `cargo run --locked -p kiln-cli -- demo` | Printed the one-line state above; no network, VCS or filesystem write and no dispatch |
| `cargo run --locked -p kiln-dashboard --example preview -- /tmp/kiln-dashboard-preview` | Wrote a synthetic journal and an offline HTML dashboard |
| `cargo test --locked --workspace` | Exit 0: 1608 passed, 0 failed, 5 ignored across 160 result lines; 3m36s wall time with a warm cache |
| `cargo test --locked -p kiln-adapter-github --features protocol-fixtures --test local_ci_e2e` | 17 passed against isolated fixtures; not a live GitHub run |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Pass |
| `cargo fmt --all --check` | Pass |
| `scripts/check-nickel-fixtures.sh` | Pass with 78 positive and 567 negative fixtures, after canonical formatting of two configuration files that failed its format check |
| `cairn validate` | Pass |
| `cairn traceability coverage` | Pass: 1742 referenced, none missing, none dangling; declared linkage, not execution truth |
| `cairn release-readiness --root .` | `valid: true`, `verdict: pass`, `onix_release_ready: false` |

Not run: the Octet check and `nix flake check` with its 45 checks. The
packaged `nix build path:$PWD#kiln` output was not exercised, so `nix run` is
unverified.

Retained receipts for earlier recorded cohorts. None validates this revision:

| Receipt | Bound |
| --- | --- |
| [GitHub exact-source jobs on a separate Iroh worker](../../.cairn/archive/1970-01-01-qualify-live-github-iroh-ci/evidence/verification.json) | Only the three genuine operated GitHub-to-separate-Iroh-worker-to-GitHub cases; not persistent deployment, whole-workspace remote CI certification or release readiness |
| [Native Radicle peer update to exact-revision status, observed 2026-10-21](../../.cairn/archive/1970-01-01-qualify-live-radicle-ci/evidence/verification.json) | One fresh same-host native Radicle publisher, CI and observer topology on trusted reviewed canary source; not WAN or distinct-host delivery, hostile-worker confinement or production deployment |
| [Radicle source delivery across a private tailnet between two owned hosts](../../.cairn/archive/1970-01-01-qualify-distinct-host-radicle-ci/evidence/verification.json) | Not WAN, hostile-network, hostile-worker confinement, production deployment, activation or release authority |
| [Single-tenant web service exercised natively](../../.cairn/archive/1970-01-01-operate-single-tenant-web-service/evidence/native-final/verification.json) | Isolated local evidence, not production deployment; bounds are not hostile confinement |

## Requirements and limits

- Nix is the install path; no crates.io, binary release or container image.
- `x86_64-linux` was exercised on one host; `aarch64-linux` is unchecked.
- Workflow execution goes through the Lattice adapter; GitHub, Gitea and the web
  service need a reviewed Lattice runtime.
- Kiln is not a sandbox and claims no hostile-code confinement.
- No hosted service is available; hosted CI is planned.
- Gitea/Forgejo and Tangled are fixture-tested, not live interoperability.
- The tree is mixed-license; read [LICENSES.md](../../LICENSES.md).

## Hosted CI

Hosted CI is planned and not available. The [commercial plan](../commercial-launch.md)
proposes an invite-only B2B pilot with GitHub as the first connector. No
numerical price, merchant approval or live charge exists.

## Links

[README](../../README.md), [status](../status.md),
[operator guide](../operator.md),
[security and non-claims](../security-and-non-claims.md),
[security policy](../../SECURITY.md), [contributing](../../CONTRIBUTING.md),
[license map](../../LICENSES.md), and [changelog](../../CHANGELOG.md).

When pasting into a GitHub release, replace each relative link with
`https://github.com/OnixResearch/kiln/blob/<REVISION>/...`, where `<REVISION>` is
the tagged commit.

## License

Kiln is a mixed-license source tree. Kiln-owned packages default to
`MIT OR Apache-2.0`, except `kiln-adapter-pijul` (`GPL-2.0-only`). Several
binaries link `AGPL-3.0-or-later` or `MPL-2.0` components, so combined
artifacts are not permissive-only. See [LICENSES.md](../../LICENSES.md).
