# Issue #28 acceptance and exercised boundary

## Implemented acceptance

- Host receipts: `vm.host` and direct `vm` fields retain kernel/package/status, SMT control/active, KSM raw/run boolean, cmdline, vulnerability entries, resctrl mount and CAT/MBA capability maps, MPTCP/tcp_migrate_req sysctls and bounded `ss -M -l -n -p -H` listener count. Unavailable observations stay explicit.
- Admission: every dispatch on an untrusted-capable Firecracker runner checks host status, not only requests with untrusted provenance. Unknown/vulnerable issue-policy kernel status cannot be overridden; SMT/KSM require an explicit retained override. Partial MPTCP exposure observations are not a kernel fix. Pure regressions cover fixed, vulnerable, vendor-backported, unparseable, SMT/KSM override and residual/unknown listener cases.
- Placement: existing admission transaction reserves the actual local machine identity independently of configured aliases. Cross-attempt/organization untrusted versus workload-policy overlap refuses before finance/process effects. Stale cleanup is organization-and-attempt bound; unknown attempts remain reserved. Existing cross-attempt refusal regression now uses actual host identity.
- Actual overlap: a separate running-VMM ledger records symmetric peer organization/attempt/job identities after live confinement observation and until cleanup. Every candidate peer must still have its recorded live PID and Linux process start-time; exited/zombie peers awaiting final journaling or finish are excluded. Persistent overlap rows survive a peer completing. Regressions cover overlapping attempts, sequential peers and process-exit-before-finish. Boot-only activity before observation, external services and unmanaged host processes are outside this evidence boundary; reservation safety spans the larger whole attempt.
- Optional controls: taskset affinity is inherited by VMM/vCPU threads. Existing operator-created CAT groups are assigned and checked before guest job dispatch, with schemata and actual thread membership retained. No false LLC partition claim follows from labels; operators own mask exclusivity and permission delegation. MBA never claims full bandwidth partitioning.
- Measurements: opt-in static guest sender/receiver and networkless/diskless Firecracker worker, explicit local/SSH argv ports, three workloads and three reference placement modes. Receiver gets no sender payload; 32 training symbols select the threshold and 32 held-out symbols measured errors in the retained reference run. Regressions reject SMT siblings, fake distinct pool labels sharing an LLC and malformed training; distinct OS-instance labels explicitly do not establish physical separation.
- Documentation names cache-eviction, CPU-contention and DRAM-bandwidth channels and preserves `hostile_isolation_claim=false`.
- Guest fixed-series gate/pin: coordinated with UnikernelRunner; guest-only Nix input was tool-resolved to revision `1d060086d60cb76a40cf37109f979d88a5bd6396`, observed kernel 6.18.60. Flake implementation/lock owned by that worker, not this package.

## Exercised verification (not unrun claims)

Proposal/design/tasks gates passed before source implementation. Initial gate attempts required delta-spec and uppercase WHEN/THEN corrections; final preimplementation design/tasks gates passed.

Native runtime commands:

```console
nix shell nixpkgs#pkgsStatic.stdenv.cc --command python3 scripts/microvm-channels.py build --cc x86_64-unknown-linux-musl-cc --output /tmp/kiln-channel-probe-drained-28
python3 scripts/microvm-channels.py run --config /tmp/kiln-channel-placement-final-28.json --output /tmp/kiln-channel-observations-drained-28.json --symbols 32 --symbol-ms 50 --lead-ms 3000
```

Observed 9 completed real guest runs. Complete raw samples, serial, configured endpoints, observed topology, binaries/source BLAKE3 and qualification boundaries are retained in `reference-channel-observations.json`. Isolated reducer smoke recomputed all 9 error rates from that evidence and rejected two different pool labels on the same observed LLC. The default `cc -static` lacked libc; the explicit musl toolchain built successfully. Early runtime spikes exposed mandatory Firecracker `drives: []`, unavailable poweroff, Firecracker 1.15.1 guest-reboot shutdown exit behavior, and pending serial output at reboot; the final probe uses one complete report write plus `tcdrain` before reboot. Final measurements completed, rather than treating truncated output or timeout as success.

All raw offered symbol rates were 20 symbols/s. Error rates by placement and cache/CPU/memory order:

- scheduler same-host: 0.40625 / 0.53125 / 0.125;
- pinned distinct cores 0/1, same LLC: 0.53125 / 0.40625 / 0.46875;
- explicit pools llc-a/llc-b pinned 0/8, distinct LLC: 0.5625 / 0.53125 / 0.4375.

These short observations do not establish channel capacity, statistical absence, hostile isolation or safe production multitenancy. Source/secret confidentiality is not claimed.

## Exact external qualification boundaries

Production admission chooses whole-host exclusion. The separate-pool measurement instead explicitly provisions two distinct LLC pools on the same reference machine (LLCs `0-7,16-23` and `8-15,24-31`), verifies their topology and marks the result measurement-only. This is not silently substituted separate-core measurement and does not assert physical-host separation. Actual cross-physical-host qualification requires a second independently provisioned physical machine with the probe image and synchronized clocks; none was provided.

Host 6.18.50 is `vulnerable` by issue #28's supplied threshold 6.18.56, not an independently validated advisory claim. Booted package is `/nix/store/9xdswfnj5dv4al2cd51f3nlvsh78s5vv-linux-6.18.50/bzImage`. SMT was on, KSM off; no host knobs changed. A patched/rebooted host is required for real untrusted-capable execution. The trusted measurement guest used existing 6.18.42, independent of the new guest pin.

CAT assignment runtime qualification requires a mounted resctrl filesystem, supported L3 CAT hardware, an operator-created exclusive CAT group and write delegation. The reference `/sys/fs/resctrl` was empty; no assignment success is fabricated. The implementation fails guest dispatch when any of those prerequisites is absent.

## Parent-owned integrated verification commands

No Rust build/test/lint/formatter or project-wide checks were run mid-flight. Parent runs once after integration:

```console
cargo test --locked -p kiln-microvm host::tests
cargo test --locked -p kiln-hosted --lib
python3 -m unittest scripts/test_microvm_channels.py
cargo test --locked -p kiln-hosted --test microvm -- --nocapture
```

Use the repository's existing microVM integration environment/opt-in variables for the final command; it now asserts all required host receipt keys and real empty co-residency for its sequential plan. Nickel positive/negative exports and the guest fixed-series flake check are integrated parent gates. Passing these commands is not asserted by this worker.

## Review corrections and follow-on exercised smoke

All five HostPlacement review findings were addressed: live PID/start-time peer checks close the exit-before-finish false-observation window; worker bounds derive from the full admitted schedule and own process groups through timeout/cancellation; CAT mount detection requires both `/sys/fs/resctrl` and filesystem type `resctrl`; raw machine IDs are absent from customer VM receipts; measurement identities are private report-keyed OS-instance pseudonyms, never hardware attestations. Native receipt removal and the new `placement::begin(..., pid)` call were coordinated with UnikernelRunner.

Historical `reference-channel-observations.json` is explicitly privacy-redacted: raw machine IDs became per-evidence `REDACTED` OS-instance labels preserving equality only. Samples, serial, error rates and historical cohort hashes are unchanged. Corrected classifications state observed same-instance/disjoint-LLC scope, with no inferred cross-physical-host proof. This is a documented redaction, not a new measurement or unmodified raw-identity claim.

After corrections, an isolated native smoke exercised the maximum admitted schedule calculation (1,096 seconds including guest-finish margin), launched real owned subprocesses and observed them reaped after both TimeoutExpired and KeyboardInterrupt, replayed historical placement classifications with redacted labels, then ran a fresh real sender/receiver pair on pinned cores 0/1. The receiver observed CPU work samples `[9073, 9053, 9021, 9039]`; both corrected worker results omitted raw `physical_host` fields and reported only same observed OS instance / physical placement unverified. This four-symbol smoke verifies the changed worker lifecycle/privacy path, not a new bit-error qualification. Permanent regressions were supplied for final parent verification, not claimed run here.

## Parent integrated results

The shared locked workspace and native-feature command set passed (1,580 pass
occurrences across 162 summaries, including repeated packages; five ignored).
After the final native preboot and teardown changes, the hosted and microVM
library suites passed again: 94 hosted plus 42 microVM tests, one operator-only
smoke ignored. The six affected production libraries passed Clippy with
`-D warnings`. The six channel-script regressions passed, and all nine retained
channel rates and placements were recomputed with the current reducer.

Final real Firecracker cold/warm cache journeys passed on guest kernel 6.18.60.
Eight native hosted journeys retained current host facts and strict Seccomp 2
observations; three actual preboot policy mutations refused before application
start and cleaned their owning units. These trusted-only runs do not qualify
untrusted execution on the still-vulnerable host, mounted/delegated CAT, or
cross-physical-host placement. The external boundaries above remain unchanged.

