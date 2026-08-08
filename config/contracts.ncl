let NonEmptyString = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let Positive = std.contract.from_predicate (fun value =>
  std.is_number value && value > 0
) in
let VersionOne = std.contract.from_predicate (fun value =>
  std.is_number value && value == 1
) in
let FairnessStrategy = std.contract.from_predicate (fun value =>
  std.is_string value && (value == "weighted_scalar" || value == "dominant_share")
) in
let UnknownFactPolicy = std.contract.from_predicate (fun value =>
  std.is_string value && (value == "deny" || value == "block" || value == "retain_unknown")
) in
let MirrorAuthority = std.contract.from_predicate (fun value =>
  std.is_string value && value == "read_only_projection"
) in
let LocalCapacityOnly = std.contract.from_predicate (fun value =>
  std.is_bool value && value == false
) in
{
  Config = {
    version | VersionOne,
    domain | {
      max_identifier_bytes | Positive,
      max_collection_items | Positive,
      max_transition_items | Positive,
      max_diagnostic_bytes | Positive,
    },
    provider | {
      contract_version | VersionOne,
      max_request_bytes | Positive,
      max_result_bytes | Positive,
      max_in_flight | Positive,
      required_outcomes | Array NonEmptyString,
      no_silent_fallback | Bool,
    },
    journal | {
      max_events_per_commit | Positive,
      lease_ticks | Positive,
      retention_ticks | Positive,
      replay_versions | Array Positive,
    },
    fairness | {
      strategy | FairnessStrategy,
      accounting_window_ticks | Positive,
      decay_numerator | Positive,
      decay_denominator | Positive,
      rounding | NonEmptyString,
      stale_usage_policy | UnknownFactPolicy,
      tie_break | NonEmptyString,
      resource_weights | { _ : Positive },
    },
    task_operations | {
      status | { max_records | Positive },
      capacity | {
        max_classes | Positive,
        default_in_flight_limit | Positive,
        observation_max_age_ticks | Positive,
        ordering | NonEmptyString,
        hold_policy | NonEmptyString,
        unsupported_fact_policy | UnknownFactPolicy,
        affects_tenant_fairness | LocalCapacityOnly,
      },
      logs | {
        max_streams | Positive,
        max_segments_per_stream | Positive,
        max_segment_bytes | Positive,
        max_total_bytes | Positive,
        max_read_bytes | Positive,
        retention_ticks | Positive,
        redaction_marker | NonEmptyString,
      },
      follow | {
        heartbeat_ticks | Positive,
        inactivity_ticks | Positive,
        deadline_ticks | Positive,
      },
      wait | {
        max_selection_items | Positive,
        deadline_ticks | Positive,
        predicates | Array NonEmptyString,
      },
      controls | {
        timeout_ticks | Positive,
        operations | Array NonEmptyString,
      },
    },
    hosting | {
      state | NonEmptyString,
      canonical_branch | NonEmptyString,
      rid | String,
      delegates | Array NonEmptyString,
      threshold | Positive,
      required_replicas | Positive,
      license | NonEmptyString,
      rollback_revision | NonEmptyString,
      signed_ref_features | Array NonEmptyString,
      seeds | Array NonEmptyString,
      required_replica_observations | Positive,
      acquisition_routes | Array {
        kind | NonEmptyString,
        route | NonEmptyString,
        writable | Bool,
      },
      freeze | Bool,
      non_claims | Array NonEmptyString,
      mirrors | Array {
        route | NonEmptyString,
        authority | MirrorAuthority,
      },
    },
    cob | {
      max_profiles | Positive,
      max_heads | Positive,
      max_payload_bytes | Positive,
      max_actions | Positive,
      retry_limit | Positive,
      unknown_type_policy | UnknownFactPolicy,
      required_profile_types | Array NonEmptyString,
    },
  },
}
