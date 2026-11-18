# Artifact Preview Specification

## Purpose

Defines the `artifact-preview` capability.

## Requirements

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

### Requirement: Exact preview evidence comparison
r[kiln.preview.comparison]
The preview server MUST retain only bounded browser reports whose identity, binding, engine and invocation are copied from the admitted descriptor, under a domain-tagged BLAKE3 identity with finite expiry and a bounded count. The review service MUST project only verified, unexpired records for the change's project, repository and revision commits, and MUST mark a record current only for the current revision commit and the selected successful CI run. A comparison MUST pair one projected baseline with one current candidate, derive a deterministic identity from both and the rule version, and report every engine, platform, argument or executable mismatch. Unknown, expired or unprojected subjects MUST be refused without substitution. Only the current candidate MAY offer a launch. Observations MUST NOT change CI, review, merge or implementation-completion state.

#### Scenario: Reviewer compares a baseline with the current candidate
- GIVEN retained observations for a baseline and the current candidate
- WHEN the reviewer selects both or reloads the deep link
- THEN both exact identities, phases and bounded outputs appear side by side with the comparison identity and any incompatibility reasons

#### Scenario: Forged or mismatched evidence is refused
- GIVEN a report for another descriptor, a foreign origin, an oversized or malformed body, an unknown phase or a full store
- WHEN it is submitted or read back
- THEN no record is retained or projected and the refusal is explicit

#### Scenario: Candidate moves
- GIVEN a retained comparison whose candidate revision is no longer current
- WHEN the review is reloaded or a late response arrives
- THEN the observation is shown as historical, cannot be compared as the current candidate and offers no launch

### Requirement: Opt-in structural preview evidence

r[kiln.preview.structural] A preview comparison MUST add structural source evidence only when the request explicitly selects it; without that selection the comparison response, launch rules and graphical cohort MUST be unchanged. The service MUST compute the evidence with the code review's structural function for the exact commit comparison of the baseline and candidate revisions, with exact whitespace and the review's default bounds, and no grammar MUST run in the browser. The record MUST bind the comparison identity, both revision commits, the structural rules, a grammar identity, the bounds and either each file's structural rows or its named fallback, under a domain-tagged BLAKE3 identity. The grammar identity MUST cover the exact package, version and registry checksum of the Tree-sitter runtime, highlight engine and every grammar, checked against the workspace lock file. The record is display-only preview evidence: it MUST NOT change CI, review approval, merge or implementation-completion state.

#### Scenario: Off unless selected

r[kiln.preview.structural.scenario.off]
- GIVEN a baseline and a current candidate observation
- WHEN the comparison is requested without selecting structural evidence
- THEN the response MUST be the existing comparison with no structural record, and selecting it MUST add only the record

#### Scenario: Frozen fixture matches the structural review

r[kiln.preview.structural.scenario.fidelity]
- GIVEN a frozen fixture of a pure reformat, a moved unchanged function, one edit and named fallbacks, retained with its comparison settings
- WHEN structural preview evidence and the review's structural mode compare the same two commits
- THEN each file MUST carry the same structural rows or the same named fallback on both sides, the reformat MUST show no change and the function MUST be reported as moved

#### Scenario: Unusable input leaves an explicit reason

r[kiln.preview.structural.scenario.fallback]
- GIVEN incompatible subjects, a file without a grammar, binary or truncated content, a parse error, a hit bound, an unreadable source comparison or an oversized record
- WHEN structural evidence is requested
- THEN the record MUST carry the named incompatibility, fallback or unavailability reason, MUST retain no partial tree and MUST NOT substitute another observation or revision
