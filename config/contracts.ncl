let NonEmptyString = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let Positive = std.contract.from_predicate (fun value =>
  std.is_number value && value > 0
) in
let NonNegative = std.contract.from_predicate (fun value =>
  std.is_number value && value >= 0
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
let CDEventsVersion = std.contract.from_predicate (fun value =>
  std.is_string value && value == "0.5.1"
) in
let CDEventsRevision = std.contract.from_predicate (fun value =>
  std.is_string value && value == "df02ce0b6563a13bf35cc109993b03b18da8a14a"
) in
let CloudEventsSpec = std.contract.from_predicate (fun value =>
  std.is_string value && value == "1.0.2"
) in
let CloudEventsContext = std.contract.from_predicate (fun value =>
  std.is_string value && value == "1.0"
) in
let ApacheLicense = std.contract.from_predicate (fun value =>
  std.is_string value && value == "Apache-2.0"
) in
let OtlpVersion = std.contract.from_predicate (fun value =>
  std.is_string value && value == "1.11.0"
) in
let OtlpRevision = std.contract.from_predicate (fun value =>
  std.is_string value && value == "790608c4d51e6ffc12210b541e8514cbed9e91a4"
) in
let SemanticConventionsVersion = std.contract.from_predicate (fun value =>
  std.is_string value && value == "1.43.0"
) in
let SemanticConventionsRevision = std.contract.from_predicate (fun value =>
  std.is_string value && value == "89aae438b3b3b0a8dd33003c9d70592baf7dbd0d"
) in
let ReleaseCandidate = std.contract.from_predicate (fun value =>
  std.is_string value && value == "release_candidate"
) in
let OtlpHttpJson = std.contract.from_predicate (fun value =>
  std.is_string value && value == "otlp_http_json"
) in
let TlsRequired = std.contract.from_predicate (fun value =>
  std.is_bool value && value == true
) in
let TelemetryDropPolicy = std.contract.from_predicate (fun value =>
  std.is_string value && (value == "drop_newest" || value == "reject_batch")
) in
let ValenceRevision = std.contract.from_predicate (fun value =>
  std.is_string value && value == "220307a371e7bc4e3309447b09447d0fe6c4db5f"
) in
let ArtifactAuthRevision = std.contract.from_predicate (fun value =>
  std.is_string value && value == "e41340bec587b6d049b5cc518ec7db925dde84be"
) in
let ValenceEvidenceSchema = std.contract.from_predicate (fun value =>
  std.is_string value && value == "valence.evidence-ir.v1"
) in
let ValenceSidecarSchema = std.contract.from_predicate (fun value =>
  std.is_string value && value == "valence.stack-provenance-sidecar.v1"
) in
let ValenceReceiptSchema = std.contract.from_predicate (fun value =>
  std.is_string value && value == "valence.receipt-transcript.v1"
) in
let ValenceRoleRegistry = std.contract.from_predicate (fun value =>
  std.is_string value && value == "valence-core::stack_role_registry@220307a371e7bc4e3309447b09447d0fe6c4db5f"
) in
let ValenceSpanRegistry = std.contract.from_predicate (fun value =>
  std.is_string value && value == "valence-core::evidence_ir::EvidenceSpan@220307a371e7bc4e3309447b09447d0fe6c4db5f"
) in
let ValenceLicense = std.contract.from_predicate (fun value =>
  std.is_string value && value == "MPL-2.0"
) in
let ValenceSubjectKind = std.contract.from_predicate (fun value =>
  std.is_string value && std.array.elem value ["candidate", "pipeline", "run", "attempt", "product", "event", "receipt"]
) in
let ValenceIdentityRole = std.contract.from_predicate (fun value =>
  std.is_string value && std.array.elem value ["producer", "checker", "builder", "signer", "provider", "worker", "linker", "consumer"]
) in
let ValenceSpanKind = std.contract.from_predicate (fun value =>
  std.is_string value && std.array.elem value ["source", "receipt", "event"]
) in
let HttpsScheme = std.contract.from_predicate (fun value =>
  std.is_string value && value == "https"
) in
let PostMethod = std.contract.from_predicate (fun value =>
  std.is_string value && value == "POST"
) in
let JsonContentType = std.contract.from_predicate (fun value =>
  std.is_string value && value == "application/json"
) in
let WebhookAuthKind = std.contract.from_predicate (fun value =>
  std.is_string value && std.array.elem value ["none", "bearer", "blake3_signature"]
) in
let CelldRelease = std.contract.from_predicate (fun value =>
  std.is_string value && value == "v0.2.1"
) in
let CelldRevision = std.contract.from_predicate (fun value =>
  std.is_string value && value == "ae8fac053d79f971bfcb996054bb43eb2f9b05da"
) in
let TrustDomain = std.contract.from_predicate (fun value =>
  std.is_string value && value == "single"
) in
let HmacSha256Auth = std.contract.from_predicate (fun value =>
  std.is_string value && value == "hmac-sha256"
) in
let RedirectsDisabled = std.contract.from_predicate (fun value =>
  std.is_bool value && value == true
) in
let TrueOnly = std.contract.from_predicate (fun value =>
  std.is_bool value && value == true
) in
let EmptyString = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value == 0
) in
let RusqliteVersion = std.contract.from_predicate (fun value =>
  std.is_string value && value == "0.40.2"
) in
let LibsqliteVersion = std.contract.from_predicate (fun value =>
  std.is_string value && value == "0.38.2"
) in
let SqliteRuntimeVersion = std.contract.from_predicate (fun value =>
  std.is_string value && value == "3.53.2"
) in
let MitLicense = std.contract.from_predicate (fun value =>
  std.is_string value && value == "MIT"
) in
let SqliteTransaction = std.contract.from_predicate (fun value =>
  std.is_string value && value == "immediate"
) in
let SqliteJournalMode = std.contract.from_predicate (fun value =>
  std.is_string value && value == "WAL"
) in
let SqliteSynchronous = std.contract.from_predicate (fun value =>
  std.is_string value && value == "FULL"
) in
let TrueOnly = std.contract.from_predicate (fun value =>
  std.is_bool value && value == true
) in
{
  SQLiteProfile = {
    database_path | NonEmptyString,
    open_mode | NonEmptyString,
    rusqlite_version | RusqliteVersion,
    libsqlite3_sys_version | LibsqliteVersion,
    sqlite_runtime_version | SqliteRuntimeVersion,
    license | MitLicense,
    schema_version | VersionOne,
    transaction_kind | SqliteTransaction,
    journal_mode | SqliteJournalMode,
    synchronous | SqliteSynchronous,
    backup_required | TrueOnly,
    destructive_repair | LocalCapacityOnly,
    bounds | {
      busy_timeout_millis | Positive,
      max_events_per_append | Positive,
      max_replay_records | Positive,
      max_outbox_records | Positive,
      max_event_bytes | Positive,
      max_snapshot_bytes | Positive,
      max_backup_bytes | Positive,
      max_migration_steps | Positive,
      max_retention_deletes | Positive,
    },
    migration_edges | Array {
      from | NonNegative,
      to | VersionOne,
    },
    non_claims | Array NonEmptyString,
  },
  WebhookProfile = {
    id | NonEmptyString,
    endpoint | {
      scheme | HttpsScheme,
      host | NonEmptyString,
      port | Positive,
      path | NonEmptyString,
      method | PostMethod,
      content_type | JsonContentType,
      dynamic_destination | LocalCapacityOnly,
      allow_redirects | LocalCapacityOnly,
      allow_private_ranges | LocalCapacityOnly,
    },
    auth | {
      kind | WebhookAuthKind,
      secret_reference | NonEmptyString,
      inline_secret | EmptyString,
      signature_algorithm | NonEmptyString,
    },
    idempotent_receiver | Bool,
    reconciliation_supported | Bool,
    bounds | {
      max_request_bytes | Positive,
      max_response_bytes | Positive,
      max_resolver_candidates | Positive,
      deadline_ticks | Positive,
      retry_limit | Positive,
    },
    non_claims | Array NonEmptyString,
  },
  ValenceProfile = {
    source_revision | ValenceRevision,
    artifact_auth_revision | ArtifactAuthRevision,
    evidence_ir_schema | ValenceEvidenceSchema,
    sidecar_schema | ValenceSidecarSchema,
    receipt_schema | ValenceReceiptSchema,
    role_registry | ValenceRoleRegistry,
    span_registry | ValenceSpanRegistry,
    license | ValenceLicense,
    subject_kinds | Array ValenceSubjectKind,
    identity_roles | Array ValenceIdentityRole,
    span_kinds | Array ValenceSpanKind,
    required_non_claims | Array NonEmptyString,
    bounds | {
      max_artifact_bytes | Positive,
      max_facts | Positive,
      max_roles | Positive,
      max_text_bytes | Positive,
      max_response_bytes | Positive,
    },
  },
  OpenTelemetryProfile = {
    otlp_proto_version | OtlpVersion,
    otlp_proto_revision | OtlpRevision,
    semantic_conventions_version | SemanticConventionsVersion,
    semantic_conventions_revision | SemanticConventionsRevision,
    cicd_stability | ReleaseCandidate,
    license | ApacheLicense,
    signals | Array NonEmptyString,
    allowed_attributes | Array NonEmptyString,
    allowed_metric_names | Array NonEmptyString,
    resource_attributes | { _ : NonEmptyString },
    drop_policy | TelemetryDropPolicy,
    bounds | {
      max_attributes | Positive,
      max_value_bytes | Positive,
      max_record_bytes | Positive,
      max_batch_records | Positive,
      max_queue_records | Positive,
    },
    transport | {
      protocol | OtlpHttpJson,
      endpoint_profile | NonEmptyString,
      credential_reference | String,
      tls_required | TlsRequired,
      deadline_ticks | Positive,
      max_response_bytes | Positive,
      retry_limit | Positive,
    },
    non_claims | Array NonEmptyString,
  },
  CDEventsProfile = {
    cdevents_spec_version | CDEventsVersion,
    cdevents_source_revision | CDEventsRevision,
    cloudevents_spec_version | CloudEventsSpec,
    cloudevents_context_version | CloudEventsContext,
    license | ApacheLicense,
    supported_event_types | Array NonEmptyString,
    allow_lossy_mapping | LocalCapacityOnly,
    bounds | {
      max_envelope_bytes | Positive,
      max_text_bytes | Positive,
      max_fields | Positive,
    },
    non_claims | Array NonEmptyString,
  },
  CelldProfile = {
    celld_release | CelldRelease,
    celld_revision | CelldRevision,
    license | ApacheLicense,
    worker_protocol_version | VersionOne,
    schema_version | VersionOne,
    origin | HttpsScheme,
    path | NonEmptyString,
    tls_policy | HttpsScheme,
    auth_profile | HmacSha256Auth,
    redirects_disabled | RedirectsDisabled,
    trust_domain | TrustDomain,
    output_gate_enabled | TrueOnly,
    storage_probe_required | TrueOnly,
    bounds | {
      max_request_bytes | Positive,
      max_response_bytes | Positive,
      max_replay_page_bytes | Positive,
      max_events_per_append | Positive,
      max_outbox_records | Positive,
      max_event_bytes | Positive,
      max_partition_count | Positive,
      max_lease_ticks | Positive,
      max_migration_steps | Positive,
      timeout_millis | Positive,
    },
    non_claims | Array NonEmptyString,
  },
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
