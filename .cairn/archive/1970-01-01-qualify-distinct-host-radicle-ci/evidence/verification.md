# Verification: distinct-host Radicle publisher canary

## Verdict and scope

PASS for one distinct-host native Radicle publisher feeding the unchanged local
CI authority. The publisher node and its source worktree ran on `aspen1`; the CI
node, broker, controller, project worker, Aspen host, Lattice and independent
observer remained on `britton-desktop`. Source delivery crossed the private
tailnet directly. Status dissemination remained the separate same-host observer
boundary.

The observed window is `2026-10-21T12:36:15Z` through `12:39:48Z`. The canary
repository is `rad:z4Zax66FYJQkL9qtuxTqjbSV3tBp9` on branch `ci-canary`.

This is not WAN, hostile-network, hostile-worker confinement, production
deployment, activation or release authority. The remote host is a trusted,
operator-owned machine and is not presented as an untrusted boundary.

The raw runner report is [`verification.json`](verification.json); the complete
bundle is [`live/`](live/).

## Source binding

- Kiln main commit: `99e58831b972822dd04c5045281a7d4eb6073c8b`.
- Working-copy change: `ktvrmuzk` (the distinct-host change is uncommitted).
- The successful run used the distinct-host runner, base hooks and profile.
  After the run, requirement comments were added to the runner, the live
  admission contract was tightened to keep the base profile closed while the
  distinct profile removes only its declared `remote` field before base
  validation, and the new Nickel files were formatted. For the exercised valid
  input the admitted settings and runner behavior are identical. The recorded
  hashes are the final working-copy bytes.
- No private key, `.env`, seed or operator credential is present in this bundle.

| File | BLAKE3 |
| --- | --- |
| `scripts/radicle-ci-live-distinct-host.py` | `fb3a224b228657bdaaf0e01df0346ce1df4a28214b7683e9e5a4d1a7f8ebe650` |
| `scripts/radicle-ci-live.py` | `be3c8380c7a18e4ed5d7327ffcb6cdd850c08d536232fd35544f29fb26746654` |
| `config/radicle-ci-live-distinct-host.ncl` | `b5681addbe7eec4a7e60cb1cc177c70e200720f1153341d1582e58ed761a4255` |
| `config/radicle-ci-live.ncl` | `946769a35fab72587276504fb8e024079b06965038e4b293103fea858c49d1cc` |

## Exact repeat command

Use the same canonical `/var/tmp` path on both hosts. Both roots must not exist.
The publisher worktree and node home are copied from the local prepared canary;
only the publisher is placed remotely. SSH is orchestration only.

```console
nix develop path:$PWD -c python3 scripts/radicle-ci-live-distinct-host.py \
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
  --root "/var/tmp/kiln-radicle-live-distinct-$(date -u +%Y%m%dT%H%M%SZ)" \
  --remote-host aspen1 \
  --remote-root "<same path>" \
  --remote-address 100.100.103.95 \
  --remote-port 52000 \
  --remote-readiness-seconds 60
```

The runner admits only a canonical fresh `/var/tmp/<name>` on both hosts, a
tailnet `100.64.0.0/10` address, the declared SSH destination, a bounded port
and the pinned store executables. It refuses pre-existing remote state before
any node, source or status effect.

## Topology

| Role | Placement | Node ID | Address | Home |
| --- | --- | --- | --- | --- |
| publisher | `aspen1` (remote) | `z6MkfwNA1sKPhsLFCu9jZgRZzLpHcmq1Sj3t2j2xvaM8Pse6` | `100.100.103.95:52000` | `<root>/rad-publisher` (remote) |
| CI | `britton-desktop` (local) | `z6MkgKZ2F2GFdq3ikVpkg89jSHYWaXSEdLHHfEQdf1qESPDc` | `127.0.0.1:42123` | `<root>/rad-ci` |
| observer | `britton-desktop` (local) | `z6MkmPSgtSfKLivrH4QjW53xSu1uq1XbyQ5mDrG8ZE2rnXVv` | `127.0.0.1:56003` | `<root>/rad-observer` |

The remote node listened on `0.0.0.0:52000` and advertised
`100.100.103.95:52000`. The local CI node opened the TCP connection directly;
SSH carried only provisioning, process control and publisher command execution.
No port was forwarded.

## Transport evidence

[`transport-evidence.json`](transport-evidence.json) records the extracted node
lines:

- local CI: `Connected to z6Mkfw… (100.100.103.95:52000) (Outbound)` and
  repeated successful fetches of `rad:z4Zax66…` from that peer;
- remote publisher: `Connected to z6MkgKZ… (100.110.43.11:50598) (Inbound)` and
  repeated `Peer … fetched … from us successfully`;
- remote listener: `LISTEN 0 4096 0.0.0.0:52000 0.0.0.0:*`.

A pre-run LAN probe timed out in both directions; the private tailnet accepted
direct TCP both ways. These are two owned hosts on a private overlay, not a WAN
or hostile-network observation. The instantaneous `ss` count after delivery was
zero because the persistent connection is idle between fetches; the node
connect/fetch logs are the direct-transport evidence.

## Cases

| Case | Source revision | Canonical | Broker replies | Remote status | Operation |
| --- | --- | --- | --- | --- | --- |
| success | `6cca179cc29140928e99a8df52d46719f564b711` | `Success` | `triggered`, `finished(success)` | `success`, head `ea77e4b2f0ae0db7cd7dea0d9459012b41e774b3` | `41abe06bb5940413989cafaea117cdac75a30257803ed1885fc6cb45cd14293c` |
| failure | `0ee1e0a182ff0c82adfdb96bb0f15dcfd996030a` | `DomainFailure` | `triggered`, `finished(failure)` | `failure`, head `bc455820b4dc576cba634eba3dbf4e528d63a929` | `8af3383945a945ea8470734b8a1c8234db0b871cd89017b1dbbc97042621c4ce` |
| unavailable | `afa94e4c161020bec8ae6095147154159e319d05` | `Pending` | `triggered` only | absent | `c9dcdabc604d5699c16c61baa7788474e603e649ec741254e8149b690f3c7bae` |

Each broker `request.json` carries the exact source revision above. The retained
ingress trees contain the original request and attempt bytes. No manual
`cibtool trigger`, fixture provider or per-event workflow rewrite was used.

### Success

The local controller host was killed while the worker and Lattice continued:
the canonical outcome was `Pending`, the worker result was absent and the
original operation remained. After restart and reconciliation of the original
broker bytes, the run completed with the exact root
`/nix/store/zsisy972746hxg18cyly1sh1937ha33l-kiln-radicle-live-ci`. The status
observation was written locally before signing; the independent observer fetch
returned the same repository, revision, run, outcome, object ID and head, with
the CI node as the dedicated status signer.

After the success case the remote publisher was stopped and restarted (PID
`3880907` to `3894386`), its control socket disappeared, the local CI
reconnected, and reconciliation preserved the original operation and `Success`
without a second worker invocation.

### Source-controlled failure

The failure build genuinely exited 19 with
`source-requested-genuine-build-failure`; the retained build log is in
[`live/failure-diagnostics/build.log`](live/failure-diagnostics/build.log).
Canonical `DomainFailure` and the independently fetched `failure` status agree
at the same revision.

### Unavailable provider

The local host was stopped before dispatch. The journal retained the original
operation as `Pending`, the only broker reply was `triggered`, no status intent
or remote status existed, and no build root was produced. A real duplicate
native fetch preserved the operation and did not redispatch.

## Duplicate delivery and invocation count

The provider invocation log contains exactly the two genuine source revisions:

```text
6cca179cc29140928e99a8df52d46719f564b711
0ee1e0a182ff0c82adfdb96bb0f15dcfd996030a
```

Duplicate fetch and the remote restart did not add a third invocation. The
unavailable revision never reached a worker result.

## Status boundary

The observer is a separate local node and storage instance with its own identity.
The status signer is the CI node identity, distinct from the publisher and
observer. Exact readback succeeded for success and failure and was absent before
sign/sync. Status dissemination remained same-host; this qualification does not
claim distinct-host status dissemination.

## Configuration admission

[`config-admission.json`](config-admission.json) records two positive
compositions (same-host and distinct-host) and fifteen negatives. The base
profile remains closed; the distinct profile removes only the declared `remote`
field before base validation. Negatives cover a shell-injection host, a
non-`/var/tmp` root, a non-tailnet address, an invalid port, a missing remote
block, the existing root/branch/tool/SecretSpec cases, and unknown fields or
tools. Every negative rejects before runner effects.

## Rejected attempt

[`rejected-attempt-a/summary.json`](rejected-attempt-a/summary.json) records the
first distinct-host attempt. The remote home copy excluded the node policy
database, so the remote publisher started with the default block policy; the
local CI fetch reset and no broker ingress arrived. The attempt retired its
remote root and is not promoted. The fix was to seed the exact RID on the remote
publisher after start and after restart, and to retain remote logs and commands
before root removal.

## Retirement

[`live/remote-retirement.json`](live/remote-retirement.json) records the remote
node stopped, the remote root removed, no errors and no production state
mutation. The local retirement record reports private credentials retired and
nine owned services stopped; the two controller hosts were intentionally
SIGKILLed by the recovery cases. The remote publisher keys existed only under
the removed remote root and are not present in this bundle.

## Non-claims

- two owned hosts on a private tailnet are not WAN or hostile-network
  qualification;
- the remote publisher host is trusted and is not a hostile-worker boundary;
- status dissemination remained same-host in this qualification;
- distinct-host source delivery is not production deployment, activation or
  release authority.

## Gates

All required repository gates were run on the final source bytes; each receipt is
retained under [`gates/`](gates/). The archive move followed those gates, and
post-archive validation, readiness and traceability are recorded separately.
Configuration admission is recorded in [`config-admission.json`](config-admission.json).

| Gate | Result | Command | Receipt |
| --- | --- | --- | --- |
| python-syntax | PASS | `python3 -m py_compile scripts/radicle-ci-live-distinct-host.py scripts/radicle-ci-live.py` | `gates/python-syntax.json` |
| format | PASS | `nix develop path:. --no-write-lock-file -c cargo fmt -- --check` | `gates/format.json` |
| workspace-tests | PASS | `nix develop path:. --no-write-lock-file -c cargo test --locked --workspace` | `gates/workspace-tests.json` |
| clippy | PASS | `nix develop path:. --no-write-lock-file -c cargo clippy --locked --workspace --all-targets -- -D warnings` | `gates/clippy.json` |
| cairn-validate | PASS | `nix develop path:. --no-write-lock-file -c cairn validate --root . --policy cairn-policy/generated/cairn-policy.json` | `gates/cairn-validate.json` |
| release-readiness | PASS | `nix develop path:. --no-write-lock-file -c cairn release-readiness --root .` | `gates/release-readiness.json` |
| octet | PASS | `nix build path:.#checks.x86_64-linux.octet --no-link -L` | `gates/octet.json` |
| pijul-native-fixtures | PASS | `nix build path:.#checks.x86_64-linux.pijul-native-fixtures --no-link -L` | `gates/pijul-native-fixtures.json` |
| nix-flake-check | PASS | `nix flake check path:. --no-write-lock-file -L` | `gates/nix-flake-check.json` |
| tasks-after-completion | PASS | `nix develop path:. --no-write-lock-file -c cairn gate tasks qualify-distinct-host-radicle-ci --root . --policy cairn-policy/generated/cairn-policy.json` | `gates/tasks-after-completion.json` |
| sync-execute | PASS | `nix develop path:. --no-write-lock-file -c cairn sync qualify-distinct-host-radicle-ci --root . --policy cairn-policy/generated/cairn-policy.json --execute` | `gates/sync-execute.json` |
| traceability | PASS | `nix develop path:. --no-write-lock-file -c cairn traceability coverage --root . --policy cairn-policy/generated/cairn-policy.json --json` | `gates/traceability.json` |
| archive-execute | PASS | `nix develop path:. --no-write-lock-file -c cairn archive qualify-distinct-host-radicle-ci --root . --policy cairn-policy/generated/cairn-policy.json --execute` | `gates/archive-execute.json` |
| cairn-validate-post-archive | PASS | `nix develop path:. --no-write-lock-file -c cairn validate --root . --policy cairn-policy/generated/cairn-policy.json` | `gates/cairn-validate-post-archive.json` |
| release-readiness-post-archive | PASS | `nix develop path:. --no-write-lock-file -c cairn release-readiness --root .` | `gates/release-readiness-post-archive.json` |
| traceability-post-archive | PASS | `nix develop path:. --no-write-lock-file -c cairn traceability coverage --root . --policy cairn-policy/generated/cairn-policy.json --json` | `gates/traceability-post-archive.json` |
| status-post-archive | PASS | `nix develop path:. --no-write-lock-file -c cairn status --root .` | `gates/status-post-archive.json` |
