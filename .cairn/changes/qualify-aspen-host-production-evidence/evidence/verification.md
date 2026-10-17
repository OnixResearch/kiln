# Exact-cohort local host qualification

Owner: Kiln maintainers. Consumer: the bounded Aspen/Lattice CI host.

## Exact inputs

The [cohort](cohort.json) identifies unpublished input payload `fa826a0545692ceb9f05860c699e00a8736dbef7d2a0d0045192e5484ad11a82`, a 23,224,320-byte TAR snapshot of the raw Nix path input at `/nix/store/qs6ys6z64y518qizxhc5xwsrl9h0qss1-source`. The [source manifest](source-manifest.json) records all 4,343 file members, including archive hardlinks. This is an input snapshot, not a published Git revision or the normalized release-archive format. Later receipt/task/documentation files are evidence-only additions; they are not retroactively included in the tested payload.

The raw snapshot includes 2,730 VCS bookkeeping files and is not a source-publication archive. The product-input comparison excludes changing `.jj/` and `.git/` state and checks all 1,613 original product files. Only the declared task/operator-documentation files and the current semantic plan-review binding changed after freeze; compiled code, manifests and runtime profiles did not. The TAR remains in the temporary qualification root, not in this evidence package.

The frozen host, extension fixture and provider executable identities, actual primary profile identities, host/input binding and explicit unpublished-publication binding are in the cohort. The host/input artifact enumerates all 13 exact native runtime/collaboration profile variants. Native negative variants and their closed Lattice transport fixtures remain distinct from the separately recorded real Lattice/Nix profile. These observations qualify only their declared finite cases, not an operator deployment.

All 29 accepted spec files and 13 lifecycle policy files are byte-identical to baseline `2bf997aeb81b6c2047e816526a99b0a6d2847eae`; the preservation artifact retains both identities for each file. Historical publication receipts and pending provider-backed live shadow tasks remain unchanged.

## Executed observations

| Case | Observed boundary | Retained case |
|---|---|---|
| `durability` | Full acknowledgements preserved after SIGKILL; actual Lattice stopped before duplicate and restart. | [exact inputs and artifacts](artifacts/df5f72f973590892aca20977c3650e630ce95d4fb89c01198e802fc3d1c6e7df) |
| `uncertain_recovery` | One original dispatch; recoverable uncertainty resolved through RecoverDispatch and Observe without dispatch; unavailable recovery retained Unknown with six recovery attempts. Earlier quarantined actual state failed closed and is not a successful recovery. | [exact inputs and artifacts](artifacts/1db31ac220720b5f36d08f3b6c34665b6d6c23cfed316598fd3407719268f85c) |
| `corruption` | Malformed marker, truncated ledger, corrupt native journal, corrupt callback value and missing callback directory rejected before ingress. | [exact inputs and artifacts](artifacts/9c91f1e0f187b87849110242bb9ce861f7eae9eafff4fc64ea003f215acdce45) |
| `provider_teardown` | Success, timeout, stdout flood, stderr flood and SIGTERM cancellation tests passed; retained PID/group/start-time observations report all original processes absent (None), not zombie-as-absence. | [exact inputs and artifacts](artifacts/942d45b47ec8da405562c41cfcfcc33541df00e2ef0b1b043b52c1fff442e399) |
| `finite_load` | Every submit/duplicate/reconcile and post-restart acknowledgement matched; exactly four dispatches, four observations and zero recovery dispatches. | [exact inputs and artifacts](artifacts/3890ff1843ca00f3bbfd412c899cfbf032e71e2688273e2b443f80512cc601f7) |
| `backup_restore` | Complete four-component state and sidecars copied only after host exit, byte identities retained, original acknowledgement replayed. Missing-journal partial restore rejected before ingress and repaired from the same backup. | [exact inputs and artifacts](artifacts/7d304d0278ae2803f6ca3e92f5733588d9503634253acb9520d0b85581107438) |
| `rollback` | Mixed profile/executable and old-profile-on-candidate-state transitions rejected; separately admitted candidate ran against fresh state; explicit prior executable/profile/quiescent-state restoration replayed the original full acknowledgement. | [exact inputs and artifacts](artifacts/261cbbd4809b9990775d122872e887105ddfaaa1f1ca49840ff1efe30e3ace28) |
| `authority` | Profile/executable/source/environment admission and bounded receipt rejects exercised. Synthetic format smoke is separately labeled, not native qualification evidence. | [exact inputs and artifacts](artifacts/aeb198bcdebe422f74ecb3eddb9ad48f9efc19c21e367665c93102b0e3da55b9) |
| `publication` | Source payload, manifest and exact executable/profile bytes bound; 29 accepted spec files and all 13 policy files unchanged from published baseline. No new publication, signing, push or deployment authority. | [exact inputs and artifacts](artifacts/5379fbb744165d64fcba77ce8f8f48282fa38fba5bd32ce458e8e6c1f2407706) |

The prescribed root toolchain passed 410 workspace tests (two opt-in live tests ignored), strict Clippy and the three-crate `wasm32-unknown-unknown` check. The separately pinned native runtime environment passed 26 host tests, nine provider tests and strict host/provider Clippy. Full commands and outputs are retained, not reconstructed PASS declarations. Native host observations include 28 actual child launches, 38 acknowledgements, six explicit Unknown results and 13 quiescent state snapshots.

The separately identified real Aspen host → real Lattice → Nix path produced an `outcome: success`, exit-code-0 report for local source fixture `a9e67a14178c83654c4bf932bfc84c160ecd749d`. With Lattice stopped, duplicate ingress and post-SIGKILL restart/reconciliation returned the same complete acknowledgement. This is not Nix/CI correctness, live Radicle operation or external status publication.

## Gate and consumer records

[Artifact index](artifact-index.json) maps logical names to exact retained BLAKE3 bytes and sizes, including gate logs, profile bytes, execution cases and consumer results. [Current native Cairn validation and proposal/design/tasks gates](artifacts/4a5a1b7244b4de6a04bb2e70361188bd47a10a617dd0f0adb507bc592276551c) report `valid: true`; stage verdicts are `PASS`. Declared traceability is 625/625 with no missing or dangling IDs; its assurance remains declared linkage, not execution truth. The accepted requirements and policy were not weakened.

The [incomplete receipt](receipt-incomplete.json) preserves an earlier point at which Octet, Nickel and Nix broad results had not been observed. The [current blocked receipt](receipt-blocked.json) records the passing Nickel observations and corrected current lifecycle result, while Nix and Octet remain `unknown`. The prescribed-toolchain CLI rejects both with `qualification_gate_not_passed`; no complete passing qualification is claimed.

### Selected Nix/Octet gates blocked

The explicit same-RID Bounded Exec cache seed matched the documented revision and NAR identity `sha256-BVmqyUYyoNpY6LfOABxwPO3DY88ZtXeKNg3TPoGCcL0=`; product URLs and pins did not change. Host-package evaluation nevertheless queued behind the root-owned Nix daemon worker `750625`, which held `/nix/store/qwsa00lfmyc1kb015pxx03wkc9zxs5hh-build-cohort.lock` (device 31, inode 2198957); the observed waiter was daemon worker `3688438`. Daemon info and a small owned-file store publication succeeded. This is an exact source-import lock blocker, not evidence that the package or gate passed. The [retained diagnosis](artifacts/577a96f81dc9dc82d596e6f74df605e1a362fcdf3222fe476531f430b25de3ed) binds the actual invocation and observations.

Passwordless sudo was unavailable. Only the owned blocked gate was cancelled; the unattributed root process and its lock were not modified. The installed native Octet source has a [different producer identity](artifacts/773d2df5bf2f4fa47725b51a7a53d4d72b22fba43cab37ef449466190be40a91) and was not substituted for the required exact producer. An operator must establish the lock holder's current ownership/state and safely resolve the blocked import before rerunning the unchanged required Nix/Octet gates. Preserve the interruption and all later failures; do not unlink the lock blindly, weaken policy, or promote the current receipt to qualified.

Earlier lifecycle logs incorrectly treated machine-command success as a passing verdict: validation and the tasks gate actually reported `valid: false` because checking completed tasks had made the semantic review's task hash stale. The [correction](artifacts/0ca0c214ec60e4d9f8122f50eb1ec2250c320bba79224c1d2f8d12fe25f84234) retains those outputs. A genuine current rereview preserved the no-spec classification and refreshed only the exact task binding; the linked current native verdicts then passed. The original incomplete receipt remains historical, not current lifecycle qualification.

## Retained failures and non-claims

Earlier native build-environment/test failures remain in the artifact index. An offline development-shell invocation timed out after 1,800 seconds; the delivered timeout is retained without fabricating its unretained full command. The successful prescribed-toolchain root checks are separate observations.

Early actual-provider attempts exposed missing device access, a read-only Nix cache and an absent report namespace. Their failures remain retained. A minimal shell workflow can report `completed` while its provider report/exit output is failure; qualification checks the separately retained provider outcome rather than treating that acknowledgement as CI success.

The earlier state root containing failed attempts rejected restart from the native Quarantined phase after SIGKILL. It was not repaired or relabeled successful. The successful actual-provider crash/replay observation uses its separately identified clean state root. The recoverable and persistently Unknown cases belong to the explicitly closed-transport native drills. No generic recovery from quarantine is claimed.

Receipt admission checks supplied consistency and retained bytes, not observation truth, global durability, storage-device persistence, exactly-once delivery, CI correctness, provider truth, host sandboxing or availability. It grants no credential, deployment, release, publication or governance authority. No remote mutation or lifecycle sync/archive was performed.

## Repeatability

The permanent native drills live in `runtime/kiln-aspen-host/tests/hosted/qualification.rs` and `runtime/kiln-radicle-nix-provider/tests/process_teardown.rs`; pure qualification admission/rejection lives in `crates/kiln-core/src/hosting/qualification/`. Run the host/provider gates documented in their READMEs and the root checks in `docs/operator.md`. Nix package checks retain the native JSON observations with `--nocapture`.

For a fresh qualification, rebuild and record the actual source/executable/profile/host identities and exact case inputs, execute the named cases and all required gates, retain every failure and bounded artifact, and run `kiln host-qualify` against the reviewed expected cohort. Changed inputs require a new cohort and observations; neither this receipt nor historical publication transfers qualification.
