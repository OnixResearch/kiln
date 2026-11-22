# Traceability coverage of the convergence and foreign-realize specs

`cairn traceability coverage --root . --policy "$PWD/cairn-policy/generated/cairn-policy.json" --json`
reported 33 missing IDs before this change, all from the archived changes
`2026-11-20-admit-cairn-convergence-campaigns` and
`2026-11-21-realize-convergence-evidence-through-mantle`. The accepted specs
`.cairn/specs/convergence-campaign/spec.md` and
`.cairn/specs/mantle-foreign-realize-adapter/spec.md` are unchanged; this change
adds implementation and verification markers only. Coverage now reports
`valid: true` with zero missing and zero dangling
(`evidence/traceability-coverage.txt`).

## Implementation bindings

- `crates/kiln-adapter-cairn/src/convergence.rs` —
  `admit_convergence_receipt` owns bounded intake.
- `crates/kiln-core/src/convergence.rs` — `select_unsatisfied`,
  `mint_evidence_campaign`, `plan_evidence_builds` and
  `EvidenceBuildRequest::into_build_effect_plan`, which also carries
  `kiln.convergence.nonclaims` as the only lowering to an effect.
- `crates/kiln-adapter-mantle/src/realize.rs` — `binding_for` resolves the
  plan-keyed realize binding, `dispatch_inner` owns the bounded dispatch, and
  `admit_realization_receipt` owns closed-world completion.

## Verification bindings

| Requirement ID | Test |
|---|---|
| `kiln.convergence.intake.scenario.pilot` | `admits_the_retained_pilot_receipt` |
| `kiln.convergence.intake.scenario.schema` | `rejects_unknown_schema`, `rejects_unknown_top_level_or_row_field`, `rejects_unknown_state_after_identity_holds`, `rejects_an_unknown_bound_field` |
| `kiln.convergence.intake.scenario.identity` | `rejects_a_tampered_receipt_identity`, `rejects_a_stale_declared_binding`, `rejects_a_non_git_receipt_revision` |
| `kiln.convergence.intake.scenario.oversized` | `rejects_oversized_input_before_parsing` |
| `kiln.convergence.intake.scenario.current` | `current_and_untargeted_rows_are_not_acceptance_or_work` |
| `kiln.convergence.unsatisfied.scenario.selection` | `selects_only_unsatisfied_targeted_rows_in_registry_order`, `exclusions_retain_their_reason` |
| `kiln.convergence.unsatisfied.scenario.moved` | `moved_or_unbound_target_rows_stay_selected` (new) |
| `kiln.convergence.unsatisfied.scenario.satisfied` | `satisfied_or_empty_selection_mints_none` |
| `kiln.convergence.campaign.scenario.mint` | `mints_one_frozen_revision_from_the_selection` |
| `kiln.convergence.campaign.scenario.identity` | `repeated_mint_is_identical`, `moved_candidate_or_row_changes_the_campaign_identity`, `changed_item_order_changes_the_campaign_identity`, `candidate_set_identity_binds_the_existing_candidate_value` |
| `kiln.convergence.campaign.scenario.empty` | `empty_rows_are_refused`, `satisfied_or_empty_selection_mints_none`, `a_duplicate_item_is_refused` |
| `kiln.convergence.campaign.scenario.job` | `a_missing_evidence_job_is_refused` |
| `kiln.convergence.verify.scenario.request` | `plans_one_typed_request_per_item`, `requests_are_deterministic_and_lower_to_build_plans`, `a_platform_change_changes_the_request_identity` |
| `kiln.convergence.verify.scenario.capability` | `provider_without_dispatch_is_unsupported`, `another_role_or_profile_is_refused`, `a_missing_outcome_class_is_refused` |
| `kiln.convergence.verify.scenario.realization` | `preserve_realization_fails_before_effects` |
| `kiln.convergence.verify.scenario.realization_mismatch` | `build_realization_must_not_select_a_realize_profile` |
| `kiln.convergence.verify.scenario.plan` | `a_missing_plan_is_refused`, `an_unexpected_plan_is_refused`, `duplicate_empty_and_unsorted_products_are_refused` |
| `kiln.convergence.nonclaims.scenario.advisory` | `complete_record_and_planned_request_grant_no_release` (new) |
| `kiln.convergence.realize` | `preserve_realization_is_admitted_only_with_the_declared_capability` |
| `kiln.convergence.realize.scenario.pilot` | `pilot_receipt_is_admitted` |
| `kiln.convergence.realize.scenario.substitution` | `each_bound_receipt_identity_is_required`, `a_tampered_digest_is_refused` |
| `kiln.mantle.foreign_realize.profile.scenario.match` | `pinned_realize_profile_is_admitted` (new) |
| `kiln.mantle.foreign_realize.profile.scenario.drift` | `profile_admission_refuses_cohort_drift` |
| `kiln.mantle.foreign_realize.dispatch` | `profile_or_plan_mismatch_is_refused` |
| `kiln.mantle.foreign_realize.dispatch.scenario.command` | `dispatch_runs_the_exact_reviewed_command` |
| `kiln.mantle.foreign_realize.dispatch.scenario.identity` | `stale_profile_and_input_identities_stop_before_dispatch` |
| `kiln.mantle.foreign_realize.completion.scenario.pilot` | `pilot_receipt_is_admitted` |
| `kiln.mantle.foreign_realize.completion.scenario.drift` | `unknown_and_missing_fields_are_cohort_drift`, `wrong_schema_is_refused`, `incomplete_status_is_refused`, `a_tampered_digest_is_refused`, `each_bound_receipt_identity_is_required`, `zero_or_two_roots_are_refused`, `a_root_outside_the_prefix_is_refused`, `an_unmapped_or_misplaced_product_is_refused` |
| `kiln.mantle.foreign_realize.completion.scenario.oversized` | `oversized_receipt_is_a_bounded_refusal`, `malformed_receipt_becomes_a_provider_error` |
| `kiln.mantle.foreign_realize.recovery.scenario.replay` | `replayed_dispatch_returns_the_recorded_observation`, `reconciliation_refuses_a_changed_plan`, `restore_replays_only_the_exact_observation` |
| `kiln.mantle.foreign_realize.recovery.scenario.classes` | `timeout_cancellation_and_unknown_keep_their_classes`, `failure_envelope_is_a_distinct_failed_observation` |
| `kiln.mantle.foreign_realize.nonclaims.scenario.release` | `complete_realization_grants_no_release_authority` (new) |

## Non-claims

Coverage proves that each accepted ID names an implementation site and a test
site. It does not prove the test is exhaustive, the implementation correct, or
the accepted requirement sufficient. No release, publication or merge authority
follows.
