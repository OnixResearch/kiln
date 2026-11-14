# Cairn

<img width="400" height="400" alt="Cairn project logo" src="https://github.com/user-attachments/assets/5f3ecfb2-0abb-42c8-b03c-8cd35afa7b77" />

Cairn is a Rust lifecycle engine for repository work. It uses a native `.cairn/` layout and a Nickel-authored policy layer.

Old `cairn/`, `ironspec/`, and `openspec/` trees are migration inputs only. Normal commands do not use them as a fallback.

Cairn separates a pure core from an imperative shell:

- Nickel owns policy, schemas, stages, gates, reports, markers, and bounded exemptions.
- `cairn-core` owns deterministic transforms over loaded values.
- `cairn-gate-contract` owns host-independent external gate request and result binding.
- `cairn-cli` owns arguments, files, policy loading, output, exits, and mutation.
- Runtime commands use checked-in generated policy JSON.

The strict core is in `crates/cairn-core/src/verified/`. Slotcar checks its purity boundary.

## Project status

Cairn is under active development. Documented stable receipt fields are compatibility surfaces. Other commands, schemas, and policy can change.

A passing Cairn gate proves conformance to selected policy and inputs. It does not prove code correctness, evidence truth, or release eligibility.

## Repository layout

```text
cairn-policy/                Nickel policy, contracts, fixtures, and generated JSON
contracts/                   Consumer-facing typed Nickel contracts
crates/cairn-core/           Pure Rust models and lifecycle logic
crates/cairn-gate-contract/  Pure gate interchange contract
crates/cairn-cli/            CLI and MCP shell
fixtures/                    Native, interchange, and legacy lifecycle fixtures
docs/tutorials/      Operator tutorials
docs/hermes/         Copyable agent workflow source
examples/            Copyable command examples
tools/               Export and check tools
```

A repository that uses Cairn has this lifecycle layout:

```text
.cairn/specs/         Accepted specifications
.cairn/changes/       Active changes
.cairn/archive/       Archived changes
```

`cairn init` and `cairn change create` create native artifacts. Only `migrate openspec` reads legacy OpenSpec content.

## Quick start

Run Cairn from the public flake:

```bash
export REPO=/path/to/your/repo
nix run github:OnixResearch/cairn#cairn -- init --root "$REPO"
nix run github:OnixResearch/cairn#cairn -- validate --root "$REPO"
```

Pin a tag or commit for repeatable automation.

For local development, run the source checkout:

```bash
export CAIRN_SOURCE=$PWD
export REPO=/path/to/your/repo

nix run path:$CAIRN_SOURCE#cairn -- init --root "$REPO"
nix run path:$CAIRN_SOURCE#cairn -- template list --root "$REPO"
nix run path:$CAIRN_SOURCE#cairn -- template apply generic-cli --root "$REPO"
nix run path:$CAIRN_SOURCE#cairn -- template apply generic-cli --root "$REPO" --execute
nix run path:$CAIRN_SOURCE#cairn -- scaffold change add-friendly-start \
  --root "$REPO" \
  --request "Add a friendly start path for first-time users." \
  --execute
nix run path:$CAIRN_SOURCE#cairn -- validate --root "$REPO"
nix run path:$CAIRN_SOURCE#cairn -- gate proposal add-friendly-start --root "$REPO"
nix run path:$CAIRN_SOURCE#cairn -- gate design add-friendly-start --root "$REPO"
nix run path:$CAIRN_SOURCE#cairn -- gate tasks add-friendly-start --root "$REPO"
```

Templates, scaffolds, sync, archive, and migration are preview-first. Add `--execute` only after you review the plan.

Read the [Make a Cairn tutorial](docs/tutorials/make-a-cairn.md) or the [copyable example](examples/make-a-cairn/README.md).

Direct CLI agents can use `cairn --machine <command> ...` for one versioned JSON outcome.
Read the [CLI machine contract](docs/cli-machine-contract.md) before you depend on its status and failure rules.

External CI consumers can bind one native gate receipt to an exact candidate, policy, evidence set, and tool cohort. Read the [gate interchange contract](docs/gate-interchange.md) before you consume this boundary.

Select a policy-owned workflow profile when you create a change:

```bash
nix run path:$PWD#cairn -- change create NAME --profile spec-driven --root .
nix run path:$PWD#cairn -- change create NAME --profile assurance --root .
nix run path:$PWD#cairn -- change create NAME --profile no-spec-delta --root .
```

Inspect policy-owned artifact readiness and instructions without mutation:

```bash
nix run path:$PWD#cairn -- workflow status CHANGE --root .
nix run path:$PWD#cairn -- workflow instructions ARTIFACT --change CHANGE --root .
nix run path:$PWD#cairn -- context --root . --profile assurance --artifact tasks
```

Read [Workflow profiles](docs/workflow-profiles.md) for selection, obligations, review evidence, sync, archive, compatibility, and non-claims.
Read [Artifact workflow inspection](docs/artifact-workflow.md) for states, fields, MCP parity, and claim boundaries.
Read [Agent guidance context](docs/agent-guidance-context.md) before you declare local context or advisory text.
Read [Generated agent workflows](docs/generated-agent-workflows.md) before you install project-local agent guidance.
Read [Developer-loop profiles](docs/developer-loop-profiles.md) before you define or verify a fast local loop.
Read [RFD collaboration](docs/rfd-collaboration.md) before you use the RFD document commands.
Read [ADR collaboration](docs/adr-collaboration.md) before you use the ADR document commands.
Read [Verification-obligation collaboration](docs/verification-obligation.md) before you use the verification-obligation commands.
Read [Release-decision collaboration](docs/release-decision.md) before you use the release-decision commands.
Read [Policy-exception collaboration](docs/policy-exception.md) before you use the policy-exception commands.

<!-- r[impl cairn.artifact_workflow_contract.docs] -->
<!-- r[impl cairn.lifecycle_workflow_profiles.docs] -->
<!-- r[impl cairn.developer_loop_profiles.docs] -->
<!-- r[impl cairn.agent_guidance_context.docs] -->

## Local checks

```bash
nix run path:$PWD#cairn -- policy export --check
cargo fmt --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo tigerstyle check --output-format json --artifact-dir artifacts/slotcar-run
nix run path:$PWD#cairn -- validate --root .
nix flake check path:$PWD -L
nix run path:$PWD#cairn -- doctor --root fixtures/basic
nix run path:$PWD#cairn -- status --root fixtures/basic
```

`nix flake check` covers policy freshness, formatting, tests, lifecycle checks, traceability, Clippy, Slotcar, and self-dogfood.

Cairn keeps maintained gates in Rust and Nickel. It does not use checked-in Python or shell helpers for these checks.

## Requirements and traceability

Cairn uses an EARS-style requirement form. Each requirement has a stable `r[...]` ID and a normative word.

Accepted normative words are `MUST`, `SHALL`, `SHOULD`, and `MAY`. Scenario blocks use `GIVEN`, `WHEN`, `THEN`, and optional `AND` lines.

```markdown
### Requirement: Semantic review CLI
r[cairn.semantic_review.semantic_review_receipts.cli] The system MUST expose semantic review receipt validation through a deterministic CLI command.

#### Scenario: Valid receipt passes
r[cairn.semantic_review.semantic_review_receipts.cli.cli.valid]
- GIVEN a valid semantic review receipt
- WHEN the operator runs `cairn review validate <receipt>`
- THEN Cairn emits a valid deterministic validation receipt
```

Read the original EARS paper at [DOI 10.1109/RE.2009.9](https://doi.org/10.1109/RE.2009.9).

Run the maintained traceability gate:

```bash
cairn traceability coverage --root .
cairn traceability coverage --root . --json
```

`cairn tracey coverage --root .` remains a compatibility alias. External Tracey commands are optional diagnostics.

The marker core parses `r[...]`, `r[impl ...]`, `r[verify ...]`, `r[depends ...]`, and `r[related ...]`.

A passing marker result proves syntax, vocabulary, and declared linkage only. It does not prove requirement satisfaction or implementation correctness.

Export, verify, and query a content-bound trace index:

```bash
cairn traceability export --root . \
  --registry target/requirement-registry.json \
  --out target/trace-index.json
cairn traceability verify --root . \
  --registry target/requirement-registry.json \
  --index target/trace-index.json
cairn traceability show cairn.trace_index.query \
  --index target/trace-index.json
```

Read [Content-bound trace index](docs/content-bound-trace-index.md) for identities, workflow, freshness checks, retained live-proof receipts, and non-claims.

<!-- r[impl cairn.trace_index.docs] -->

## Policy and deterministic enforcement

Nickel policy source is in `cairn-policy/`. Runtime commands do not run Nickel.

During development, `cairn policy export` checks the contracts and writes `cairn-policy/generated/cairn-policy.json`.

Run this command before you trust a policy change:

```bash
nix run path:$PWD#cairn -- policy export --check
```

Every deterministic decision follows this model:

1. Export reviewed policy and make sure that generated JSON is current.
2. Read a bounded repository snapshot.
3. Compute `input_hash` and `policy_hash`.
4. Evaluate the snapshot in the pure core.
5. Emit a receipt with issues, verdict, and hashes.
6. Recheck blockers before mutation.
7. Let CI or operator policy reject failed or stale receipts.

Cairn can block its own mutation tools. It cannot stop direct file edits outside Cairn.

To make Cairn mandatory, require passing receipts in CI before merge or release.

## Evidence boundaries

Cairn evidence records bounded facts. A receipt binds content, policy, tool result, and a receipt hash.

A receipt does not prove that the tool is correct. Traceability does not prove behavior, and tests do not prove all cases.

The reusable lifecycle receipt core checks envelope shape, diagnostics, command identity, hash profiles, receipt hashes, and non-claims.

The Preserves ingest core checks carrier identity, role, scope, schema, producer, and non-claims. Opaque payloads remain opaque.

The evidence-chain core checks typed links through an explicit profile registry. Optional present evidence fails closed when invalid.

The ast-grep gate checks saved scan and test receipts. Pure code never runs ast-grep.

Runtime evidence uses Cairn-owned BLAKE3 identities. A local action chain proves only presented-sequence integrity.

Read [Nominal evidence identities](docs/nominal-evidence-identities.md) for typed roles, wire compatibility, and non-claims.

A matching witness checkpoint proves only a bounded witnessed prefix. Cairn does not decide social, legal, or organizational trust.

### Stable receipt compatibility

Stable public gate fields include `change`, `stage`, `verdict`, `valid`, issues, and receipt hashes.

Stable public lifecycle
receipt fields include the command result, actions, policy identity, and receipt identity. The sync receipts also include `delta_specs`.

Stable public migration receipt
fields include `plan_hash`, source facts, and destination facts. The contract keeps `source_hash` and `destination_hash` fields.

Stable public determinism replay fields include case results, selected cases, selected groups, and receipt identities. Mismatch entries expose stable `case`, `comparison`, `field_path`, `class` fields.

Replay groups are policy-owned named suites. Receipts include `available_groups`, `selected_groups`, `entropy_inventory`, `entropy_assumptions`, `entropy_policy_hash`, and `environment_attestation`.

The attestation includes `attestation_hash`. It does not expose raw environment values, usernames, or host paths.

Stable public replay verification fields come from `cairn determinism verify-receipt <receipt.json> --root .`.

Stable public agent evidence bundle verification fields come from `cairn agent-receipt verify-bundle <bundle.json> --root .`.

Rejected
entropy failures disclose source names and handling but never environment values.

Stable public agent receipt validation fields include `current_replay_policy_hash`, `required_replay_group`, `required_replay_cases`, `replay_evidence`, and `replay_selected_groups`.

Run the agent suite with `cairn determinism replay --group agent`.

<!--
r[verify cairn.runtime_evidence.docs.claims]
r[verify cairn.runtime_evidence.docs.compatibility]
r[verify cairn.release_evidence.lifecycle_receipt_core.docs]
r[verify cairn.preserves_evidence_ingest.docs]
r[verify cairn.preserves_evidence_ingest.docs.non_claims]
r[verify cairn.stack_provenance_gates.evidence_chain_readiness.validation]
r[verify cairn.ast_grep_gate.shell_boundary]
-->

## Structured lifecycle and probes

Nickel contracts can validate structured lifecycle bundles. `cairn lifecycle export` writes or checks canonical JSON bundles.

Markdown remains a compatibility view. Use `cairn lifecycle render-markdown` and `cairn lifecycle import-markdown` for that boundary.

Gates do not run probe commands by default. Run probes first, save their receipts, and then run gates.

```bash
cairn probe run --out target/probe-receipts
cairn gate proposal my-change --root .
```

Kani uses the same saved-evidence boundary. Read [`docs/kani-receipt-gates.md`](docs/kani-receipt-gates.md).

Nickel owns authored structure. Generated JSON is the current runtime format.

Preserves is a future canonical candidate. Steel can become a bounded probe DSL, but it is not lifecycle source.

<!--
r[impl cairn.structured_lifecycle.typed_authoring]
r[impl cairn.structured_lifecycle.runtime_bundle]
r[impl cairn.structured_lifecycle.markdown_projection]
r[impl cairn.structured_lifecycle.migration]
r[impl cairn.structured_lifecycle.format_strategy]
r[impl cairn.probe_execution_boundary.default_gate_no_execution]
r[impl cairn.probe_execution_boundary.explicit_probe_run]
r[impl cairn.probe_execution_boundary.gates_verify_receipts]
r[impl cairn.probe_execution_boundary.inline_probe_mode]
r[impl cairn.probe_execution_boundary.fail_closed]
r[verify cairn.structured_lifecycle.format_strategy.nickel_authoring]
r[verify cairn.structured_lifecycle.format_strategy.generated_json]
r[verify cairn.structured_lifecycle.format_strategy.preserves_candidate]
r[verify cairn.structured_lifecycle.format_strategy.steel_not_source]
r[verify cairn.structured_lifecycle.format_strategy.steel_probe_dsl]
-->

## Collaboration backends

Cairn collaboration semantics do not require Radicle. The neutral state uses BLAKE3 identities over lifecycle meaning, not provider object IDs.

### Portable JSON

The `portable-json.v1` backend needs no profile, node, or repository ID.

```bash
cairn collaboration import my-change \
  --backend portable-json.v1 \
  --store /path/to/my-change.cairn.json \
  --workspace example/workspace \
  --base-revision opaque:review-one \
  --actor operator-label \
  --root /path/to/repo [--execute]

cairn collaboration status my-change \
  --backend portable-json.v1 \
  --store /path/to/my-change.cairn.json \
  --root /path/to/repo

cairn collaboration update my-change \
  --backend portable-json.v1 \
  --store /path/to/my-change.cairn.json \
  --actor operator-label \
  --root /path/to/repo [--execute]

cairn collaboration conflicts my-change \
  --backend portable-json.v1 \
  --store /path/to/my-change.cairn.json \
  --root /path/to/repo [--show-head proposal=REVISION]

cairn collaboration resolve my-change \
  --backend portable-json.v1 \
  --store /path/to/my-change.cairn.json \
  --actor operator-label \
  --choose-head proposal=REVISION \
  --root /path/to/repo [--lifecycle open] [--execute]
```

Import, update, checkout, and resolution are preview-first. Conflicts require complete and explicit current-head choices.

Actor and workspace values are declarations. They are not signatures or remote authority.

### Radicle COB format

The `radicle-cob.v1` adapter uses an explicit Git repository and keystore. It does not run a Radicle node.

```bash
cairn collaboration import my-change \
  --backend radicle-cob.v1 \
  --git-repository /path/to/git/repository \
  --keystore /path/to/ssh-keystore \
  --workspace workspace-id \
  --delegate z6MkDelegatePublicKey \
  --root /path/to/repo [--execute]

cairn collaboration status my-change \
  --backend radicle-cob.v1 \
  --git-repository /path/to/git/repository \
  --root /path/to/repo

cairn collaboration update my-change \
  --backend radicle-cob.v1 \
  --git-repository /path/to/git/repository \
  --keystore /path/to/ssh-keystore \
  --root /path/to/repo [--execute]
```

The selected Git repository must have local `user.name` and `user.email` values. These display values grant no authority.

The adapter has no discovery, seeding, announce, fetch, sync, profile, or peer behavior. External Git transport remains outside Cairn.

Run the format compatibility gate:

```bash
nix build path:$PWD#checks.x86_64-linux.radicle-cob-format-compatibility --no-link -L
```

Passing collaboration receipts prove only local state, selected authority facts, and provider mutation results.

<!--
r[impl cairn.collaboration_backend.semantic_core]
r[impl cairn.collaboration_resolution.inspection]
r[impl cairn.collaboration_resolution.plan]
r[impl cairn.collaboration_resolution.portable_execution]
r[verify cairn.collaboration_resolution.portable_execution.success]
r[verify cairn.collaboration_resolution.portable_execution.blocked]
r[impl cairn.collaboration_update.portable_execution]
r[verify cairn.collaboration_update.portable_execution.success]
r[verify cairn.collaboration_update.portable_execution.blocked]
r[impl cairn.collaboration_backend.portable_state]
r[impl cairn.collaboration_backend.portable_shell]
r[impl cairn.collaboration_backend.cli]
r[impl cairn.collaboration_backend.receipts]
r[verify cairn.collaboration_backend.non_claims.provider]
r[impl cairn.radicle_cob_only.store]
r[verify cairn.radicle_cob_only.store.roundtrip]
r[verify cairn.radicle_cob_only.store.negative]
r[impl cairn.radicle_cob_only.authority]
r[verify cairn.radicle_cob_only.authority.allowed]
r[verify cairn.radicle_cob_only.authority.denied]
r[verify cairn.radicle_cob_only.authority.bounded]
r[impl cairn.radicle_cob_only.cli]
r[verify cairn.radicle_cob_only.cli.explicit]
r[verify cairn.radicle_cob_only.cli.reject_runtime]
r[impl cairn.cob_interop.boundary]
r[verify cairn.cob_interop.boundary.offline]
r[impl cairn.radicle_cob_only.no_runtime]
r[verify cairn.radicle_cob_only.no_runtime.sync]
r[verify cairn.radicle_cob_only.no_runtime.artifacts]
r[impl cairn.radicle_cob_only.compatibility]
r[verify cairn.radicle_cob_only.compatibility.legacy]
r[verify cairn.radicle_cob_only.compatibility.git_refs]
r[verify cairn.radicle_cob_only.compatibility.signatures]
r[verify cairn.radicle_cob_only.compatibility.malformed]
r[impl cairn.radicle_cob_only.dependencies]
r[verify cairn.radicle_cob_only.dependencies.excludes_runtime]
r[impl cairn.radicle_cob_only.docs]
r[impl cairn.collaboration_resolution.receipts]
r[verify cairn.collaboration_resolution.receipts.modes]
r[verify cairn.collaboration_resolution.non_claims.local]
r[impl cairn.collaboration_update.plan]
r[verify cairn.collaboration_update.plan.exact]
r[verify cairn.collaboration_update.plan.noop]
r[verify cairn.collaboration_update.plan.blocked]
r[impl cairn.collaboration_update.receipts]
r[verify cairn.collaboration_update.receipts.modes]
r[verify cairn.collaboration_update.non_claims.local]
-->

## Views and onboarding

Cairn can render lifecycle facts as a kanban board, mind map, or graph. JSON is the deterministic default.

Markdown, Mermaid, DOT, and the read-only terminal viewer are explicit views. Display coordinates never enter receipt hashes.

```bash
cairn view kanban --root . --format markdown
cairn view mindmap --root . --format mermaid
cairn view graph --root . --interactive
```

Read [`docs/lifecycle-views.md`](docs/lifecycle-views.md).

Templates create quick-start files but do not create accepted specs or remote state.

`scaffold change` creates deterministic proposal, design, task, and delta-spec drafts. It does not call a model or network service.

`discover-specs` inventories selected local files and emits review-needed candidates. Candidates are not accepted requirements or implementation proof.

<!--
r[impl cairn.lifecycle_views.docs]
r[verify cairn.lifecycle_views.docs.boundary]
r[impl cairn.developer_experience.onboarding_templates.quickstart]
r[impl cairn.developer_experience.onboarding_template_catalog_expansion.docs]
r[impl cairn.developer_experience.prompt_to_change_scaffold.docs]
r[impl cairn.developer_experience.reverse_spec_discovery.docs]
r[impl cairn.developer_experience.reverse_spec_directory_inventory.docs]
-->

## Change dependencies and review receipts

A change can declare local dependencies in `metadata.json`. Cairn rejects unknown changes, duplicates, self-dependencies, and cycles.

```bash
cairn change depend list demo --root fixtures/basic
cairn change depend add demo prerequisite --root fixtures/basic
cairn change graph --root fixtures/basic --format mermaid
cairn change next --root fixtures/basic
```

Archived dependencies count as satisfied. Active dependencies block sync and archive.

A task list can declare local order with `[task:<id>]` and `[after:<task-id>]`:

```markdown
- [x] [serial] [task:core] Implement the core. r[example.core]
- [ ] [serial] [task:cli] [after:core] Add the CLI. r[example.cli]
```

Ordering stays inactive when both markers are absent. If ordering is active, every checkbox task needs exactly one task identity.

Cairn rejects malformed markers, missing tasks, duplicates, self-dependencies, cycles, and progress before predecessor completion. Task-gate and drain-plan receipts report stable order and ready tasks.

The existing `[depends:...]` marker keeps its lifecycle-link meaning. It does not create a local task-order edge.

Review receipt commands check structure, hashes, and policy vocabulary. They do not run reviewers or interpret model output.

```bash
cairn review profiles
cairn review validate review.json --root .
cairn review ledger --include reviews --root .
cairn review revision validate revision.json --root .
```

Read [`docs/preserves-receipts.md`](docs/preserves-receipts.md), [`docs/preserves-dependency-bumps.md`](docs/preserves-dependency-bumps.md), and [`docs/ai-assisted-draft-provenance.md`](docs/ai-assisted-draft-provenance.md).

<!--
r[impl cairn.change_dependency_graph.docs.discoverability]
r[verify cairn.change_dependency_graph.docs.discoverability.local]
r[verify cairn.change_dependency_graph.docs.discoverability.boundary]
r[impl cairn.task_ordering.markers]
r[impl cairn.task_ordering.drain]
r[impl cairn.determinism.preserves_operator_docs.discoverability]
r[impl cairn.determinism.preserves_dependency_bump_runbook.discoverability]
r[impl cairn.semantic_review.semantic_review_receipts.docs]
r[impl cairn.semantic_review.semantic_review_receipts.docs.docs.non_claims]
r[impl cairn.semantic_review.semantic_review_boundary.boundary.boundary.claims]
-->

## Sync and archive

`sync` and `archive` use dry-run mode by default. Execution rechecks the selected gate and writes a BLAKE3 mutation manifest.

```bash
nix run path:$PWD#cairn -- sync ready --root /path/to/repo --execute
nix run path:$PWD#cairn -- archive ready --root /path/to/repo --execute
```

Sync merges delta requirements into accepted specifications. It preserves the accepted title, purpose, and requirements wrapper.

## Lossless archive compaction

Compaction is a separate preview-first operation:

```bash
nix run path:$PWD#cairn -- compact archive --root . --profile lossless-pack --out target/compact-plan.json
nix run path:$PWD#cairn -- compact archive --root . --plan target/compact-plan.json --execute
nix run path:$PWD#cairn -- compact verify --root .
nix run path:$PWD#cairn -- compact unpack --root . --archive 2026-05-18-example-change
nix run path:$PWD#cairn -- compact unpack --root . --archive 2026-05-18-example-change --execute
```

The lossless pack stores raw objects under BLAKE3 identities and retains canonical manifests and pointers.

A hash-chained ledger detects changed, inserted, removed, or reordered operations inside the observed ledger.

Only `lossless-pack` can execute. Cairn rejects `lossy-summary` unless a later policy adds explicit irreversible behavior and non-claims.

A passing compaction result proves local representation integrity only.

<!--
r[impl cairn.sync_archive.compaction.docs]
r[impl cairn.sync_archive.compaction.docs.chain_merkle]
r[impl cairn.sync_archive.compaction.docs.non_claims]
-->

## Determinism replay and agent receipts

Run the read-only replay preflight:

```bash
cairn determinism replay --root . --group agent --format summary --out target/replay.json
cairn determinism verify-receipt target/replay.json --root .
```

Policy owns replay cases, groups, comparison modes, display fields, and entropy handling.

Replay rejects hidden entropy, unexpected mutation, stale policy, and output mismatch. Its environment attestation omits raw environment values and host-specific paths.

Agent wrappers can bind the replay receipt into an agent-run receipt:

```bash
cairn agent-receipt validate agent.json
cairn agent-receipt verify-bundle bundle.json --root .
```

The core checks receipt shape and replay coverage. It does not prove agent correctness.

<!--
r[verify cairn.supersede.replay_receipt_verification.docs]
r[verify cairn.release_evidence.cap_std_boundary.docs]
r[verify cairn.supersede.agent_run_receipts.schema]
r[verify cairn.supersede.agent_receipt_replay_evidence.docs]
r[verify cairn.supersede.agent_receipt_replay_group_coverage.docs]
r[verify cairn.supersede.agent_evidence_bundle_verification.docs]
r[verify cairn.supersede.agent_run_receipts.completion_gate]
-->

## Evidence publication

Export and check a portable evidence bundle:

```bash
cairn evidence export --root . --out target/cairn-evidence/
cairn evidence verify --root . --bundle target/cairn-evidence/
cairn evidence import --root . --bundle target/cairn-evidence/ --into target/cairn-imported-evidence/
cairn evidence republish --root . --imported target/cairn-imported-evidence/ --out target/cairn-republished-evidence/
```

Each normal evidence export includes `requirement-registry.json`, `trace-index.json`, and `trace-index-verification.json`. The manifest binds the trace index to the exact registry. Evidence verification also compares the saved index with the current root.

Cairn preserves these artifacts through import, republish, indexing, registry selection, mirror verification, and local fetch workflows.

These commands do not perform network transport or credential handling. They do not establish remote trust. Trace artifacts record declared linkage and freshness only. They do not prove requirement satisfaction or implementation correctness.

<!-- r[impl cairn.trace_index.evidence_bundle.docs] -->

Generate release evidence for a clean current revision:

```bash
cairn release-evidence index --root . --out target/release-evidence-current-head/index.md --artifact-dir target/release-evidence-current-head
cairn release-evidence check-index --root . --index target/release-evidence-current-head/index.md --mode current-head
```

Cairn can also materialize local publication trees and plan local retention. It does not create tags, upload releases, or delete remote evidence.

Read [`docs/agent-run-receipts.md`](docs/agent-run-receipts.md) for receipt fields, privacy rules, and full commands.

<!--
r[cairn.supersede.fetch_execute_policy_docs.docs.flow]
r[cairn.supersede.fetch_execute_policy_docs.docs.non_claims]
r[verify cairn.supersede.signed_evidence_manifest.docs]
r[verify cairn.supersede.evidence_manifest_generate.docs]
r[verify cairn.supersede.determinism_coverage_release_evidence.release]
r[verify cairn.supersede.determinism_coverage_release_evidence.bundle_manifest]
r[verify cairn.supersede.determinism_coverage_release_evidence.docs]
r[verify cairn.supersede.determinism_coverage_release_evidence.archive]
r[impl release_evidence.automated_current_head_index.operator_docs]
r[verify release_evidence.automated_current_head_index.operator_docs]
r[verify release_evidence.automated_current_head_index.operator_docs.non_claims]
r[impl release_evidence.publish_materialization.operator_docs]
r[verify release_evidence.publish_materialization.operator_docs]
r[verify release_evidence.publish_materialization.operator_docs.non_claims]
r[impl release_evidence.retention_rotation.operator_docs]
r[verify release_evidence.retention_rotation.operator_docs]
r[verify release_evidence.retention_rotation.operator_docs.non_claims]
r[verify release_evidence.current_head_staleness_guard.operator_docs]
r[verify release_evidence.current_head_staleness_guard.archive_ready]
r[verify release_evidence.current_head_evidence_index.behavior_preserved.focused_checks]
r[impl release_evidence.current_head_staleness_guard.operator_docs]
-->

## MCP server

`cairn mcp` exposes bounded JSON-RPC tools. The default server includes read-only inspection and dry-run planning only.

Start it with `--allow-mutations` to expose mutation tools.

```bash
cairn mcp --describe
cairn mcp --call <tool> '<json>'
cairn mcp --allow-mutations
```

Read [`docs/cairn-mcp.md`](docs/cairn-mcp.md).

<!-- r[verify cairn.supersede.mcp_server.verification.docs] -->

## Import legacy content

Migration uses dry-run mode by default. It never overwrites existing native files.

Review the preview and copy its `plan_hash`. Execution requires that exact hash. Cairn rejects source, destination, action, or policy drift before mutation.

```bash
cairn migrate cairn --root PATH
cairn migrate cairn --root PATH --execute --plan-hash HASH
cairn migrate openspec --root PATH
cairn migrate openspec --root PATH --execute --plan-hash HASH
```

Normal lifecycle commands reject a legacy-only `cairn/` root. Run the explicit migration preview before normal lifecycle work.

## Agent workflow source

`docs/hermes/cairn-workflow-skill.md` contains the repository-maintained agent workflow. It documents native lifecycle, boundaries, checks, commands, and known errors.

Cairn also generates bounded, project-local workflows for Agent Skills, Claude, and Codex. Generation uses the checked policy projection without network or model calls.

```bash
cairn agent workflow list
cairn agent workflow install --root . --adapter agent-skills --destination .agents/skills
```

Installation is preview-first. Add `--execute --plan-hash <PLAN_HASH>` only after you review the exact paths and BLAKE3 identities.

## License

`cairn-core` and reusable templates use `MIT OR Apache-2.0`. The `cairn-cli` shell uses `AGPL-3.0-or-later`.

Complete texts are in [LICENSES](LICENSES). The package map is in [docs/licensing.md](docs/licensing.md).

The license split does not turn lifecycle conformance into code-correctness or release evidence.

## References

- [`../rats/subwayrat/`](https://github.com/brittonr/subwayrat) — read-only lifecycle viewer components.
- `../trellis/` — verified graph and dependency model reference.
- [oxidecomputer/steno](https://github.com/oxidecomputer/steno) — saga and recovery-log reference.
- [oxidecomputer/rfd-api](https://github.com/oxidecomputer/rfd-api) and [oxidecomputer/rfd-site](https://github.com/oxidecomputer/rfd-site) — lifecycle document reference.
- [oxidecomputer/expectorate](https://github.com/oxidecomputer/expectorate) — deterministic golden-update reference.
- [oxidecomputer/bundle-cat](https://github.com/oxidecomputer/bundle-cat) and [oxidecomputer/support-bundle-viewer](https://github.com/oxidecomputer/support-bundle-viewer) — read-only bundle reference.
- [joweeba/dpub](https://github.com/joweeba/dpub) — publication ledger reference.
- [joweeba/tla2](https://github.com/joweeba/tla2) — state-machine receipt reference.
- [`radicle-cob`](https://crates.io/crates/radicle-cob) — signed Git-backed object format.
- [`radicle`](https://crates.io/crates/radicle) — fixture-only upstream Git storage adapter.
- [Fission-AI/OpenSpec stores beta guide](https://github.com/Fission-AI/OpenSpec/blob/main/docs/stores-beta/user-guide.md) — standalone planning-store, reference, context, and workset interaction reference.
- [OnixResearch/Ambient](https://github.com/OnixResearch/ambient) — content identity, source occurrence, canonical snapshot, and deterministic diff implementation reference.
- [Unison](https://github.com/unisonweb/unison) — immutable definition identity, mutable naming, recursive component, and dependency identity model reference.

These references do not transfer their domain-specific correctness or authority claims into Cairn.
