# Verification: distinct-host Radicle status dissemination

## Verdict and scope

PASS for one distinct-host status observer with local status authority. The
publisher and observer nodes and their source/status storage ran on `aspen1`.
The local CI node listened on the private tailnet and accepted the observer's
direct TCP connection. Status object creation, signing and checkpoint
synchronization remained local. The remote observer fetched and reduced the
exact status in its own storage with the pinned read-only
`kiln-radicle-ci status-reconcile` command.

The observed window is `2026-10-21T13:15:32Z` through `13:19:39Z`. The canary
repository is `rad:zytcgVoHgABnUrHm8j5vKaGo2cBS` on branch `ci-canary`.

This is not WAN, hostile-network, hostile-node isolation, production
deployment, activation or release authority. The second host is a trusted,
operator-owned machine.

The raw runner report is [`verification.json`](verification.json); the complete
bundle is [`live/`](live/).

## Source binding

- Kiln main commit: `13550b839db6be786138fdb2341e44597d281203`.
- Working-copy change: `wzvlqvwp` (this change is uncommitted).
- The run used the status runner with the control-socket readiness fix, the
  generic remote shell helpers and the status/distinct profiles. Post-run
  changes are a requirement comment and Nickel formatting; valid-input
  admission and runner behavior are unchanged. Hashes are the final bytes.
- No private key, `.env`, seed or operator credential is present in this bundle.

| File | BLAKE3 |
| --- | --- |
| `scripts/radicle-ci-live-distinct-status.py` | `a27b2dfceb3aade55c0050584e152bce5458a3e811e47fe4a739bf266f4277bc` |
| `scripts/radicle-ci-live-distinct-host.py` | `51419beec200ae3c11ce813b243bf47a5574ccdff54b737c27004fa80f8c5799` |
| `scripts/radicle-ci-live.py` | `be3c8380c7a18e4ed5d7327ffcb6cdd850c08d536232fd35544f29fb26746654` |
| `config/radicle-ci-live-distinct-status.ncl` | `28a6555261abc80ba70972e12fcb30b781f3c6c17c648dfa6a35187cfa6d4318` |
| `config/radicle-ci-live-distinct-host.ncl` | `a4b771849e0f37bfe8047fcc5c3767ef00ff080b6b9760884bac7b0473d6264a` |
| `config/radicle-ci-live.ncl` | `946769a35fab72587276504fb8e024079b06965038e4b293103fea858c49d1cc` |

## Exact repeat command

Use the same canonical `/var/tmp` path on both hosts. Both roots must not exist.
The publisher is remote as in the previous qualification; this exercise also
moves the observer and adds the local CI tailnet address.

```console
nix develop path:$PWD -c python3 scripts/radicle-ci-live-distinct-status.py \
  --inputs .cairn/archive/1970-01-01-build-authenticated-onixos-home-closure/evidence/success/inputs.json \
  --nickel /nix/store/5yqkl5bk4d335hnbss23w4a2crfafxlp-nickel-1.17.0/bin/nickel \
  --nixpkgs /nix/store/1dffznc7fgw6xnzssy1h4waxz63i11qa-source \
  --worker /nix/store/z66g0kfhxfglwbjd8m74f7wjssjd6ll4-kiln-0.1.0/bin/kiln-onixos-worker \
  --extension /nix/store/z66g0kfhxfglwbjd8m74f7wjssjd6ll4-kiln-0.1.0/bin/kiln-aspen-extension \
  --coordinator /nix/store/z66g0kfhxfglwbjd8m74f7wjssjd6ll4-kiln-0.1.0/bin/kiln-radicle-ci \
  --radicle /nix/store/pjwcxc0r6xaqjps6yy80nf4jv7af3mza-radicle-node-1.10.2/bin/rad \
  --radicle-node /nix/store/pjwcxc0r6xaqjps6yy80nf4jv7af3mza-radicle-node-1.10.2/bin/radicle-node \
  --broker /nix/store/vs43s62r8vsvfxfx2c5xm01338x9y4n6-radicle-ci-broker-0.31.0/bin/cib \
  --broker-tool /nix/store/vs43s62r8vsvfxfx2c5xm01338x9y4n6-radicle-ci-broker-0.31.0/bin/cibtool \
  --jj /nix/store/hwifmhfi50y1595lhwiqsn7acppa7zdm-jujutsu-0.45.1/bin/jj \
  --git /nix/store/8yjpgadavqdwphcd2kw4dfvxmgdp5sb3-git-2.55.0/bin/git \
  --secretspec /nix/store/a9hlwn1m4p0pi17yif3aa3ak2d0majz5-secretspec-0.19.0/bin/secretspec \
  --root "/var/tmp/kiln-radicle-live-dist-status-$(date -u +%Y%m%dT%H%M%SZ)" \
  --remote-host aspen1 \
  --remote-root "<same path>" \
  --remote-address 100.100.103.95 \
  --local-address 100.110.43.11 \
  --remote-port 52000 \
  --remote-readiness-seconds 60
```

The runner admits only a canonical fresh `/var/tmp/<name>` on both hosts, a
tailnet `100.64.0.0/10` local and remote address, the declared SSH destination
and the pinned cohort. It waits for the previous CI control socket to disappear,
starts the tailnet listener, and requires a successful `rad node status` command
before connecting peers. The remote observer is stopped and its log/config are
copied back before the shared remote root is removed.

## Topology

| Role | Placement | Node ID | Address |
| --- | --- | --- | --- |
| publisher | `aspen1` (remote) | `z6MkpA15SwepG5VoEhV8GK3tFjWt42soQcRpfM55u349CZvF` | `100.100.103.95:52000` |
| CI | `britton-desktop` (local) | `z6MkgqVzJrFdryg9731vYNzfRtc9zhLv235GzyBANbuPf1js` | `100.110.43.11:60763` (listener `0.0.0.0:60763`) |
| observer | `aspen1` (remote) | `z6MkoZt6HpmXfBnyJd1hQWGEpMExiNfuR6tNB15W65AzAKRk` | observer outbound to the CI; local listen `127.0.0.1:34979` |

The remote observer storage is
`<root>/rad-observer/storage` on `aspen1`. The local CI node retains its local
control socket for the broker and controller.

## Transport evidence

[`transport-evidence.json`](transport-evidence.json) records:

- local CI: `Listening on 0.0.0.0:60763` and
  `Connected to z6MkoZt6… (100.100.103.95:55204) (Inbound)`;
- remote observer: `Connected to z6MkgqVz… (100.110.43.11:60763) (Outbound)`
  and repeated successful fetches of `rad:zytcgVo…`;
- remote reduction commands (`status-reconcile`) exited zero against the remote
  observer storage.

A pre-run LAN probe timed out in both directions; the private tailnet accepted
direct TCP both ways. SSH was used only for provisioning, process control and
the read-only reconcile invocation. No Radicle port was forwarded. The private
overlay is not presented as WAN or a hostile network.

## Cases

| Case | Source revision | Canonical | Broker replies | Remote reduction |
| --- | --- | --- | --- | --- |
| success | `33343c7dc3c782b8685368f5236ef09200b8929a` | `Success` | `triggered`, `finished(success)` | `success`, head `70dec4bbb32e5c3968a009fed2b458071dcdc518` |
| failure | `ebd099d8499eb28f64f7613d486d35bd1f8c8059` | `DomainFailure` | `triggered`, `finished(failure)` | `failure`, head `f521ead2f4f0adb0df5fdd1ad8bb38c2b3518350` |
| unavailable | `a3f8f5367e568531276d9680abbafe0fe4684780` | `Pending` | `triggered` only | absent |

For success and failure the remote pre-sign read returned no object
(`before_sign_sync: null`), then after local signing/synchronization and the
remote fetch the pinned remote `status-reconcile` returned the exact repository,
revision, run, outcome, object ID and head written locally. The unavailable case
retained no status intent and no remote object.

The success case also retained the controller host-kill recovery and the remote
publisher restart from the distinct-host source qualification. The provider
invocation log contains exactly the two genuine source revisions:

```text
33343c7dc3c782b8685368f5236ef09200b8929a
ebd099d8499eb28f64f7613d486d35bd1f8c8059
```

## Status authority

- Status object creation and the exact intent stay with the local controller.
- The local CI node signs and synchronizes only `refs/heads/kiln-status-checkpoint`.
- The remote observer has its own node identity and storage; it fetches the
  checkpoint ref and reduces it with the pinned read-only command.
- The status signer is the CI node identity, distinct from the publisher and
  observer.
- The remote storage is not a copy of the local observer storage; the remote
  reconcile reads the remote path directly.

## Configuration admission

[`config-admission.json`](config-admission.json) records 20 cases: three
positive compositions (base, distinct-host, distinct-status) and seventeen
negatives. The status negatives cover a non-tailnet local address and a missing
local address; the distinct-host and base negatives cover the existing host,
root, address, port, missing block, branch, tool, SecretSpec and unknown-field
cases. Every negative rejects before runner effects. The base profile remains
closed; the distinct profile projects the base fields; the status profile
removes only its declared `local_address` before the distinct call.

## Rejected attempt

[`rejected-attempt-a/summary.json`](rejected-attempt-a/summary.json) records the
first status attempt. After the second CI restart the control-socket file
appeared before the node accepted commands; `rad node connect` failed with
`connection refused`, and the failure path stopped the node. The fix waits for
the previous socket to disappear, removes a stale socket, and requires a
successful `rad node status` command before connecting peers. The attempt
retired its remote root and is not promoted.

## Retirement

[`live/remote-observer-retirement.json`](live/remote-observer-retirement.json)
records the remote observer stopped; the shared remote root was removed with no
errors. The local retirement record reports private credentials retired and ten
owned services stopped; the two controller hosts were intentionally SIGKILLed by
the recovery cases. No remote keys or seed values are present in this bundle.

## Non-claims

- status object creation and signing remained local; dissemination and readback
  were distinct-host;
- the remote observer is a trusted canary identity; no hostile-node isolation
  is claimed;
- the private tailnet between two owned hosts is not WAN or hostile-network
  qualification;
- distinct-host status dissemination is not production deployment, activation
  or release authority.

## Gates

All required repository gates were run on the final source bytes; each receipt is
retained under [`gates/`](gates/). The archive move followed those gates, and
post-archive validation, readiness and traceability are recorded separately.
Configuration admission is recorded in [`config-admission.json`](config-admission.json).

| Gate | Result | Command | Receipt |
| --- | --- | --- | --- |
| python-syntax | PASS | `python3 -m py_compile scripts/radicle-ci-live-distinct-status.py scripts/radicle-ci-live-distinct-host.py scripts/radicle-ci-live.py` | `gates/python-syntax.json` |
| nickel-format | PASS | `nix develop path:. --no-write-lock-file -c nickel format --check config/radicle-ci-live-distinct-status.ncl config/radicle-ci-live-distinct-host.ncl config/fixtures/radicle-ci-live-distinct-status-inputs.ncl config/fixtures/radicle-ci-live-distinct-status-positive.ncl config/fixtures/negative-radicle-ci-live-distinct-status-local-address.ncl config/fixtures/negative-radicle-ci-live-distinct-status-missing.ncl` | `gates/nickel-format.json` |
| format | PASS | `nix develop path:. --no-write-lock-file -c cargo fmt -- --check` | `gates/format.json` |
| workspace-tests | PASS | `nix develop path:. --no-write-lock-file -c cargo test --locked --workspace` | `gates/workspace-tests.json` |
| clippy | PASS | `nix develop path:. --no-write-lock-file -c cargo clippy --locked --workspace --all-targets -- -D warnings` | `gates/clippy.json` |
| cairn-validate | PASS | `nix develop path:. --no-write-lock-file -c cairn validate --root . --policy cairn-policy/generated/cairn-policy.json` | `gates/cairn-validate.json` |
| release-readiness | PASS | `nix develop path:. --no-write-lock-file -c cairn release-readiness --root .` | `gates/release-readiness.json` |
| octet | PASS | `nix build path:.#checks.x86_64-linux.octet --no-link -L` | `gates/octet.json` |
| pijul-native-fixtures | PASS | `nix build path:.#checks.x86_64-linux.pijul-native-fixtures --no-link -L` | `gates/pijul-native-fixtures.json` |
| nix-flake-check | PASS | `nix flake check path:. --no-write-lock-file -L` | `gates/nix-flake-check.json` |
| tasks-after-completion | PASS | `nix develop path:. --no-write-lock-file -c cairn gate tasks qualify-distinct-host-radicle-status --root . --policy cairn-policy/generated/cairn-policy.json` | `gates/tasks-after-completion.json` |
| sync-plan | PASS | `nix develop path:. --no-write-lock-file -c cairn sync qualify-distinct-host-radicle-status --root . --policy cairn-policy/generated/cairn-policy.json` | `gates/sync-plan.json` |
| sync-execute | PASS | `nix develop path:. --no-write-lock-file -c cairn sync qualify-distinct-host-radicle-status --root . --policy cairn-policy/generated/cairn-policy.json --execute` | `gates/sync-execute.json` |
| traceability | PASS | `nix develop path:. --no-write-lock-file -c cairn traceability coverage --root . --policy cairn-policy/generated/cairn-policy.json --json` | `gates/traceability.json` |
| archive-execute | PASS | `nix develop path:. --no-write-lock-file -c cairn archive qualify-distinct-host-radicle-status --root . --policy cairn-policy/generated/cairn-policy.json --execute` | `gates/archive-execute.json` |
| cairn-validate-post-archive | PASS | `nix develop path:. --no-write-lock-file -c cairn validate --root . --policy cairn-policy/generated/cairn-policy.json` | `gates/cairn-validate-post-archive.json` |
| release-readiness-post-archive | PASS | `nix develop path:. --no-write-lock-file -c cairn release-readiness --root .` | `gates/release-readiness-post-archive.json` |
| traceability-post-archive | PASS | `nix develop path:. --no-write-lock-file -c cairn traceability coverage --root . --policy cairn-policy/generated/cairn-policy.json --json` | `gates/traceability-post-archive.json` |
| status-post-archive | PASS | `nix develop path:. --no-write-lock-file -c cairn status --root .` | `gates/status-post-archive.json` |
