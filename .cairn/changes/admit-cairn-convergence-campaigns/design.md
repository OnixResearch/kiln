## Context

Cairn origin/main `231d61e` documents and emits `cairn.convergence.v1`. The
receipt lists every registry row in registry order with one of eight states:
`untargeted`, `not_projected`, `current`, `unrealized`, `stale`, `failed`,
`unsupported` or `incomplete`. Only Octet's decision is acceptance; a Mantle
success, a cache hit and the relay value `proof_acceptance` never are. The
retained pilot receipt at
`/home/brittonr/wt/convergence-evidence/e2e/step6/receipt-retained.json`
contains 740 rows: one `current` checked-proof row and 739 `untargeted` rows.
Its row binds a `cache-only-preserve-v1` realization route, meaning Mantle
preserved a prebuilt closure rather than building it.

Kiln already owns the values this change needs:

- `kiln-core::gating::CandidateSet` and `CandidateId` identify one exact
  candidate set;
- `kiln-core::build::BuildEffectPlan` and `validate_build_effect_plan` are the
  typed build effect and its admission;
- `kiln-core::provider::CapabilityProfile`, `OperationRequest` and
  `admit_operation` decide provider capability before an effect;
- `kiln-adapter-mantle::MantleAdapter` implements `BuildPort` by running one
  bounded `mantle --json build --plan <plan>` command against the pinned
  Mantle cohort `d75158b0`, admitting only `mantle.build-request.v1`,
  `mantle.build-observation.v1`, `mantle.build-receipt.v1` and
  `crunch-build-report-v1`.

The pinned adapter has no `foreign-import realize` path. The phase-1 pilot
evidence was realized on Mantle main `ead7b8dac` through that route, and the
adapter cannot reproduce it. This change therefore maps the Verify stage to the
capability the adapter actually exposes — a fresh plan build — and treats every
preserve/realize request as an unsupported capability that fails before any
effect. Adding a preserve operation is a separate Mantle cohort and profile
change, not part of this delta.

## Decisions

### Decision (a): Intake admits bounded receipt facts, not acceptance

**Choice:** `kiln_adapter_cairn::convergence::admit_convergence_receipt` is a
pure function over already-read bytes. It accepts exactly
`schema_version = "cairn.convergence.v1"` and `status = "evaluated"`, and it
parses a closed model (`deny_unknown_fields` at every level) of:

- receipt: `schema_version`, `status`, `repository_id`, `revision`
  (`kind`/`value`), `inputs` (`case_blake3`, `policy_blake3`,
  `registry_blake3`, `targets_blake3`, `generated_sources_blake3`, `contracts`),
  `rows`, `unbound_target_issues`, `targets_current`, `coverage_complete`,
  `complete`, `authority`, `non_claims`, `receipt_blake3`;
- row: `requirement_id`, `specification_path`, `row_blake3`, `state`,
  `assurance`, `contract_id`, `job_id`, `bound`, `issues`;
- bound: `node_id`, `producer_job_identity`, `realization_route`,
  `receipt_identity`, `root_derivation`, `signer`, `source_revision`,
  `target_derivation`, `target_path`.

States are a closed enum. Optional facts stay optional, so a non-current row
with a partial binding is representable, but any present BLAKE3 field must be
64 hexadecimal characters and `b3:`-prefixed producer identities must carry 64
hexadecimal characters. Kill-identified facts are revalidated after
deserialization.

Named bounds: `MAX_CONVERGENCE_RECEIPT_BYTES = 4 MiB`,
`MAX_CONVERGENCE_ROWS = 4096`, `MAX_CONVERGENCE_ID_BYTES = 128`,
`MAX_CONVERGENCE_PATH_BYTES = 512`, `MAX_CONVERGENCE_TEXT_BYTES = 1024`,
`MAX_CONVERGENCE_ITEMS = 4096`. The byte bound is checked before parsing; the
retained pilot is 231,695 bytes.

Stable issue codes (all `convergence_receipt.` prefixed): `oversized`,
`malformed`, `unknown_schema`, `unknown_status`, `unknown_state`,
`invalid_identity`, `invalid_revision_kind`, `duplicate_requirement`,
`row_bound`, `row_addressing`, `receipt_identity_mismatch`, `binding_mismatch`.
Each returns a `Diagnostic` whose `field` is a stable JSON path root, never a
row index alone.

Identity binding is explicit: the intake takes a
`ConvergenceReceiptBinding { receipt_blake3, revision_kind, revision,
registry_blake3, case_blake3, policy_blake3 }` declared by the caller. Intake
recomputes the canonical JSON of the parsed receipt without `receipt_blake3`,
hashes it with BLAKE3, and requires the recomputed value to equal both the
receipt field and the declared binding. Any other receipt, revision kind or
value, registry, case or policy identity is a stale/substituted binding and is
rejected; no row is admitted. Revision kind `git` is the only admitted kind,
because a Kiln candidate names a Git source snapshot in this profile.

A `current` row is never Kiln acceptance. Intake returns facts and an admitted
identity only; it sets no gate, run, attempt, candidate or provider outcome.

### Decision (b): Unsatisfied set is the five non-current targeted states

**Choice:** `kiln_core::convergence::select_unsatisfied` returns exactly rows
whose state is `unrealized`, `stale`, `failed`, `unsupported` or `incomplete`,
in registry order. `current` is excluded because the declared assurance target
has current bound evidence. `not_projected` is excluded because a `reviewed`
target's row binding holds and no evidence job exists to build.
`untargeted` is excluded because the row has no declared assurance target, so
Kiln has nothing to request. The function returns each exclusion with its
reason (`ExclusionReason`), so the operator sees why a row was not selected
rather than silently dropped. `unbound_target_issues` are carried into the
selection result and never discarded; a non-empty list keeps the repository
incomplete, so the campaign is still a request for the missing evidence, not a
claim that everything else is current.

Selection keeps the row's requirement id, specification path, row BLAKE3,
state, contract id and job id. A selected row without a contract or job id
cannot be requested; minting refuses it with `campaign_item_job_missing`.

### Decision (c): One frozen revision bound to one candidate-set BLAKE3

**Choice:** `kiln_core::convergence::mint_evidence_campaign` accepts a
`ConvergenceCampaignBinding { candidate_id, candidate_set_identity,
receipt_blake3, revision_kind, revision, registry_blake3, case_blake3,
policy_blake3 }` and the selected items, and returns
`Option<EvidenceCampaignRevision>`: `None` when the set is empty, one revision
otherwise. The candidate-set identity is computed by
`candidate_set_identity(&CandidateSet)`, a domain-separated BLAKE3 over the
existing candidate value (id, ordered changes, dependencies, frozen sources,
policy). The campaign therefore names both the existing `CandidateId` and the
exact candidate-set BLAKE3; a moved target or changed policy yields a new
candidate and a new campaign identity.

The revision identity is
`BLAKE3("kiln.evidence-campaign.v1" || length-prefixed binding fields ||
ordered items)`. Items are never reordered by the mint; the receipt already
fixes registry order. Identical facts give an identical identity; any changed
binding, row, state or order gives a different identity. The revision is an
immutable value with no mutation route. `MintRefusal::duplicate_requirement`
and `MintRefusal::empty_rows` keep an empty or forged set from becoming a
campaign. An all-current receipt returns `None` and records no campaign.

The mint is idempotent by value: repeating it over the same facts returns an
equal revision. Kiln owns this identity; the adapter and provider never supply
it.

### Decision (d): Verify requests a fresh plan build, and refuses anything else

**Choice:** `kiln_core::convergence::plan_evidence_builds` takes the frozen
revision, an operator `EvidenceBuildSelection { provider_profile,
policy_identity, pipeline_revision, platform, realization, plans }` where
`plans` maps each campaign job id to `{ plan_identity, required_products }`,
and the selected `CapabilityProfile`. For each item in order it produces one
`EvidenceBuildRequest` carrying:

- `campaign_id`, `candidate_set_identity`, `candidate_id`;
- `requirement_id`, `specification_path`, `row_blake3`, `contract_id`,
  `job_id`;
- `effect_id` and `idempotency_key`, both deterministic BLAKE3-derived from the
  campaign and item binding;
- `plan_identity`, `policy_identity`, `provider_profile`, `platform`,
  `required_products`, `realization`.

Before producing any request it calls `admit_operation` with a build-role
`OperationRequest` for `Operation::Dispatch` and the required outcomes
(`Success`, `Failed`, `Cancelled`, `TimedOut`, `Unknown`). A missing
`Dispatch`, another role, another profile identity, a missing outcome class or
an exceeded request bound is `unsupported_capability` /
`provider_substitution` / `role_mismatch` / `outcome_support_missing` before any
effect. It then requires an exact plan for every campaign job and no extra
plan; a missing, unexpected, duplicate, unsorted or empty product set is
refused without producing a request.

`realization` is `EvidenceRealization::BuildFromPlan` or
`EvidenceRealization::PreserveCacheOnly`. The latter is refused with
`unsupported_capability` at `convergence.verify.realization` because
`kiln-adapter-mantle` runs only `mantle build --plan` and has no
`foreign-import realize` operation. This is the exact mapping the phase-1
pilot needs: its `cache-only-preserve-v1` route is not reproducible through
the pinned adapter and fails before effects.

Each request lowers with `into_build_effect_plan(attempt_id)` to the existing
`BuildEffectPlan`, which calls `validate_build_effect_plan`. The Mantle shell
then dispatches that plan through the existing `BuildPort::dispatch`; no new
provider call, scheduler or claim is invented here.

### Decision (e): Non-claims

**Choice:** An admitted receipt, selected set, minted revision and planned
request prove only that the checked relations between the receipt facts, the
selection rule, the candidate binding and the typed request hold. They do not
prove requirement correctness, evidence truth, build correctness, cache truth,
reproducibility, source honesty, sandbox or verifier soundness, release
eligibility, or that any provider performed an effect. Cairn `current`, Mantle
success and a cache hit stay advisory. No gate result, merge, publication or
release may follow from this record alone.

## Risks / Trade-offs

- The receipt's canonical JSON is reproduced in Kiln. Cairn must keep
  sorted-object-key canonicalization; a change there is a receipt schema
  change and must arrive with a new schema identifier and a new binding, not a
  silent Kiln adjustment.
- The intake admits the full 4096-row bound in memory. The pilot is 740 rows;
  the bound keeps a hostile receipt from exhausting memory while parsing.
- Refusing preserve/realize means Kiln cannot replay phase-1 pilot evidence
  through `kiln-adapter-mantle`. That is intentional and recorded as the
  missing typed operation; adding it needs a new pinned Mantle cohort.
- The campaign is a value, not a scheduler. Durable storage, authorization,
  dispatch and operator controls remain separate changes; this delta must not
  be read as an implemented evidence scheduler.
