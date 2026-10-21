# Verification: live native Radicle peer and broker canary

## Verdict and scope

PASS for one fresh same-host native Radicle publisher/CI/observer topology on
trusted reviewed canary source. The observed window is
`2026-10-21T05:30:40Z` through `2026-10-21T05:33:57Z`. The canary repository is
`rad:z3YGUHdF38sPfhBHYjMUuQ8VKDveJ` on branch `ci-canary`; source and status use
distinct node identities.

This is not WAN or distinct-host delivery, hostile-worker confinement,
production deployment, activation, release readiness or authority over a
production repository. The complete case records are in
[`verification.json`](verification.json) and the raw runner report in
[`live-report.json`](live-report.json).

## Source binding

- Kiln parent commit: `17e8d9c80e2cd4819bb5129223ea15216600e181`
  (`Record exact Radicle CI source publication and anonymous readbacks`).
- Working-copy change: `mvnlmrvnyouolqyurttvpzquurmtrvnw`.
- The runner, signer helper and live admission contract are working-copy
  additions; `crates/kiln-radicle-ci/src/main.rs` adds the read-only
  `status-reconcile` command.
- The canary runner and live contract were not modified after the run. A
  repository formatting pass changed only the import layout of
  `crates/kiln-radicle-ci/src/main.rs`; the run-time coordinator BLAKE3 below is
  the authoritative execution identity.
- Secret material is absent from this bundle: no node private keys, no generated
  `.env`, and no operator credential. The SecretSpec audit log records key name,
  provider path, outcome and reason only.

| Run-time artifact | Path | BLAKE3 |
| --- | --- | --- |
| coordinator | `/var/tmp/kiln-live-radicle-ci-cargo/debug/kiln-radicle-ci` | `bfbc49a722488b58d5076636925ce068d78a6e7d44642eea91fd4d0e5e80599f` |
| project worker | `/var/tmp/kiln-live-radicle-ci-cargo/debug/kiln-onixos-worker` | `c8a18576e32124b05fa3703214d490de091f6f0c11386391e7b41c6b8b80fa2b` |
| Aspen extension | `/var/tmp/kiln-live-radicle-ci-cargo/debug/kiln-aspen-extension` | `89fe72116fe16e3edf1ecaad84bb3f675634a23947d578fe357fc42c58a52892` |
| Aspen host | `/nix/store/nwd0m4dmlb24kh2hr8m5p9s9vms6l47h-kiln-aspen-host-0.1.0/bin/kiln-aspen-host` | `33b2e2d94fea23469d7fb1ee1b4723033382a7f5336c635c7b5a59451caf0f5d` |
| Lattice | `/nix/store/mwqnyq525wpxn0jw6w7c06fq41l98z9m-lattice-0.1.0/bin/lattice` | `4c3773e852f6b66cb33677e74899d37a2bed0dcf4742698c40d12d0813783aba` |
| broker | `/nix/store/vs43s62r8vsvfxfx2c5xm01338x9y4n6-radicle-ci-broker-0.31.0/bin/cib` | `1f363ac9e7b00b078a6a6efcdcdb50777b3223090e7272c3d7db264e509d365d` |
| broker tool | `/nix/store/vs43s62r8vsvfxfx2c5xm01338x9y4n6-radicle-ci-broker-0.31.0/bin/cibtool` | `230a8bc8717d38f8cdffa7de5b3ceb6fd1a0c8a0d2a48bce9f96e72efd4b7348` |
| Radicle | `/nix/store/pjwcxc0r6xaqjps6yy80nf4jv7af3mza-radicle-node-1.10.2/bin/rad` | `4ba3aca4155c9ee0a05d17e94748b355f5903ebd1b7565a51591cc4e02f40f38` |
| Radicle node | `/nix/store/pjwcxc0r6xaqjps6yy80nf4jv7af3mza-radicle-node-1.10.2/bin/radicle-node` | `e9dda54b463e77766d18bd7810b79f8be203e6cd1d42e9c532ac646103b2ffd1` |
| Nix | `/nix/store/g1k5iczz0l8i8d7kk5bk8rwv139g3l02-nix-2.34.8/bin/nix` | `82a5990db42a7f104b2c6eb17edf840ac9a7ad04936c8ab2630cc766e929f852` |

Full executable paths and digests, runtime profile, source-selection limits and
settings are in [`cohort.json`](cohort.json). The tested source files are:

| File | BLAKE3 |
| --- | --- |
| `scripts/radicle-ci-live.py` | `dc30685acbbcabf168a001a423d2fa50c502081531876ac6f668940ffe39ef25` |
| `scripts/radicle_ci_live_signer.py` | `80f677843b49c58de33b87a30b98b9ed932f505243b3c748fcb314bfc6322e53` |
| `scripts/onixos-ci-pilot.py` | `40cf8828a362720ea74fcc6741c5426aa42b7a21d16eb147cebc59309f7159c3` |
| `config/radicle-ci-live.ncl` | `946769a35fab72587276504fb8e024079b06965038e4b293103fea858c49d1cc` |
| `config/radicle-ci.ncl` | `7d1f09253686068665b75a554c8dc6f54d25aae43ff4e6fda763a23b32a1441b` |
| `config/radicle-ci-worker.ncl` | `99c2528d88d51a61fcb3b72cac082e5581d4c09a608c28afaf8b917bd7906ef4` |

## Exact repeat command

Build the current controller, worker, extension and helper binaries first and
record their identities. Use resolved paths from the recorded cohort; the
`--inputs` file is the archived closure input set, which the runner re-exports
as `<root>/admitted-inputs.ncl`. The root must not exist.

```console
nix develop path:$PWD -c python3 scripts/radicle-ci-live.py \
  --inputs .cairn/archive/1970-01-01-build-authenticated-onixos-home-closure/evidence/success/inputs.json \
  --nickel /nix/store/5yqkl5bk4d335hnbss23w4a2crfafxlp-nickel-1.17.0/bin/nickel \
  --nixpkgs /nix/store/1dffznc7fgw6xnzssy1h4waxz63i11qa-source \
  --worker /var/tmp/kiln-live-radicle-ci-cargo/debug/kiln-onixos-worker \
  --extension /var/tmp/kiln-live-radicle-ci-cargo/debug/kiln-aspen-extension \
  --coordinator /var/tmp/kiln-live-radicle-ci-cargo/debug/kiln-radicle-ci \
  --radicle /nix/store/pjwcxc0r6xaqjps6yy80nf4jv7af3mza-radicle-node-1.10.2/bin/rad \
  --radicle-node /nix/store/pjwcxc0r6xaqjps6yy80nf4jv7af3mza-radicle-node-1.10.2/bin/radicle-node \
  --broker /nix/store/vs43s62r8vsvfxfx2c5xm01338x9y4n6-radicle-ci-broker-0.31.0/bin/cib \
  --broker-tool /nix/store/vs43s62r8vsvfxfx2c5xm01338x9y4n6-radicle-ci-broker-0.31.0/bin/cibtool \
  --jj /nix/store/hwifmhfi50y1595lhwiqsn7acppa7zdm-jujutsu-0.45.1/bin/jj \
  --git /nix/store/8yjpgadavqdwphcd2kw4dfvxmgdp5sb3-git-2.55.0/bin/git \
  --secretspec /nix/store/a9hlwn1m4p0pi17yif3aa3ak2d0majz5-secretspec-0.19.0/bin/secretspec \
  --root "/var/tmp/kiln-radicle-live-$(date -u +%Y%m%dT%H%M%SZ)"
```

`config/radicle-ci-live.ncl` admits only a canonical `/var/tmp/<fresh>`
descendant, the exact source/checkpoint branch pair and pinned store
executables. The signer helper derives the dedicated status seed from the fresh
CI node's unencrypted Ed25519 key, writes a mode-0600 SecretSpec manifest and
`.env`, and returns both the key and `.env` for retirement. No operator
credential or existing Radicle home is read.

## Topology

All three nodes are separate processes with distinct identities on one host.
Peers are declared explicitly; preferred seeds, relays, external addresses and
default connectivity are disabled.

| Role | Node ID | Loopback port | Owned home |
| --- | --- | --- | --- |
| publisher | `z6MkgUFx1QThezL9gA481xG1PUFqGie3JqJAFyEodBLKqxVf` | 50505 | `<root>/rad-publisher` |
| CI | `z6MkkxMnmHmh1JH92F1DMWFmbS4ytH3kAvJWRko6Z2vmNixK` | 51445 | `<root>/rad-ci` |
| observer | `z6MkpNkn52JKmoXxQGk97wgRu9HRL9mSxf2XkrstCpf1quR6` | 60215 | `<root>/rad-observer` |

The broker trigger admits exactly the canary RID, the publisher node, branch
`ci-canary`, and `BranchCreated`/`BranchUpdated` native events. The exact broker
configuration is [`broker.json`](broker.json).

## Observable cases

| Case | Source revision | Canonical outcome | Broker replies | Independent remote status |
| --- | --- | --- | --- | --- |
| success | `5e911e728257459e2e4d842097142c8183eec4d2` | `Success` | `triggered`, `finished(success)` | `success` at same revision |
| failure | `a87d000066f716094c62c8d7582e2ba7c7772755` | `DomainFailure` | `triggered`, `finished(failure)` | `failure` at same revision |
| unavailable | `b830e845152a5caf983ce75156017f92fd6bff68` | `Pending` | `triggered` only | absent |

Each source update arrived as a real native peer fetch; the broker's own event
subscription wrote the original adapter request. The retained
[`ingress/`](ingress/) tree contains the exact `request.json`, attempt markers
and attempt stdout for every revision. No manual `cibtool trigger`, fixture
provider or rewritten workflow supplied this ingress.

### Success

The canonical journal reserved operation
`45ef9b4727d8c96f1313bee5f08a7f3dfc969d8accd00df737a52fd1adced13c`. Its
consumer output was absent before dispatch. An explicit interruption stopped the
controller host while the independent Lattice execution and worker continued;
the canonical outcome was `Pending`, the worker result was absent, and the
original operation was unchanged. After host restart and reconcile of the
original broker bytes, the run completed with exactly one provider invocation
and produced root `/nix/store/wnjqwam4y62w8nvwpiq1hhz0cqqfd5zd-kiln-radicle-live-ci`
containing the exact source-controlled output.

The status observation was written locally before signing. After explicit
signing/synchronization of `refs/heads/kiln-status-checkpoint`, an independent
fetch/reduce in observer storage returned the same repository, revision,
`run_id`, `success`, object ID and head
`a5f4b42418dfab6b8799dd20d644b67ca6cf2eb3`, with the same signer identity. The
pre-sync remote read was absent. Two idempotent releases then removed the
source and build roots.

### Source-controlled failure

Revision `a87d000066f716094c62c8d7582e2ba7c7772755` genuinely fails its Nix
build; the retained diagnostics contain
`source-requested-genuine-build-failure`. Canonical `DomainFailure` and the
independently fetched `failure` status agree at that revision (head
`10bcc44ff4b03e7808da7061931ed10888d737d7`, operation
`97516dae8109150d1bf214872affd90279684e81868c6f6166a5754f0a3b52c9`). No build
output root was produced, and both release invocations succeeded.

### Unavailable provider

For revision `b830e845152a5caf983ce75156017f92fd6bff68` the host was stopped
before dispatch and its socket removed. The original reservation
`3bded2cff41266c431c7e1262f3c40161bfa1426cea40169f6da7494c32d17b9` remained
nonterminal: the only broker reply was `triggered`, no status intent or remote
status existed, and no root was produced. Duplicate native fetch and
reconciliation preserved the original operation and did not redispatch a worker.

## Duplicate delivery and invocation count

A real duplicate native fetch was performed after each case. Reconciliation
preserved the original operation in all three cases, and the provider invocation
log remained exactly the two genuine source revisions:

```text
5e911e728257459e2e4d842097142c8183eec4d2
a87d000066f716094c62c8d7582e2ba7c7772755
```

The upstream broker's deduplication is retained separately and is not treated
as canonical reconciliation.

## Configuration admission

[`canary-admission.json`](canary-admission.json) records 11 Nickel cases: one
positive composition and ten negatives covering a non-canary root, `.` and `..`
components, the `/var/tmp` parent, wrong checkpoint/source branches, a
substituted SecretSpec executable, a non-store tool, and unknown fields or
tools. Each negative fails before any canary effect.

## Rejected attempts

[`rejected-attempts.json`](rejected-attempts.json) retains the six earlier
fresh roots that stopped before a verification report: two bootstrap attempts
and four canary attempts. Five of the six reached the pilot's retirement path
with no errors and `private_credentials_retired: true`; the earliest bootstrap
root stopped before teardown and is retained only as a setup failure, not as a
retired or qualified result. Each root used its own node homes and no result
from an earlier root is promoted to the qualified cases.

## Retirement

[`retirement.json`](retirement.json) records eight owned services stopped with
no errors, no mutation of production state or status, and
`private_credentials_retired: true`. Six services received SIGTERM; the two
controller hosts were explicitly SIGKILLed by the interruption and
unavailable-provider cases. Node private-key files and the generated status
`.env` are absent from the canary root; the bundle contains no private keys or
seed values. [`teardown-releases.json`](teardown-releases.json) records the
idempotent release results for the completed cases.

## Non-claims

- same-host native loopback TCP is not WAN or distinct-host qualification;
- trusted canary source and shared-user workers do not establish hostile-worker
  confinement;
- a real Nix build is not activation, deployment, release authority or arbitrary
  reproducibility;
- upstream event deduplication is distinct from canonical controller
  reconciliation;
- private dedicated test identities grant no production repository or
  governance authority.


## Gates

All required repository gates were run on the final source and lifecycle bytes;
each receipt is retained under [`gates/`](gates/).

| Gate | Result | Command | Receipt |
| --- | --- | --- | --- |
| format | PASS | `nix develop path:. --no-write-lock-file -c cargo fmt -- --check` | `gates/format.json` |
| workspace-tests | PASS | `nix develop path:. --no-write-lock-file -c cargo test --locked --workspace` | `gates/workspace-tests.json` |
| clippy | PASS | `nix develop path:. --no-write-lock-file -c cargo clippy --locked --workspace --all-targets -- -D warnings` | `gates/clippy.json` |
| cairn-validate | PASS | `nix develop path:. --no-write-lock-file -c cairn validate --root . --policy cairn-policy/generated/cairn-policy.json` | `gates/cairn-validate.json` |
| traceability | PASS | `nix develop path:. --no-write-lock-file -c cairn traceability coverage --root . --policy cairn-policy/generated/cairn-policy.json --json` | `gates/traceability.json` |
| release-readiness | PASS | `nix develop path:. --no-write-lock-file -c cairn release-readiness --root .` | `gates/release-readiness.json` |
| octet | PASS | `nix build path:.#checks.x86_64-linux.octet --no-link -L` | `gates/octet.json` |
| pijul-native-fixtures | PASS | `nix build path:.#checks.x86_64-linux.pijul-native-fixtures --no-link -L` | `gates/pijul-native-fixtures.json` |
| nix-flake-check | PASS | `nix flake check path:. --no-write-lock-file -L` | `gates/nix-flake-check.json` |
| tasks-after-completion | PASS | `nix develop path:. --no-write-lock-file -c cairn gate tasks qualify-live-radicle-ci --root . --policy cairn-policy/generated/cairn-policy.json` | `gates/tasks-after-completion.json` |
| sync-execute | PASS | `nix develop path:. --no-write-lock-file -c cairn sync qualify-live-radicle-ci --root . --policy cairn-policy/generated/cairn-policy.json --execute` | `gates/sync-execute.json` |
| archive-execute | PASS | `nix develop path:. --no-write-lock-file -c cairn archive qualify-live-radicle-ci --root . --policy cairn-policy/generated/cairn-policy.json --execute` | `gates/archive-execute.json` |
| cairn-validate-post-archive | PASS | `nix develop path:. --no-write-lock-file -c cairn validate --root . --policy cairn-policy/generated/cairn-policy.json` | `gates/cairn-validate-post-archive.json` |
| release-readiness-post-archive | PASS | `nix develop path:. --no-write-lock-file -c cairn release-readiness --root .` | `gates/release-readiness-post-archive.json` |
| traceability-post-archive | PASS | `nix develop path:. --no-write-lock-file -c cairn traceability coverage --root . --policy cairn-policy/generated/cairn-policy.json --json` | `gates/traceability-post-archive.json` |
| status-post-archive | PASS | `nix develop path:. --no-write-lock-file -c cairn status --root .` | `gates/status-post-archive.json` |
