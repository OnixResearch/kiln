## ADDED Requirements

### Requirement: Exact admitted descriptor
r[kiln.preview.exact]
The core MUST admit only an opted-in successful exact build with every required product, current candidate and complete bounded runtime closure. Descriptor identity MUST bind run, attempt, candidate, product, executable, closure, trust, engine and finite limits. Partial or stale facts MUST NOT launch.

#### Scenario: Reject altered evidence
- GIVEN an admitted build and complete runtime closure
- WHEN candidate, closure, product or runner differs or a required member is absent
- THEN launch MUST fail without substituting an artifact

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
