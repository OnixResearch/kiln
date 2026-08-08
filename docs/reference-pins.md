# Design reference pins

The review date is 2026-08-08. These references inform bounded Kiln contracts and do not create runtime dependencies.

| Reference | Exact pin | License or specification | Selected concept | Rejected concept |
|---|---|---|---|---|
| Hydra | `07213d087c827266df136157d5a6c7ccd24db7ca` | GPL-3.0-or-later | CI vocabulary, evaluations, products, metrics, operator views | Monolithic control-plane authority |
| Radicle CI broker | RID `rad:zwTxygwuz5LDGBq255RA2CbNGrz8`, revision `a688afde1f730ef60402a809d9cdadc55e8dd749` | MIT OR Apache-2.0 | Node-local filtering and one-line JSON adapters | Broker database as Kiln state authority |
| Radicle native CI | RID `rad:z3qg5TKmN83afz2fj9z3fQjU8vaYE`, revision `f8353f411e6b97c78b988e4025f02013bdf4c522` | MIT OR Apache-2.0 | Two-response adapter lifecycle | Unisolated repository shell execution |
| Jujutsu | `786e65a1ee9648b2a72dc8c8f94d811833a17987` | Apache-2.0 | Change IDs, commit IDs, bookmarks, operations, divergence | Change IDs or revsets as immutable revisions |
| Choregraph | `2afbafb266c60530ae05a6862eb06ace72ae787a` | repository license | Pure bounded transitions | Execution authority in the graph core |
| Aspen | `08e4dfa5443737c81577f6691c673ab1ad78db4d` | repository license | Optional runtime shell | Runtime types in Kiln core |
| Mantle | `5dd6e8826e2934709e0b584ccf445a6fc5c57ba1` | repository license | Optional build observations | Builder authority transfer to Kiln |
| Lattice | `5c9902f777412296c13e2347e45b3bede335b828` | repository license | Optional workflow execution | Workflow types as CI domain types |
| Valence | `b37177bbdd9ca0665412e6600f401e5cb54908d2` | repository license | Optional evidence projection | Evidence linkage as behavioral proof |
| Cairn | `837d730799faffe6a088cfd6dc49e08c7e0f1822` | repository license | Gates and six COB profiles | Lifecycle authority in Kiln |
| Zuul | release `12.0.0` | Apache-2.0 | Exact candidate sets and dependent queues | Gerrit, Git, executor, or merge authority |
| Concourse | `95872ce1a3af6fc0c4ab7f23658ec81f3e778d7c` | Apache-2.0 | Discovery, pinning, acquisition, mutation split | Universal opaque resources |
| Bazel Remote Execution API | `becdd8f9ff811df88a22d3eadd6341753d51d167`, API package `build.bazel.remote.execution.v2` | Apache-2.0 | CAS, action, operation, cache, and log identities | Protocol types in Kiln core |
| SLSA | Provenance `v1.1` | Community Specification | Provenance projection vocabulary | Automatic SLSA-level claims |
| in-toto | Statement `v1` | Apache-2.0 | Subject and predicate separation | Signer trust from schema validity |
| TUF | Specification `v1.0.31` | CC0-1.0 | Threshold, rollback, freeze, mirror threats | TUF metadata as Radicle governance |
| Kueue | API line `v1beta1` | Apache-2.0 | Admission and resource-fairness tests | Worker placement ownership |
| Dominant Resource Fairness | NSDI 2011 paper | Paper reference | Dominant-share invariant | Universal comparability of provider units |
| Temporal | Event-history compatibility model, review date 2026-08-08 | MIT | Replay compatibility checks | Temporal runtime dependency |
| CDEvents | Specification `v0.4.1` | Community Specification | CI event projection vocabulary | Imported events as canonical state |
| CloudEvents | Specification `v1.0.2` | Apache-2.0 | Event envelope binding | Envelope identity as Kiln identity |
| OpenTelemetry CI/CD | Semantic conventions development status, review date 2026-08-08 | Apache-2.0 | Rebuildable telemetry projections | Telemetry as lifecycle authority |
| Buildbot | `e055ebda6b4800b4762e45088b23b736e046cecf` | GPL-2.0-or-later | Source, scheduler, request, worker, reporter split | Buildbot runtime dependency |
| Pueue | `b3fe223ef0a3c51d95d5c6128700b79442c54af7` | MIT | Status, logs, wait, capacity groups, scoped controls | Scheduler, daemon, protocol, provider, or runtime integration |

## Maturity and non-claims

Hydra, Concourse, Buildbot, Jujutsu, and the Remote Execution API are mature references. Their maturity does not transfer to Kiln.

Radicle CI and its native adapter remain active ecosystem references. Passing compatibility fixtures does not prove node, broker, or runner correctness.

The standards define interchange shapes. They do not grant source, builder, signer, gate, or release authority.
