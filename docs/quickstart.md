# 5-Minute Local Factory & CI Quickstart

Get Kiln—the OnixResearch AI Software Factory control plane—running locally in five minutes: try the zero-setup interactive console first, then initialize a single-config `kiln.json` workspace for your own Git repository.

> **Scope & claim boundary:** Kiln coordinates multi-machine agent fleets (`kiln-fleet` + Herdr + swarms), deterministic exact-revision CI gating (`kiln-core` + Nix/Lattice/Aspen), and `jj`/Cairn/Code-Review delivery behind one pure `no_std + alloc` state machine. Local exploration and `kiln init` workspaces are self-hosted developer tools — they do not execute untrusted code in a sandbox or prove build/release eligibility on their own. See [Security and non-claims](security-and-non-claims.md).

## 1. Clone and enter the Nix shell

Clone the repository from the GitHub mirror (or see the [README](../README.md#quick-start) for the canonical Radicle HTTPS mirror):

```console
git clone https://github.com/OnixResearch/kiln.git
cd kiln
nix develop path:$PWD
```

Why `nix develop`? It pins the exact Rust toolchain (`1.97.1`), `pkg-config`, `openssl`, `nickel`, `git`, `jj`, and the matching `cairn` CLI so builds and local workspace setup do not fail on missing system headers or version-mismatched tools.

Verify the deterministic core offline (optional, takes a minute on first build):

```console
cargo run --locked -p onix-kiln-cli -- demo
```

## 2. Try the zero-setup interactive console (60 seconds)

Launch the disposable example server with pre-populated CI runs, reviews, sources, and fleet views — no configuration or external providers needed:

```console
nix run path:.#kiln-website-exploration -- --port 18787
```

Open <http://127.0.0.1:18787/examples> in your browser. Everything is local and disposable (restart to reset; `Ctrl-C` to stop).

What to click first:
1. **`0.0 Guide`** (`/?page=guide`) — overview of every console area plus the **Plain-English Glossary** (`Cairn`, `Lattice`, `Herdr`, `Fence`, `Admitted`, `Candidate-set identity`, and more).
2. **`2.2 Runs & Evidence`** — inspect an exact-revision CI run, step logs, artifacts, and replayable event history.
3. **`2.3 Code Review & Queue`** — browse revision-bound patch reviews, interdiffs, and required-CI checks.
4. **`1.1 Factory`** — see how escalations, active delivery lanes, and machine claims surface for an operator.

## 3. Initialize a single-config workspace (`kiln.json`)

Generate a self-contained local workspace directory (`kiln.json` with schema `kiln.factory-workspace.v1`, `service.json`, `access.json`, `sources.json`, `START_HERE.txt`, private bearer tokens, and a local `localhost` TLS certificate) with `kiln init`:

```console
cargo run --locked -p onix-kiln-cli -- init --repo /path/to/your/git-repo --out /tmp/my-kiln
```

(`kiln init` refuses to overwrite an existing `--out` directory and makes zero network or provider calls.)

Next, run `kiln doctor`, validate the workspace configuration offline with `kiln check`, and start the local HTTPS service with `kiln serve`:

```console
cargo run --locked -p onix-kiln-cli -- doctor --config /tmp/my-kiln/kiln.json
cargo run --locked -p onix-kiln-cli -- check --config /tmp/my-kiln/kiln.json
cargo run --locked -p onix-kiln-cli -- serve --config /tmp/my-kiln/kiln.json
```

If you built or installed the binaries onto `$PATH` (e.g., via `nix build`), run the unified CLI commands directly:

```console
kiln init --repo /path/to/your/git-repo --out /tmp/my-kiln
kiln doctor --config /tmp/my-kiln/kiln.json
kiln check --config /tmp/my-kiln/kiln.json
kiln serve --config /tmp/my-kiln/kiln.json
kiln fleet --help
kiln ci --help
```

Open <https://127.0.0.1:9443>, accept the local self-signed certificate (`/tmp/my-kiln/cert.pem`), and sign in with the operator token stored in `/tmp/my-kiln/operator.token`. You can browse local commits and files immediately; dispatching live workflow jobs later requires connecting a [Lattice](lattice-workflow.md) worker to `/tmp/my-kiln/lattice.sock`.

## Which binary do I run?

| Binary | Package | What it does |
| --- | --- | --- |
| `kiln` | `onix-kiln-cli` | Unified Factory & onboarding CLI (`init`, `doctor`, `check`, `serve`, `fleet`, `ci`, `dashboard`, `demo`, `evidence`, `host-qualify`, `native-git`) |
| `kiln-web-service` | `kiln-web-service` | Single-tenant HTTPS web console, local CI & runbook service (`check`, `serve`, `ci`) |
| `kiln-fleet` | `kiln-fleet` | Multi-machine claim authority, work coordination, and agent swarm dispatch |
| `kiln-implementer` | `kiln-adapter-github` | Durable implementation ledger for queued deliveries, attempts, and verification |
| `kiln-dashboard` | `kiln-dashboard` | Standalone read-only terminal or offline HTML report renderer |

## Plain-English Glossary

New to Kiln's terminology (`Cairn`, `Lattice`, `Mantle`, `Aspen`, `Herdr`, `Fence`, `Admitted`, `Candidate-set identity`, `Receipt`)? Open **`0.0 Guide`** (`/?page=guide`) inside the interactive console for one-sentence plain-English definitions of every term.

## Where to go next

- [Documentation index](README.md): full map of specs, sources, and adapters.
- [Architecture](architecture.md): pure core vs. imperative shells and crate boundaries.
- [Operator guide](operator.md): runbook index covering [Deploy & upgrade](operator/deploy-and-upgrade.md), [Fleet & Factory](operator/fleet-and-factory.md), [CI & providers](operator/ci-and-providers.md), and [Recovery & qualification](operator/recovery-and-qualification.md).
- [Detailed status](status.md): maturity ledger, retained receipts, and publication record.
- [Security and non-claims](security-and-non-claims.md): what passing checks do and do not prove.
