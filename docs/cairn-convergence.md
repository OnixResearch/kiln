# Cairn convergence campaigns

`kiln-adapter-cairn::convergence` admits one `cairn.convergence.v1` receipt and
`kiln-core::convergence` selects the unsatisfied evidence set, mints one frozen
campaign revision and plans the Verify-stage evidence builds. Cairn owns the
receipt and its states; Kiln owns the selection, the campaign identity and the
CI meaning of the resulting requests.

## Intake

`admit_convergence_receipt(bytes, binding)` is pure: the shell reads the bytes,
the function decides. It admits only:

- `schema_version = "cairn.convergence.v1"` and `status = "evaluated"`;
- a closed typed model (`deny_unknown_fields` at receipt, row and binding
  level) with the eight Cairn row states;
- a Git registry revision (`revision.kind = "git"`);
- the named bounds `MAX_CONVERGENCE_RECEIPT_BYTES` (4 MiB),
  `MAX_CONVERGENCE_ROWS` (4096), `MAX_CONVERGENCE_ID_BYTES` (128) and
  `MAX_CONVERGENCE_TEXT_BYTES` (1024).

The identity binding is explicit. The caller supplies a
`ConvergenceReceiptBinding` naming the receipt BLAKE3, revision kind and value,
registry, case and policy BLAKE3. Intake removes `receipt_blake3`, canonicalizes
the remaining JSON with sorted object keys, hashes it with BLAKE3, and requires
that value to equal both the receipt field and the declared binding. A
substituted receipt, revision, registry, case or policy is rejected and no row
is admitted. Stable issue codes live under `convergence_receipt.`; the full
list is in the module constants.

A `current` row is **not** Kiln acceptance. Intake returns facts and no gate,
run, attempt, candidate or provider outcome; Mantle success, cache hits and the
`proof_acceptance` relay are advisory.

## Unsatisfied set

`select_unsatisfied(rows)` keeps targeted rows whose state is `unrealized`,
`stale`, `failed`, `unsupported` or `incomplete`, in registry order. It
excludes and reports:

| State | Reason |
| --- | --- |
| `current` | current bound evidence exists for the declared target |
| `not_projected` | a reviewed target's row binding holds; no evidence job exists |
| `untargeted` | the row declares no assurance target |

Empty rows, duplicates and oversized rows/text are refused with named codes.
`unbound_target_issues` from the receipt are retained by intake; a non-empty
list keeps the repository incomplete.

## Campaign revision

`mint_evidence_campaign(binding, selection)` returns `None` for an empty set and
otherwise one immutable `EvidenceCampaignRevision`. It binds:

- `candidate_id` and `candidate_set_identity` — the BLAKE3 produced by
  `candidate_set_identity(&CandidateSet)` over the existing candidate value;
- `receipt_blake3`, registry revision, registry BLAKE3, case BLAKE3 and policy
  BLAKE3;
- the ordered items, each with requirement, specification path, row BLAKE3,
  state, contract and evidence job.

`campaign_id` is a domain-separated BLAKE3 over those facts. Identical facts
mint an equal revision; a moved candidate, changed row, changed state or
changed order mints another. An item without a contract or job is refused
because its evidence job cannot be named.

## Verify stage

Verify admits two realizations, each against its own pinned cohort:

- `EvidenceRealization::BuildFromPlan` lowers to the build profile
  (`mantle --json build --plan <plan>`, Mantle `d75158b0`) and
  `mantle.build-request.v1`.
- `EvidenceRealization::PreserveCacheOnly` lowers to the foreign-realize
  profile (`mantle foreign-import realize` on the cache-only preserve route,
  Mantle `4b05f9f6`) and `mantle-foreign-realization-receipt-v1`. See
  [Mantle foreign realize](mantle-foreign-realize.md).

`plan_evidence_builds(revision, selection, capability)` requires the selected
`CapabilityProfile` to declare a `Build`-role `Dispatch` with the outcome
classes Kiln must preserve, and requires an exact plan per campaign job. A
preserve realization additionally requires `Operation::Realize`; a
`BuildFromPlan` selection against a profile that declares `Realize` is refused
with `realization_profile_mismatch`. Both refusals happen before any request or
effect. It produces one deterministic `EvidenceBuildRequest` per item with a
derived `effect_id` and `idempotency_key`. Each request lowers with
`into_build_effect_plan(attempt_id)` to the existing `BuildEffectPlan`, so the
shell uses the existing `BuildPort::dispatch` and the selected adapter's
completion admission.

The phase-1 pilot evidence is realized through the preserve route: the adapter
verifies the frozen plan, import receipt, source bundle, execution profile,
cache-closure policy and endpoint binding by BLAKE3 identity, dispatches one
bounded realization, and admits completion only when the closed-world receipt
binds those identities, selects exactly one root and maps every required
product to it. A campaign that requests preserve evidence without the realize
capability still fails with `unsupported_capability` before effects.

The fixture `fixtures/cairn/convergence/pilot/receipt-retained.json` is the
retained 740-row pilot receipt (one `current`, 739 `untargeted`). Tests admit it
and mutate synthetic copies for every rejection path.

## Non-claims

Admitting a receipt, selecting rows, minting a campaign or planning a Verify
request proves only those checked relations. It does not prove requirement
correctness, evidence truth, build or cache correctness, reproducibility,
source honesty, sandbox soundness, release eligibility or that any provider
performed an effect. No merge, publication, release or provider effect follows
from this record alone. The campaign is a value, not a scheduler; durable
storage, authorization and operator controls remain separate work.
