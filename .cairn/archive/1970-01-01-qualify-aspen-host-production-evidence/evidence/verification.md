# Exact-cohort local host qualification

Owner: Kiln maintainers. Consumer: the bounded Aspen/Lattice CI host.

## Exact inputs and claim

The frozen executable-input candidate is `afd06231e8e616c86a6f507fb2a69c41babd1f84`. Its normalized GNU TAR is 14,714,880 bytes with BLAKE3 `fca77390881731154dae79f179faa6cfa51d08fb626fc2ce35cac112cd07f044`; the [complete manifest](artifacts/7a113bc55621d10403728a36879fd3a05674bd205900e12f239aa4b817d50044) binds all 1183 regular files and excludes `.jj/` and `.git/`. This is the tested input, not a claim that later evidence-only descendants were rebuilt.

The [expected cohort](cohort.json), identity `871bd733bf5567f6e2e060285b6b8c4607a36e27667f6371e532b224b4716901`, binds the source, primary host/extension/provider executable bytes, actual runtime/collaboration/provider profiles, [host and native-case inputs](artifacts/579ade503a142f35e4c5c5dabe867f884739e6fbe3ee1d7d9b5979c65ebdfe8c) and [unpublished-candidate publication binding](artifacts/1e88d3b34545d3516ef41e111c3c5b7d1719cb9115e0d23e0cd66f9e2ae461b3). The 13 exact native profile variants and two deliberately different extension launches in the explicit rollback case are retained separately; they are not silently promoted to the primary executable/profile cohort or an operator deployment.

The [preservation observation](artifacts/f965ba53b4dc3e4805d45cd3f9e7078802eca5983f3fd5766b2be8679e0d3c17) records all 29 accepted specification files and all 13 lifecycle-policy inputs unchanged from published baseline `2bf997aeb81b6c2047e816526a99b0a6d2847eae`. No requirement, lifecycle gate, producer pin or deny-all policy was weakened. Pending provider-backed live shadows remain pending.

## Executed observations

| Case | Observed boundary | Evidence |
|---|---|---|
| `durability` | Actual successful provider report retained; Lattice stopped before duplicate and SIGKILL/restart; all complete acknowledgements byte-equal and report files unchanged. | [retained case](artifacts/8e0d0db6029739cfd751cec15134b083ed5f87fd807578c963de5595189025d8) |
| `uncertain_recovery` | Recoverable uncertainty resolved through RecoverDispatch and Observe without another Dispatch; persistent unavailability retained Unknown across restart. | [retained case](artifacts/ccbb76a306483dd479caa015e6cf29044c16c2956f3a68cb58bd5ea6fef55818) |
| `corruption` | Malformed marker, truncated ledger, corrupt native journal, corrupt callback value and missing callback directory rejected before admitting ingress. | [retained case](artifacts/bda4b262e7896967c49b0c4a4cd3df45064fdde73097267f64f8e4997e87177b) |
| `provider_teardown` | Success, timeout, stdout-limit, stderr-limit and SIGTERM-cancellation cases passed; original PID/group/start-time identities observed absent (None), not reported absent while merely unreaped. | [retained case](artifacts/0fb91919ac3627ba3211152ca32160a89d42f7ff2bc45906fe4d8ff3c26e5c01) |
| `finite_load` | All submit/duplicate/reconcile acknowledgements matched before and after restart; exactly four Dispatch, four Observe and zero RecoverDispatch commands. | [retained case](artifacts/e8f7eeda8103cc35323ea8947125010281797cbc902cc960bf73ca6d6499cfcc) |
| `backup_restore` | Four durable components and all sidecars copied after host exit, exact member identities retained and original acknowledgement replayed; partial missing-journal restore rejected and repaired from the same complete backup. The included snapshot helper retained all 4,095 flat members with exact bytes and rejected one more without returning a partial backup. | [retained case](artifacts/b870ec6046413b431d9f150a527eaa837696271a69ccd637486fccd132758568) |
| `rollback` | Mixed cohort transitions rejected, separately admitted candidate profile exercised on fresh state, then prior executable/profile/state restored and original complete acknowledgement replayed. | [retained case](artifacts/424c3f1ad575e675e75c8b33e9b15a6fdca2891cf1b59026c5206219cb8209a3) |
| `publication` | Product source revision, normalized source archive, manifest, executable/profile inputs and public Radicle governance snapshot bound without changing accepted specifications or lifecycle policy. | [retained case](artifacts/e6a410977880ce445aded6cd7aa5a30b5497a9f1d4aab046e74ef07ac16d00ea) |
| `authority` | Native host/provider admission and the exact-source root checks passed. The actual current Nix CLI rejected the historical blocked receipt. Complete current-cohort positive and negative consumer observations are separately retained to avoid a self-referential receipt. | [retained case](artifacts/efbcbd4460b77ba3d02c035ed138d7b73104c460ef4865571e549e1abbb427ca) |

The [native host invocation](artifacts/7271a71f4fc598487824cf960ea4d4a7ab7bd214f70fdf17609c2af0cf6fd85b) passed 26 tests and strict Clippy. Its raw observations retain 28 child launches/exits, 38 acknowledgements, six explicit Unknown results and 13 quiescent state snapshots. The [provider invocation](artifacts/cd65c4261449f9d090e187209e0e21989e5f8b78a40b5980382728a5c519a1a0) passed four unit and five actual-process tests plus strict Clippy. Closed Lattice transport fixtures establish only their named finite host/recovery cases.

The separately identified [real Aspen host -> real Lattice -> Nix observation](artifacts/80f336c42bd48d7e8983bb8af7d7d25908fdc3a14a0243a311a0c44625e3a1fe) retained a successful provider report for its isolated local source fixture, not a build of this Kiln candidate. Lattice was stopped before duplicate ingress and SIGKILL/restart/reconciliation. The [complete acknowledgement byte proof](artifacts/e4107355d69e955ea13551e9970da0276020904909c52b6a65af741ec690ffdb) binds all four actual command outputs; report files remained unchanged. This does not prove arbitrary Nix or CI correctness.

The [included snapshot-helper smoke](artifacts/2cc94a424e3b14275be2de73718e74995b5f764696eb0b848f4ebec465c2143b) and its [exact runner](artifacts/78b9e7326ac2ddc1e957aa7ec9a967f77694e5ceec66aeb93a50996b423febbe) observed depth 128 admitted, depth 129 rejected, all 4,095 flat members retained with exact bytes, and a 4,096th member rejected without returning a partial snapshot. Native snapshots retain complete matching state and sidecars or fail closed under the 128-open-directory and 4,096-step-per-component bounds.

## Required gates and receipt consumer

The [refreshed Nix invocation](artifacts/566409856e1483147177aa82c1c764bb555ed85b546728dba9fdb08e0883644d) reports **all 30 x86_64-linux checks passed**. The [exact-derivation proof](artifacts/18154819c0f33b66d921fd1be45b32c61ead04d4eb987adc2ffa7ef2950bf550) records reuse of identical already-successful derivations after the metadata-only history repair, not another execution of cached root tests. The [original complete execution](artifacts/d8c2ea484c2aed3038cb32e960db81f43af0618ab5419a6a9b36f1b2a3f86110) includes the host/provider packages, required exact-producer Octet deny-all checks, strict workspace Clippy, Nickel, source/provider conformance and the three-crate no-std check; its root package exercised 410 workspace tests, with two opt-in live tests ignored. `aarch64-linux` was not exercised.

The [frozen-source native Cairn observations](artifacts/80a1e76e95c4ab945300fae74277bc5f70179a74fbe4c89b32aa3c9be19e62d7) report validation valid, proposal/design/tasks gates PASS and declared traceability 625/625 with no missing or dangling IDs. Declared traceability is linkage, not execution truth. Later task/review/evidence bookkeeping is validated separately and does not retroactively alter this frozen input.

The [completed current task and semantic-review inputs](artifacts/f125c85f5c1f02046595028e44e2a7108dd4c7ab74d0541bbaeb1006063552b3) independently passed native validation, proposal/design/tasks gates and 625/625 declared traceability after task completion. Accepted requirements and policy remain unchanged.

The actual built current-source CLI [admitted the complete receipt](artifacts/9a97d2cac103c0a135f47b9c7c4065e4d3c351ee27c6f42f15c4544e654e1c0b) with result `admitted`, claim `bounded_local_host`, receipt identity `9b00f30e9762a03fcfcc1580bef3da3d511157e448e291de253bba3bbd593da3` and 13 directly verified distinct retained artifacts. The [qualified receipt](receipt-qualified.json) retains all nine execution cases and all nine required gate observations.

The same CLI [rejected 16 metadata variations](artifacts/20ce5af2af2a3b174b759140a7fea046514d12a77ac59992355e53a575199c26): missing, duplicate, failed or unknown observations, a failed required gate, stale and mixed cohorts, unsupported version and all eight stronger claims. It also [rejected changed retained bytes and an absent artifact root](artifacts/4d04fdc524bb69137682f2a76671a6938e92133ac2c91b835c66a8a37c033bbc). [Complete consumer proof](artifacts/a8e09121075d0bdb324e49a87bad6e474a095de49eb1c77ee082722cf943dbca) links the actual executable, inputs and command outputs. Admission checks supplied consistency and direct artifact bytes; it does not attest observation truth or recursively certify assertions inside those artifacts.

## Authorized main publication

The separately authorized [publication receipt](publication.json) records both declared `main` refs fast-forwarded from `2bf997aeb81b6c2047e816526a99b0a6d2847eae` to `c9266e05d3d44f8d0be06aa60c69edd6b3586adb`. This source-plus-evidence revision is distinct from the frozen executable-input revision above.

Fresh GitHub SSH, namespace-scoped native Radicle and anonymous Garden HTTPS checkouts each reproduced all 1441 regular members and the 23767040-byte normalized USTAR archive, BLAKE3 `561e1106ae4c836d1042042818ad181b97b190214a98f92430015ea213826c61`. Native readback used the exact configured case-preserving `rad::rad://<RID>/<delegate>` helper URL; Garden used client-scoped Git protocol v0. Earlier malformed/decorated and lowercased native-URL attempts remain failed observations, not failed source identities.

Iris, Rosa and Garden namespace APIs reported that same `main` at read time. Public visibility, the one delegate and threshold one remained unchanged. Exact post-push signed refs, acquisition commands and bounds are retained in the receipt. These observations do not prove seed trust, physical replica independence or future availability.

Post-push receipts and readback observations are retained in a local evidence-only child and are not inside the published revision or its archive. No deployment, live-service change, lifecycle sync/archive or release action follows.


## Historical failures and superseded cohorts

The [original blocked verification](artifacts/c0ad1feee13d0db6b61193dfda38d2c6f786b3a1153b8af7fc5e524499bf7be3), [incomplete receipt](receipt-incomplete.json) and [blocked receipt](receipt-blocked.json) retain their original unpublished payload/cohort. The [current-source CLI still rejects the historical blocked receipt](artifacts/edc45879fb3ed5c0f7dce7508193ae5f0e52e6ec0e69ee639f7f4f670eb3be7e). Historical source archives, publication receipts and failed or quarantined state were not relabeled current or passing.

The initial system-daemon source-import lock blocker remains historical. An owned isolated Nix store [admitted the exact clean source](artifacts/56933f8244cacf2bde6b2a64dc6b1bb1e07126c807b8bb53706c5883d1535b09) without unlinking the unrelated root-owned lock or changing the required sandbox/cgroup policies. Intermediate failed attempts remain in the [artifact index](artifact-index.json), including the [test-only traversal Octet failure](artifacts/874c7de8e4bdd34b3138755319eb75faeca159c87c12e5ccb795c7258ccbf62e). The final source adds explicit finite traversal bounds and passes the unchanged checks; no alternate Octet producer was substituted.

Earlier missing device access, read-only cache, missing report namespace and quarantined-state recovery failures remain failures in their original cohorts. A workflow acknowledgement of `completed` alone is not a successful provider report. No generic recovery from quarantine, release-upgrade compatibility or live Onix shadow is claimed.

The first authorized main push stopped at an inherited commit without a description; the [metadata-only repair](artifacts/d87457f6c3e60a961ca423a487203d8a0397d9dfef5037d6dc881d7aadfddc55) changed descendant identities without changing source bytes. The [superseded qualified narrative](artifacts/553917d18b0e7d43ec3841aad279ad91ea155a1955f0d3d55c75375c0666ba09) and [earlier refreshed recovery](artifacts/3e992dd68ddafebe060767786fc63d3c568b792836b1831b30d78497976f3adf) remain separate from this final executable cohort. A [failed final-binary fixture](artifacts/d06381f048112e08663519ddcd5f2939107ca9fc85ae16045a18ae1c89e98864) omitted the required pre-existing report namespace directory; the passing observation uses fully prepared fresh state, not that failed state.

## Repeatability and authority

Permanent host drills live in `runtime/kiln-aspen-host/tests/hosted/qualification.rs` and its case modules; process-control cases live in `runtime/kiln-radicle-nix-provider/tests/process.rs`. Pure qualification decisions live in `crates/kiln-core/src/hosting/qualification/`. Use the host/provider README commands and the root operator checks, rebuild changed inputs, record actual source/executable/profile/host identities, retain every failure, then execute the consumer against the reviewed expected cohort:

```console
EVIDENCE=.cairn/changes/qualify-aspen-host-production-evidence/evidence
kiln host-qualify "$EVIDENCE/cohort.json" "$EVIDENCE/receipt-qualified.json" "$EVIDENCE/artifacts"
```

These observations establish only bounded local-host behavior for the exact recorded inputs. They do not prove storage-device or global durability, exactly-once delivery, CI correctness, provider truth, sandboxing or availability. They supply no credential, publication, release, deployment or governance authority. Any separately authorized source push must retain its own exact revision, archive, signed-ref and acquisition observations; qualification does not change repository visibility or authorize live service changes. No lifecycle sync/archive is implied.
