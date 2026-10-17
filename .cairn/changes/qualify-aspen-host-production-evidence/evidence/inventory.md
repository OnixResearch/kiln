# Exact-cohort evidence inventory

Owner: Kiln maintainers. Consumer: bounded local Aspen/Lattice CI hosting.

## Current source and scope

The source baseline is published main `2bf997aeb81b6c2047e816526a99b0a6d2847eae`, independently acquired by the Nix Git fetcher at `/nix/store/n7lsa5rsac59mrcm53gcbxbwgzxs1sav-source` (native NAR identity `sha256-hLYcGnhg/fMicvABgZVj2f9RKHB2y+GFLNW4NtnLYrs=`). Local qualification implementation/fixtures are separate from that baseline. New executable changes must receive their own source payload and executable identities; publication of the baseline cannot qualify changed provider bytes.

The host's separately locked producer cohort is Kiln `69afc2697f4330005797256135ae81b2722a125c`, Aspen `22f8ded26ca1907c29948e08b53f35df23080733`, Lattice contract `70496e67c7fd4a8b05914161a8e09de2759bebc8`, Bounded Exec `29dac88ecded94457572db3fdfaaaab95fa91525`, and Durable File Publication `8e05e74e24b45f752d77145c4455385daaf6d6ab`. These are not replaced by root-workspace imports.

No historical receipt supplies the complete current source/executable/runtime-profile/provider-profile/host/publication binding. The historical cases below supply scenarios and limited observations, not reusable current-cohort PASS results. Fresh execution must retain actual artifact hashes, commands and bounds.

## Frozen qualification candidate

The current executable-input candidate is `05ecd14be3e43067a1dc5ebc21b9a27e21d2b297`. Its normalized GNU TAR is 14,714,880 bytes with BLAKE3 `fca77390881731154dae79f179faa6cfa51d08fb626fc2ce35cac112cd07f044`; the [complete regular-file manifest](artifacts/539df6323201fc0f0669641650756ca4715e3b3cf9e1d0071e6abd9124baed59) binds all 1,183 file members. `.jj/` and `.git/` are excluded. Later qualification receipts, review/task completion and publication observations are evidence-only descendants, not retroactive additions to this tested payload.

The preservation artifact records unchanged bodies for all 29 accepted specifications and all 13 lifecycle-policy inputs against the published baseline. The isolated Nix store admits this exact source without deleting or mutating the unrelated system-daemon import lock; required checks retain sandboxing, cgroups, locked producer inputs and their original policies.

## Historical observations and fresh obligations

| Case | Retained historical observation | Current qualification obligation |
|---|---|---|
| Durability/restart | The archived real-process fixture accepted one trigger, stopped Lattice and restarted the host, retaining byte-equal terminal acknowledgement replay. | Actual child-host restart plus process loss at pending/uncertain boundaries, without redispatch. |
| Corruption | An eight-byte-truncated copied SQLite ledger exited 1 before socket creation; other corruption cases are summarized rather than retained per-case. | Actual startup rejection of missing/partial/corrupt marker, ledger, journal and callback state. |
| Provider teardown | Provider receipt describes timeout/output tests but retains no named-bound descendant observations. | Actual provider binary, exact wrapper/profile and PID/start-time observations for success, timeout, output bounds and cancellation. |
| Finite load | Sixteen repeats of one completed request with Lattice absent; not sixteen independent effects or a capacity/soak result. | Named finite multi-operation workload, exact outcomes and dispatch/replay counts; retain the limited workload scope. |
| Backup/restore | Full root restored and acknowledgement equal, with historical state fingerprints. | Quiescent complete-state backup/restore plus partial/mismatched restore rejection for the tested executable/profile cohort. |
| Explicit rollback | Checked tasks and narrative assertions, without an executed old/new release pair or post-rollback observation. | Execute an explicit recorded executable/profile/state transition, reject incompatible transition and observe successful restoration; do not relabel generic copy as release rollback. |
| Authority | Narrow repository/user/wrapper/environment admission tests, not deployment authorization. | Exact profile/provider/source and credential/environment boundaries plus rejection of stronger receipt claims; no deployment permission follows. |
| Publication binding | Older provider/host revisions and changing store paths, without a complete final host/profile/publication binding. | Retain exact source and executable identities and explicit publication/candidate status, never claim unpublished changes are the published baseline. |

## Evidence that must remain historical

- `.cairn/archive/2026-08-30-productionize-aspen-radicle-host/evidence/real-process.md` records the actual earlier one-operation replay/restore/truncated-ledger sequence. Its first/restored acknowledgement BLAKE3 is `e48c770ed618a7e2d99bbd13497235993c3b20d9bb74f7e40170a754987d9f66`. This is not a current cohort identifier.
- That archive's `durable-host-verification.md` initially names Kiln `9ad8447f8025331917cf5c40ad956a8df40d225a`; later verification repins `69afc269...`. These cannot be merged into one cohort by copying PASS declarations.
- `.cairn/changes/pace-aspen-lattice-observation/evidence/verification.md` preserves the actual unpaced 1,023-poll exhaustion and later provider failure. Its local pacing repair does not establish the still-pending provider-backed live shadow.
- `.cairn/changes/bind-aspen-extension-runtime-profile/evidence/verification.md` preserves the deployed-versus-fixture timeout/profile mismatch and reserved operation with no provider acceptance. The later local repair does not establish a successful live shadow.
- The historical release-evidence rendering check hashes and compares declared inputs; it is not a current production qualification evaluator.

## Claim and execution boundary

Native child-process host fixtures may use the existing closed Lattice terminal handler only to establish scoped local transport/host behavior. Actual Lattice process evidence must be identified separately. Synthetic provider commands establish process-control and report behavior, not real Nix or CI correctness. A terminated but unreaped non-child descendant must not be reported as absent; direct-child reaping and descendant inability to execute are separate observations.

All fresh work uses isolated temporary roots and explicit finite bounds. No existing deployed Onix service is changed, no source-only exception becomes a production waiver, and no global durability, distributed exactly-once delivery, provider truth, sandboxing, availability or release/deployment authority is inferred.
