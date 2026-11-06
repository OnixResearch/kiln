## Context

`kiln-adapter-mantle` executes the pinned Mantle cohort with `--store` and
`--state-dir`, then admits the native `crunch-build-report-v1` document. The
pinned producer serializes the logical store prefix as `store_dir`, the
physical export root as `output_dir`, and the state root as `state_dir`.
Kiln's runtime binding previously had one `store_path`, used for both the
`--store` argument and the report `store_dir` comparison, so a real dispatch
with distinct logical and physical roots could not pass admission.

The report also carries native arrays for hermeticity, environment policy,
finish gates, network policy, workspace execution, action results, dynamic
plans, plan output bindings, scheduler decisions, overlays, store layers,
remote telemetry and observability, frontend attestations, structural and Cargo
evidence, diagnostics, failures and FOD mismatches. The previous parser kept
only `counts`, `outcomes[].outputs`, `outcomes[].log_file`, `outcomes[].cached`
and a boolean for any substitution marker.

## Decisions

### Decision: Exact pinned top-level schema

The adapter defines every top-level field serialized by the pinned producer and
uses `deny_unknown_fields`. A missing mandatory producer field or an unrecognized
top-level field is producer/contract drift and fails before completion.
Producer-specific nested evidence remains bounded JSON because Kiln has no
authority to reinterpret it; its collection count is recorded as an observation
metric.

**Rationale:** A pinned cohort must not silently accept an unadmitted report
shape. The alternative, a permanently permissive parser, hides schema drift and
makes a native fact disappear without a diagnostics path.

### Decision: Logical and physical roots are separate

`MantleRuntimeBinding` gains `store_prefix`. Dispatch passes
`--store-prefix <prefix>`. Admission requires:

- `report.store_dir == profile.runtime.store_prefix`;
- `report.output_dir == profile.runtime.store_path`;
- `report.state_dir == profile.runtime.state_path`.

Product `path` values must be under the physical output root. Artifact
`logical_path` values must be under the logical store prefix. Artifact sidecar
and log paths must be under the state root. All paths are lexical absolute
paths; Kiln does not resolve symlinks or claim filesystem containment.

**Rationale:** Mantle's logical identity and physical carrier are different
facts. Conflating them rejects real reports or accepts a report about another
store. Exact equality keeps the report bound to the selected profile.

### Decision: Counts and failure facts agree

`succeeded_total`, `built_total`, `cached_total` and `failed_total` must equal
the corresponding outcome and failure arrays. A successful report must have no
failure envelope and no FOD mismatch. Failure envelope phase and error class
must be members of the pinned producer vocabulary. Failure `saved_log_path`
values are added to the same bounded log map as successful logs.

**Rationale:** Counts are a native summary, not an independent source of truth.
Rejecting disagreement prevents a partial or fabricated summary from becoming a
successful observation. Binding failure logs preserves the operator's ability
to inspect why a build failed.

### Decision: Native facts become bounded metrics

Every native collection contributes a deterministic `native-*` count metric.
Cache substitution contributes output count, transferred bytes, reused bytes
and fallback count. Hermeticity mode contributes three mutually exclusive
zero/one metrics, and optional overlay/remote-observability presence contributes
zero/one metrics. The complete report bytes remain the receipt identity.

**Rationale:** Kiln's canonical build result is provider-neutral. Counts and
byte totals are bounded, numeric, replayable facts that fit the existing
observation model without importing Mantle-specific types or making producer
evidence authoritative. Nested evidence remains bound by the report receipt and
is not promoted to CI or release authority.

### Decision: Reject parent-directory components before root admission

The shared `require_path_under` helper must reject any
`std::path::Component::ParentDir` in addition to requiring an absolute path
and the declared root prefix. A prefix alone admits `root/../escape` and is
not lexical containment. Apply the same helper to physical products, logical
artifact paths, artifact sidecars, successful logs and saved failure logs.
Regression coverage must independently mutate each fact while retaining an
otherwise valid report, including a valid native failure envelope for its log.
This is lexical admission only: no filesystem resolution or symlink guarantee.

## Risks

- A future Mantle report field requires an adapter change and a cohort refresh;
  that is the intended exact-cohort behavior.
- Lexical path checks do not detect filesystem substitution or symlink escape.
  The runtime already consumes the emittent's paths under bounded process
  control; this change does not add filesystem capability authority.
- Metric names are Kiln-owned and must remain stable for replay comparisons.
- Failure messages remain native report text bound by the receipt; Kiln does
  not claim their truth or redact arbitrary producer messages beyond not
  projecting them into canonical state.
