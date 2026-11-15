## ADDED Requirements

### Requirement: Exact admitted descriptor
r[kiln.preview.exact]
The core MUST admit only an opted-in successful exact build with every required product, current candidate and complete bounded runtime closure. A descriptor MUST be minted only from the actual successful BuildEffectPlan and product/store observation retained on that candidate's canonical successful Engine run; caller-authored observation JSON, current-binding JSON and check digests MUST NOT grant build or launch authority. Descriptor identity MUST bind run, node, attempt, candidate, product, executable, closure, trust, engine and finite limits. Partial or stale facts MUST NOT launch.

#### Scenario: Reject altered evidence
- GIVEN an admitted build and complete runtime closure
- WHEN candidate, closure, product or runner differs or a required member is absent
- THEN launch MUST fail without substituting an artifact
#### Scenario: Caller-supplied success assertions are not build authority
- GIVEN a canonical run with no retained build product observation
- WHEN a caller supplies matching plan, observation, run and check digest JSON
- THEN no runnable descriptor or review launch MUST be produced

#### Scenario: A newer failed run supersedes an earlier success
- GIVEN an earlier successful preview and a newer failed or incomplete run for the exact source
- WHEN review projects the selected current CI run
- THEN the earlier descriptor MUST remain unavailable for launch

### Requirement: Explicit owned local build store
r[kiln.preview.owned_store]
The canonical preview fixture MAY opt into one existing private owned Nix store root. When selected, that exact root MUST be bound into the admitted build profile and operation identity, and every Nix reader and writer used by the canonical build MUST select `local?root=ROOT` explicitly. Logical product/store identities remain `/nix/store`; only physical file access maps through `ROOT/nix/store`. An invalid or unavailable selected root MUST fail closed and MUST NOT fall back to the host store. An omitted root MUST preserve existing production system-store behavior. Preview production MUST use its separate read-only native store adapter.

#### Scenario: Canonical build stays in the selected owned store
- GIVEN an existing private owned Nix root containing the admitted signed runtime package
- WHEN canonical source-bound build evaluation, preflight, realisation, observation and signing run
- THEN every Nix command MUST select that same local root and retain logical `/nix/store` identities
- AND the host shared store MUST NOT be written

#### Scenario: Invalid owned root cannot fall back
- GIVEN a selected root that is missing, non-private, non-normalized or URI-unsafe
- WHEN the build profile is admitted or a selected store becomes unavailable
- THEN profile/build admission MUST fail without retrying against the host system store

### Requirement: Existing Linux engine consumer
r[kiln.preview.runtime]
The shell MUST verify every runtime NAR hash and required trusted signature, export no secrets or build-only inputs, and invoke an exact already-built TryNix Linux engine. Resources, expiry, retention and cleanup MUST be finite. Discovery, hydration, guest boot and program readiness MUST remain distinct. Missing cache members, wrong signatures, unsupported engines and exhaustion MUST be explicit preview-only failures.

#### Scenario: Real clean and warm cache
- GIVEN exact prebuilt signed Linux CLI output and a content-pinned TryNix site
- WHEN a clean or warm disposable browser opens the retained descriptor
- THEN the exact executable MUST run with retained phase observations and no source build

### Requirement: Exact review action
r[kiln.preview.review]
The review page MUST display exact artifact and runner identity and launch only the opted-in selected current candidate. Preview failure MUST NOT change CI, review or merge evidence. A stale descriptor MUST NOT be relabelled current.

#### Scenario: Review candidate moves
- GIVEN a retained preview descriptor for an earlier candidate
- WHEN a reviewer selects the new candidate
- THEN the old descriptor MUST remain historical and MUST NOT launch as current
