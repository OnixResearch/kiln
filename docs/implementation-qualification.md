# Implementation qualification

This is the bounded repeatability rail for [Kiln #50](https://github.com/OnixResearch/kiln/issues/50), composed with the separately owned [onix-core #49 deployment](https://github.com/OnixResearch/kiln/issues/49). The consumer is `scripts/implementation-qualification.py`; its maintained input contract is `config/implementation-qualification.ncl`. The implementation coordinator/CLI owns admission, candidate capture, verification and publication semantics; Lattice owns the actual OMP process and durable attempt result; onix-core owns accounts, namespaces and packaging. This harness owns neither a scheduler nor acceptance authority.

**The runner is implemented, not live-worker qualified.** Native proposal/design/tasks gates passed for `.cairn/changes/qualify-implementation-composition`; actual worker and remote qualification still require the inputs below. A runnable script, structural fixture or passing acceptance-control test is not evidence of a live worker, verified PR, landing, source closure, hostile isolation or production readiness.

## Smallest safe command

From the checkout, with an existing owned scratch parent:

```console
python3 scripts/implementation-qualification.py prerequisites --scope campaign \
  --output /run/user/1555/kiln-implementation-qualification-preflight
```

This deliberately exits **2** when no adopted cohort/authority/provider inputs were supplied. It writes `preflight.json` and `receipt.json` naming missing inputs; it does not build, launch a worker, dispatch, push, open a PR, land, close a source, activate a service or change host policy. Existing output directories are refused. Choose a new name for every execution, including failed executions. Prerequisite success means only that the requested input cohort is present, never that any runtime scenario passed.

After the parent builds the changed coordinator and selects its actual immutable executable and BLAKE3, export an operator-owned Nickel file applying `fixtures/implementation-qualification/local-admission.ncl` to that `{ path, blake3 }` record. Export with the selected immutable Nickel executable (`nickel export --format json FILE.ncl`) and retain the JSON outside the checkout. Then:

```console
python3 scripts/implementation-qualification.py local-admission \
  --inputs /run/user/1555/kiln-qualification-inputs.json \
  --output /run/user/1555/kiln-implementation-local-admission
```

This creates only a disposable SQLite ledger, authenticated **scripted** GitHub deliveries and a random local-only webhook key. It invokes the actual coordinator CLI for duplicate/out-of-order intake, concurrent claim competition, restart/read-back, lease expiry, stale renewal and source supersession. The queue uses the existing `implementation_deliveries` wire storage, never writes authoritative `implementation_state` rows, and snapshots that ledger read-only after native operations. It does not run a worker or synthesize candidate/CI/provider facts. Its result is always `partial_local_admission_only`, even if every local assertion passes. The expiry case is an **undispatched claim**; it does not prove that an expired executing worker stopped.

The narrow parent-run scripted controls are:

```console
python3 -B -m unittest discover -s scripts -p test_implementation_qualification.py
```

Those controls prove that the frozen buggy source fails and a clearly labeled scripted corrected control passes the identical reproducer, and similarly exercise bounded feature acceptance. They do not constitute model-produced candidate or CI evidence. No build, test, formatter or runtime campaign was executed by the implementation worker adding this harness.

The parent subsequently ran the six existing `kiln-implementer` native verifier
fixtures with `--lib verification:: -- --ignored`, an explicitly private
`KILN_VERIFICATION_TEST_ROOT`, and exact Nix-store Git/shell inputs: all six
passed. These fixtures execute real frozen commands and Git acquisition, but
their candidates are scripted and their worker UID is a fixture sentinel. They
do not qualify an OMP worker, separate-UID deployment, model route or publication.

## Actual immutable prerequisites

The retained parent preflight `/run/user/1555/kiln-implementation-qualification-preflight-20261114-parent` remains untouched and unqualified. Its `input-configuration.json` has an empty `programs` map. Its recorded missing program entries are `b3sum`, `cairn_intake`, `coordinator`, `git`, `lattice`, `lattice_dispatch`, `omp`, `setpriv` and `verifier`; no campaign executable cohort was supplied. Local admission needs only newly built/reviewed `coordinator` plus `b3sum`. Campaign also needs the actual canonical verifier (`kiln-web-service`), Lattice, Cairn intake, strong Kiln Lattice dispatch, pinned `setpriv`, the adopted OMP executable and Git. Existing similarly named binaries do not stand in for rebuilt source.

The same preflight records absent retained `{ path, blake3 }` inputs for `cohort`, `source_baseline`, `policy`, `toolchain` and `provider`; absent integer `coordinator_uid/gid`, `worker_uid/gid`, `verifier_uid/gid`; no distinct existing coordinator/worker/verifier UIDs; and no exact `scenarios` bundles. The observed Python runtime is `/nix/store/xahxrjs30p3mw0a42bqg9ikgkikskw8m-python3-3.14.8/bin/python3.14`; this is the harness interpreter, not the missing protected acceptance toolchain/cohort. The host's observed b3sum, Git and Cairn paths below remain observations only, not adopted inputs. The absent provider document must bind the actually authorized local execution/model route, endpoint or authless namespace, readiness and resource grant; no model service, credentials or provider authority was inferred. The preflight made no user/group existence or isolation claim.

The runner snapshots each accepted program and cohort/source/policy/toolchain/provider file identity, then rechecks those identities before every stage. It likewise freezes protocol templates and canonical CI profiles. A changed/disappeared input blocks the campaign; a receipt from an earlier preflight is not approval or proof that later stage execution is safe.

The following paths and hashes were observed on the implementation host, not adopted as an implementation-runtime cohort. They are also recorded in `fixtures/implementation-qualification/observed-tools.ncl`:

| Tool | Immutable path | BLAKE3 |
| --- | --- | --- |
| b3sum | `/nix/store/0by3mqkn4gk970s8h0ns546jcm8hfl7w-b3sum-1.8.7/bin/b3sum` | `827e1a120331e82e9360d564b7decb920066594600db140e19820aae4c30d9ba` |
| Git | `/nix/store/xgw813vgzl2q0696sq45adpj0xd690jq-git-2.56.0/bin/git` | `49e13267501dd1cc7e8e72bdb9acb9a3ce2808c446a89f820aa815c2e35275d5` |
| Python | `/nix/store/xahxrjs30p3mw0a42bqg9ikgkikskw8m-python3-3.14.8/bin/python3.14` | `42bd94105d205eb80f3599889947003f3ba415bbf09860b4ecd6ee72f2a34bbc` |
| Cairn | `/nix/store/x37qabfj71nzk5661f3x735nni4m96qk-cairn-0.1.0/bin/cairn` | `99f643947a209d75268db31d05a2713d41bc3f69dd0c3400485ad10eeab6d69a` |

The selected root nixpkgs lock records `346dd96ad74dc4457a9db9de4f4f57dab2e5731d` and `sha256-OuZZ71OA/6fZePPqgVinI4QVu9j1QCt+Q9i5ZgmSGqI=`. This is not a claim that all changed products were built from that pin. Lattice's explicit OMP adapter requires OMP revision `e325ffc73ab86e03bdd20a91b77d8e2b3964c906`; selecting another OMP revision is not this profile.

Available source contracts:

- `crates/kiln-adapter-github/src/bin/implementer.rs`: bounded coordinator admission, metadata preparation and actual candidate capture. Its old `verify` command is removed.
- `crates/kiln-web-service/src/engine/verification.rs`: the canonical `Engine::verify_implementation_candidate` consumer; native check execution, exact node mapping, Cairn prerequisites and durable CI journal precede typed candidate acceptance. No shadow success store or deserialized report can substitute for it.
- `crates/kiln-implementer/src/verification.rs`: exact frozen profile and `VerifierAuthority`; no worker-supplied success report can construct a trusted `VerificationRun`.
- Lattice `src/implementation_attempt/cli.rs`: strict `lattice.implementation-attempt-run.v3`; native `prepare --request SEED --output FROZEN` derives workspace snapshot, canonical worker-profile/envelope identities and exact request-byte BLAKE3 without execution/admission. The real `run|recover --request FILE --state DIR` consumer requires exact source/profile, expiry, bounded paths, immutable worker/sandbox, `network:false` with only the attempt-local model broker route, and explicit OMP. Its direct route is executor diagnostics, not a full Kiln pipeline. Recovery reads retained intent/results; incomplete state is **unknown**, not rerun authority, and recovery without an intent closes the attempt (`not_started_closed_by_recovery`).
- `crates/kiln-adapter-lattice/src/worker.rs`: the strong `kiln-lattice-dispatch --worker-request FROZEN --worker-transport-profile FILE --dispatch-plan PLAN --database DB --enrollment FILE --admission-observations FRESH` route revalidates current work/attempt/fence, exact prepared request/profile and its bound `dispatch_plan`, attached needs and configured owner-grant admission before real Lattice execution. It retains native transport observations and records the actual `lattice/implementation-attempt/<request-byte-BLAKE3>` execution. Recovery reuses that transport intent instead of constructing a workflow identity in the harness. A typed Lattice refusal with `dispatched:false` or a closed-before-execution recovery ends the attempt without an execution; every other refusal stays for reconciliation (see the [runbook](implementation-runbook.md)).
- onix-core `modules/kiln-implementer/cohort.nix`: actual dereferenced executable/workflow identities and explicit source revisions/unpublished-source archives; `configuration.nix`: typed policy/runtime exports; `nixos.nix`: opt-in, operator-started stages. Default distinct UIDs are coordinator 930, executor 931, publisher 932, verifier 933; runtime name is `kiln-implementation`. Those defaults are configuration values, **not evidence those accounts or services exist on this host**.

### Truly external operator inputs

Campaign preflight requires retained `{ path, blake3 }` documents for `cohort`, `source_baseline`, `policy`, `toolchain` and `provider`. They must describe the actually selected source/patch/archive cohort, frozen exact Git baseline, verification/acceptance definitions, runtime closure, scoped execution/model route and resource limits. A branch name, executable name, configured model, free device, cached model or inferred service health cannot replace any of these facts.

`cohort.path` must resolve to the immutable onix-core `manifest.json` (`onix.kiln.implementation-cohort.v1`), with `workerAdapter = "omp"` and the exact OMP revision above. The runner matches coordinator/Lattice/OMP/intake/dispatch paths and hashes against its actual `entries`, not executable names. `source_baseline` is retained JSON with a `cases` record: its `bug` and `feature` records each contain the enrolled `repository` and exact Git `base` commit. Successful trusted verification must bind those exact values. The worker request must select the same actual OMP executable/hash/revision as the adopted cohort.

Also required are existing `coordinator_uid`, `coordinator_gid`, `worker_uid`, `worker_gid`, `verifier_uid`, `verifier_gid`, actual separate-UID launch authority, trusted Git acquisition/scratch and exact scenario bundles. The current implementation user is UID 1555/GID 998; that is not authority to assume a system UID. The runner never calls `sudo`, creates users, changes group membership, grants credentials, configures namespaces or starts services. An explicitly admitted authorized-root launch uses pinned `setpriv --reuid UID --regid GID --clear-groups -- PROGRAM ...`; otherwise it fails before launching that role. The worker role is the selected trusted executor/dispatcher shell: its operator-qualified process-local ledger group and sandbox are provider prerequisites, not blanket ledger access for an untrusted coding child. The verifier remains a different actual effective UID. The runner does not add a supplementary ledger group or silently widen account permissions to make an unqualified transport work.

Verifier-owned acquisition/scratch must be canonical Linux directories inaccessible to worker mutation, with no worker-owned or group/world-writable ancestors; scratch must be outside the repository. A writable shared worktree, devshell, UID label or per-directory naming convention is not isolation. The actual Lattice worker requires its configured sandbox, no publisher credentials, and a separately admitted model route. The model endpoint/scoped authorization or operator-qualified authless namespace, physical host/service readiness and any provider resource grant are external facts; repository inspection cannot supply them. Never put credentials in Nickel/JSON inputs, retained argv, environment or receipts.

## Frozen bounded work

`fixtures/implementation-qualification/bug/` contains a real failing `clamp` implementation and a protected reproducer with inclusive bounds and reversed-bound cases. `feature/` contains a missing `normalize_label` implementation and explicit whitespace, Unicode, empty, punctuation and non-string cases. `requirements.txt` and `acceptance.py` are protected; only the named `src/` implementation file is writable. There is no baked model-produced candidate. Acquire each into a disposable actual Git repository and record the baseline commit/tree before admission; the real OMP worker must produce its candidate under the selected boundary. The existing trusted candidate-capture consumer must derive its exact Git commit/tree from the actual bounded workspace and Lattice host receipt. Do not hand-author a successful handoff or pretend a snapshot digest is a Git commit.

The frozen verification profile selects immutable Python with `-I -B acceptance.py`, requires `artifacts/acceptance.json`, protects both frozen files and covers every obligation. `bug` selects reproducer `acceptance`, running identical bytes/argv on failing baseline and passing candidate; `feature` has no baseline reproducer and requires explicit candidate acceptance. `fixtures/implementation-qualification/verification-profile.ncl` constructs the native preparation seed, not admitted evidence. Actual verification is exclusively `kiln-web-service verify-implementation --profile CI_SERVICE_PROFILE --database DB --enrollment ENROLL --attempt ID --fence N --verification-profile FROZEN --authority AUTHORITY`. The native canonical CI service profile must include the exact verification nodes and existing Cairn prerequisite/operation mapping. Each `verify` step requires `canonical_profile = { path, blake3 }` and its `--profile` must match; missing or weakened native mapping fails closed. The command executes through Engine and prints admitted typed `CandidateVerification`, never a caller-supplied raw report. The retired `kiln-implementer verify` and `--verification-file` interfaces are not alternatives.

`prepare-verification-profile --profile SEED --output FROZEN` computes the native derived environment/toolchain and profile identities, reporting `checks_executed: false`. After an actual worker result, `capture-candidate --database DB --enrollment FILE --attempt ID --fence N --dispatch-plan PLAN --profile FROZEN --authority AUTHORITY --candidate-root ACTUAL_WORKSPACE --host-receipt NATIVE_RECEIPT --handoff-output NEW_FILE` independently materializes the real Git candidate/tree and admits the resulting handoff. It never treats preparation or worker success as accepted checks.

The legitimate unchanged path consumes the actual `lattice.unchanged-candidate-observation.v1` bytes at native `artifacts.unchanged_receipt` (request `allow_unchanged: true`). It does not synthesize an empty Produced receipt. The trusted capture requires both explicit already-satisfied policies and exact baseline equality, resolves the candidate Git commit to the baseline, then canonical verification still executes the frozen checks.

### Protected proof-evidence checks

Proof evidence is a versioned artifact of a normal frozen `VerificationCheck`, not a second runner. A trusted `VerificationProfile.proof` names the one `kind: "proof"` adapter check, the retained manifest and proof-byte paths, exact claim/theorem inventory, approved specification/dependency/assumption-policy byte identities, allowed assumptions, required semantic replay/control roles, producer/checker/tool identities, build/runtime identities and limits. Every approved input path must also be in `protected_paths`; the verifier hashes those bytes from its independently materialized exact candidate snapshot before it accepts the adapter manifest. Kiln BLAKE3 identities remain distinct from provider-native Lean SHA-256 pins.

The manifest binds work identity/revision, attempt/fence, repository, candidate/tree, acceptance and profile identities, theorem inventory, approved semantic inputs, actual retained proof-byte identity, exact producer/checker/runtime/tool identities and all required outcomes. The controller compares these values to the frozen profile and retained artifacts after executing the named frozen adapter program. Exit zero, the manifest's own hashes, or a self-asserted `PASS` is insufficient. `PASS`, `FAIL`, `NOT_RUN`, `NO_CLAIM`, `APPROXIMATE`, `DEAD`, timeout, cancellation, and unknown outcomes are distinct inputs; only the complete exact required profile can discharge the proof check.
For this pinned Lean producer, the trusted policy requires exactly nine control results, all three independent roles (`official_replay`, `producer_replay`, and `nanoda`), and exactly the source-approved axiom allowlist `propext`, `Classical.choice`, and `Quot.sound`. Each result retains bounded raw stdout/stderr whose BLAKE3 values are recomputed, not merely asserted. The adapter invocation digest covers its frozen program path/hash, arguments, prerequisites, acceptance scope, artifact paths and timeout.

For the retained Compiler Lab Lean source contract, the approved adapter must invoke `check_standalone(project, lean_path, module, declarations, work, timeout_ms)` from source revision `c582f93f00848048f1c43ff4013b9fabe103ec7e`. Its exact frozen schema-2 profile is keyed by the project's exact `lean-toolchain`; retain the upstream profile SHA-256 and each tool's source revision/SHA-256. The manifest must account for the official export replay, fresh producer replay, independent Nanoda result, and every required invalid control, with matching theorem/module/tool/runtime identities and the allowed-axiom audit. The profile's executable and runtime closure are resolved by the operator outside worker editing authority and are executed only through the existing `verify_candidate` check path.

No installed qualified Lean profile/executable is established by the retained upstream source, so source-contract fixtures are not real proof qualification. Compiler Lab #220 is open and no parser-enabled MM0 backend/profile or replay fixtures are qualified; MM0 therefore remains unsupported. Do not set a profile's `qualified` field on the basis of schema validation or synthetic fixtures. The operator entry point remains `kiln-web-service verify-implementation --profile CI_SERVICE_PROFILE --database DB --enrollment ENROLL --attempt ID --fence N --verification-profile FROZEN --authority AUTHORITY`; it runs the exact trusted adapter profile, rereads retained evidence and feeds only the resulting typed check into the existing canonical Engine admission.

### Native canonical CI graph preparation

Build/use the owning native constructor; do not relabel an unrelated successful CI profile:

```console
cargo run --locked -p kiln-web-service --example implementation_verification_profile -- \
  --template DEDICATED_EMPTY_GRAPH_SERVICE_PROFILE \
  --verification-profile FROZEN_PROFILE --project ID --output NEW_SERVICE_PROFILE
```

This parent-run command was not exercised here. The ordinary ServiceProfile template must have empty nodes/edges for the selected project and explicit real source enrollment, transports, policy, platform, controls and bounds. The constructor refuses any existing graph, derives every frozen check/kind/prerequisite and native Cairn marker/all-check dependency through `Engine::implementation_verification_operation`, and validates with the shared runtime graph validator and `ServiceProfile.admit`. It preserves unrelated fields/projects and emits configuration only (`checks_executed: false`, `effects_executed: false`), never accepted producer data. Freeze the actual output path/BLAKE3 as the verify step's `canonical_profile`.

Use actual `inspect-work --database DB --enrollment FILE --work ID` metadata for envelope source aliases/versions and native `acceptance_digest`. The strong transport checks these exact values, not merely a shape-correct task/spec digest. Native prepared JSON values are available as `@{document-CAPTURE}`, so requirements/dispatch templates can consume owner-derived records without duplicating a canonical codec.

Canonical verification retains exact native check bytes, including blocked
outcomes, rather than treating them as uploaded normalized report envelopes.
Their dedicated validator binds each check to the canonical source, attempt,
execution, receipt and retained artifact. Unknown checks may have no evidence;
successful checks cannot borrow missing or substituted evidence. Reopen still
validates full provenance and rehashes the actual retained files.

## Campaign operation

Export `config/implementation-qualification.ncl` applied to the exact operator-owned campaign record, then use `prerequisites --scope campaign --inputs FILE.json` before any runtime invocation. Only after prerequisites and authorized local isolation are available:

```console
python3 scripts/implementation-qualification.py campaign \
  --inputs /run/user/1555/kiln-qualified-campaign.json \
  --output /run/user/1555/kiln-implementation-campaign
```

A campaign requires every named scenario exactly once, with 1–128 bounded stages each. `fixtures/implementation-qualification/scenarios.json` is the scenario acceptance inventory, **not a passing receipt**. Missing scenarios, missing required actual consumer stages, absent native assertions, missing files, incorrect program identities and failed or unknown effects do not silently pass.

Each step names a fixed `stage`, explicit argv tail `arguments`, unique alphanumeric/underscore `capture`, expected native `exit_code`, `timeout_seconds` (1–3600), and explicit JSON-pointer `observations` with `equals` or `nonempty`. Negative cases require a nonzero native rejection and precise `stderr_contains`; a parse error is not meaningful acceptance rejection. Whole strings such as `@{claim#/identity}` substitute actual prior JSON without shell expansion. For protocol configuration/preparation only, a frozen `input_template = { path, blake3 }` is expanded into a nonsecret read-only file `@{stage#/input}`; native output can use `@{stage#/prepared_request}`. The path remains available as `@{input-CAPTURE}` or `@{prepared-CAPTURE}`. Native profile/request/needs producers—not this runner—compute canonical identities. No template is permitted for successful handoff, CI or provider-effect evidence. The actual native `/result/receipt` subtree is projected with provenance from retained raw stdout into verifier-private storage (`@{receipt-CAPTURE}`); nothing is invented or interpreted as acceptance.

The fixed stages include native profile/needs preparation, intake, claim, needs/admission, reservation, candidate capture, independent verification and publication preparation. `lattice-run`, `lattice-recover` and `lattice-cancel` all use the strong Kiln worker transport (`--worker-request`, not diagnostic `--request`). The actual chain is profile preparation/intake → claim → Lattice preparation → exact needs attachment/configured admission → bound reservation → strong dispatch → capture-candidate → independent verify. `lattice-cancel` additionally requires `cancel_after_seconds` and `mutation_probe = { path, quiet_seconds }`: signal the actual transport parent, retain descendant PID incarnations, observe real mutation before cancellation and unchanged bytes for the selected 1–10-second quiet interval. The native result must establish cancelled without acceptance/integration and recovery must reconcile it. Missing signal propagation, missing descendants or surviving incarnations fail/quarantine; parent exit alone is not cancellation. Timeout/output overflow kills only the owned process group for cleanup and leaves effects unknown, never native cancellation success. Raw output/artifact bounds are 16 MiB. Native paths must already have selected role access; the runner never weakens trusted-root permissions.

### Observed native-effect crash window

`restart-possible-effects` requires `lattice-crash` followed by successful native `lattice-recover` (which always adds `--recover true`) and durable coordinator recovery. A crash step requires bounded `crash_after_seconds` and expected exit `-9`. After observing actual descendants the harness stops only its owned dispatcher parent, lets the already-dispatched native worker finish under its admitted bounds, locates the real retained intent by exact request-byte digest (not a second directory-hash implementation), and waits for native raw artifacts plus quiescent observed child incarnations. While the parent is stopped it snapshots the real ledger and refuses an already-acknowledged workflow or moved fence. Only then does it kill that parent and retain the actual unknown-acknowledgement barrier. Missing artifacts, a missed window, remaining mutation or timeout are unqualified, never a fabricated successful effect. Subsequent native read-only recovery must observe the same execution without a new dispatch. This proves only the selected local worker-effect window; remote branch/PR/integration/source windows still require their own precise operator approvals and real retained observations.

### Required scenario observations

| Scenario | Required actual observation; no substitute |
| --- | --- |
| `bug` | Actual claim/reservation/OMP candidate/handoff/independent verify. Trusted report contains one failing baseline reproducer and successful candidate checks, exact source/profile/toolchain identities. |
| `feature` | Actual worker-produced candidate and independently executed explicit feature acceptance on that candidate. |
| `linked-sources` | Actual GitHub and ready Cairn observations under exact explicit source mapping; duplicate/out-of-order intake and competing claims yield one current canonical unit. |
| `expired-stale-worker` | Expiry/replacement under native provider termination or unknown barrier; old actual handoff/result is refused by fence/current authority. Expiry alone is not process termination. |
| `zero-exit-noop` | Worker really exits successfully without fixing the source; independent verifier rejects. |
| `forged-evidence` | Worker-written false success/checklist output cannot bypass actual verifier execution and native rejection. |
| `skipped-checks` | Missing required check/coverage/artifact is rejected, not skipped successfully. |
| `weakened-policy` | Candidate really changes frozen acceptance/protected policy; exact Git-object comparison refuses it. |
| `source-policy-movement` | Changed issue/spec/policy produces new admission; old evidence is refused and explicit replan/reverification is required. |
| `target-movement` | Actual target/base change makes affected evidence/publication admission stale; no publication from an old-base claim. |
| `restart-possible-effects` | New processes recover retained dispatch/run observations without a second worker. Branch-push/PR/integration/source-update crash windows require separately approved actual remote effects and remain unqualified by a local-only run. |
| `descendant-cancellation` | Actual Lattice/OMP descendant mutation stops before workspace reuse, with native recovery and retained process-incarnation observations. Unknown or surviving descendants quarantine the workspace. |
| `already-satisfied` | Exact unchanged commit is admitted only under explicit `VerifiedOrAlreadySatisfied`/`allow_already_satisfied` authority and independently passing checks; not an empty successful worker invocation. |

## Receipts and remote approvals

Every scenario retains scoped `scenario-receipt.json`; actual stages retain argv, raw stdout/stderr, BLAKE3, elapsed/exit and cancellation observations. Native artifacts and read-only ledger snapshots remain separate. Canonical verification stdout is the actual admitted `CandidateVerification`; the runner retains `authority.scratch_root/<handoff>/observations.json` only as raw execution evidence and inspects baseline/candidate observations for qualification assertions. It never imports that report into any acceptance API or creates a second successful CI store. `input-configuration.json` fixes nonsecret inputs. Keep native OMP JSONL, host receipt/unchanged artifact, real Git identities, canonical CI journal and check artifacts; never replace them with hand-written success JSON. Failed/unknown workspaces are not automatically reused or deleted.

For actual `bug`/`feature` `lattice-run` stages, the runner consumes the producer's native `artifacts.omp_observation` and `artifacts.raw_events` locators and retains both files. The completed nonempty native OMP observation must bind the raw JSONL BLAKE3 exactly. No duplicate private-directory hash codec or caller-invented artifact path is required. Only then can the campaign report an actual adopted OMP run. Remote crash windows remain separately enumerated in `remote_recovery_windows`, all unexercised by this local-only entrypoint.

`receipt.json` always separates:

- local scripted admission and acceptance controls;
- actual local worker/stage evidence (consult native OMP observations, not a fixture label);
- verified PR;
- exact remote integration;
- source-status/Cairn lifecycle reconciliation.

This entrypoint exposes **no** `publish`, `land`, `close-source` or remote integration command. Even a passing local campaign cannot claim the first verified-PR milestone. Immediately before a real remote effect the operator must approve the exact repository/provider, base/candidate, branch and PR intent, credential scope and effect; the owning CLI/provider performs it and retains native read-back observations. A recovery read cannot invent an effect. Current ordinary GitHub merge does not atomically guard both head and target, so automatic landing stays unsupported/PR-ready unless the exact remote provider is independently qualified. Source reconciliation requires separately proven integration and exact source identity; PR creation never closes work. Crash windows after branch push, PR creation, integration and source updates remain explicit separate qualification work until those precise approvals/effects are exercised.

No missing model service, authorization, adopted cohort, process boundary, candidate acquisition, native request preparation or provider effect may be papered over with a fake worker, envelope-only evidence, weakened check, new scheduler or broad host permission. Preserve an explicit missing/unsupported/inconclusive status and the exact retained reason.
