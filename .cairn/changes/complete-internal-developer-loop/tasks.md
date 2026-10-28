## Phase 1: Implementation

- [x] [parallel] T1 Deliver real multi-node pipeline execution, timing/resource observations and populated structured reports. r[kiln.web.developer_loop.pipeline]
- [x] [parallel] T2 Deliver observed worker heartbeat/capacity and fail-closed isolated execution with cancellation/recovery. r[kiln.web.developer_loop.workers]
- [x] [parallel] T3 Deliver bounded verified provider-backed shared caching, retention and hit/miss/transfer observations. r[kiln.web.developer_loop.cache]
- [x] [parallel] T4 Deliver developer submit/watch/logs/cancel/retry and exact explicitly selected dirty-worktree inputs. r[kiln.web.developer_loop.cli]

## Phase 2: Verification

- [ ] [serial] V1 Integrate all callers/contracts, exercise disposable native service/worker/CLI/browser paths, run required repository gates and document repeatability evidence and exact limitations. r[kiln.web.developer_loop.pipeline] r[kiln.web.developer_loop.workers] r[kiln.web.developer_loop.cache] r[kiln.web.developer_loop.cli]

Native integration evidence: `evidence/native-07/verification.json`. Viewer-browser
observations and four screenshots remain in `evidence/native-06/`; earlier failed
trials remain in `native-01` through `native-05`. Both final trials retired all
processes, credentials and scratch. The ownership-lifetime regression, complete
Cargo workspace, signal fixture, strict lint rails and x86_64 Nix flake checks now
pass. Explicit active-development reference coverage is recorded separately from
the unchanged accepted-only release profile. V1 remains open while that default
release-readiness gate rejects unaccepted active-change anchors; this package is
not accepted or release-ready. `evidence/qualification.json` records exact scope.
