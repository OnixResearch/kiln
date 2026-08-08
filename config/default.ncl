let Contracts = import "contracts.ncl" in
let MaxIdentifierBytes = 128 in
let MaxCollectionItems = 256 in
let MaxTransitionItems = 32 in
let MaxDiagnosticBytes = 512 in
let MaxRequestBytes = 1048576 in
let MaxResultBytes = 1048576 in
let MaxInFlight = 64 in
let MaxEventsPerCommit = 32 in
let LeaseTicks = 30 in
let RetentionTicks = 604800 in
let AccountingWindowTicks = 3600 in
let DecayNumerator = 9 in
let DecayDenominator = 10 in
let MaxStatusRecords = 1024 in
let MaxCapacityClasses = 64 in
let DefaultInFlightLimit = 4 in
let ObservationMaxAgeTicks = 30 in
let MaxLogStreams = 4096 in
let MaxSegmentsPerStream = 1024 in
let MaxSegmentBytes = 65536 in
let MaxTotalLogBytes = 16777216 in
let MaxReadBytes = 262144 in
let HeartbeatTicks = 5 in
let InactivityTicks = 30 in
let DeadlineTicks = 300 in
let MaxSelectionItems = 256 in
let ControlTimeoutTicks = 30 in
let HostingThreshold = 1 in
let RequiredReplicas = 1 in
let MaxCobProfiles = 32 in
let MaxCobHeads = 32 in
let MaxCobPayloadBytes = 1048576 in
let MaxCobActions = 16 in
let CobRetryLimit = 3 in
({
  version = 1,
  domain = {
    max_identifier_bytes = MaxIdentifierBytes,
    max_collection_items = MaxCollectionItems,
    max_transition_items = MaxTransitionItems,
    max_diagnostic_bytes = MaxDiagnosticBytes,
  },
  provider = {
    contract_version = 1,
    max_request_bytes = MaxRequestBytes,
    max_result_bytes = MaxResultBytes,
    max_in_flight = MaxInFlight,
    required_outcomes = ["success", "domain_failure", "denied", "cancelled", "timed_out", "provider_error", "unknown"],
    no_silent_fallback = true,
  },
  journal = {
    max_events_per_commit = MaxEventsPerCommit,
    lease_ticks = LeaseTicks,
    retention_ticks = RetentionTicks,
    replay_versions = [1],
  },
  fairness = {
    strategy = "weighted_scalar",
    accounting_window_ticks = AccountingWindowTicks,
    decay_numerator = DecayNumerator,
    decay_denominator = DecayDenominator,
    rounding = "integer_cross_multiplication",
    stale_usage_policy = "deny",
    tie_break = "tenant_alias_ascending",
    resource_weights = { cpu = 1, memory = 1 },
  },
  task_operations = {
    status = { max_records = MaxStatusRecords },
    capacity = {
      max_classes = MaxCapacityClasses,
      default_in_flight_limit = DefaultInFlightLimit,
      observation_max_age_ticks = ObservationMaxAgeTicks,
      ordering = "fairness_then_task_id",
      hold_policy = "new_dispatch_only",
      unsupported_fact_policy = "deny",
      affects_tenant_fairness = false,
    },
    logs = {
      max_streams = MaxLogStreams,
      max_segments_per_stream = MaxSegmentsPerStream,
      max_segment_bytes = MaxSegmentBytes,
      max_total_bytes = MaxTotalLogBytes,
      max_read_bytes = MaxReadBytes,
      retention_ticks = RetentionTicks,
      redaction_marker = "[redacted]",
    },
    follow = {
      heartbeat_ticks = HeartbeatTicks,
      inactivity_ticks = InactivityTicks,
      deadline_ticks = DeadlineTicks,
    },
    wait = {
      max_selection_items = MaxSelectionItems,
      deadline_ticks = DeadlineTicks,
      predicates = ["all_terminal", "any_terminal", "all_successful"],
    },
    controls = {
      timeout_ticks = ControlTimeoutTicks,
      operations = ["hold", "release", "cancel", "pause", "resume"],
    },
  },
  hosting = {
    state = "published",
    canonical_branch = "main",
    rid = "rad:z2wsvXm5S2sJGvuV1k5JHiwi1PbKE",
    delegates = ["did:key:z6MksnXbFoE8zkCkGWhHc8zuxpnEUhrJHv2KECRV4GSv9gkx"],
    threshold = HostingThreshold,
    required_replicas = RequiredReplicas,
    license = "MIT OR Apache-2.0",
    rollback_revision = "b16f8792849cd3e2f2802c9a5063d5567a68f389",
    signed_ref_features = ["parent"],
    seeds = ["z6Mkmqogy2qEM2ummccUthFEaaHvyYmYBYh3dbe9W4ebScxo"],
    required_replica_observations = RequiredReplicas,
    acquisition_routes = [
      {
        kind = "radicle-native",
        route = "rad://z2wsvXm5S2sJGvuV1k5JHiwi1PbKE@z6Mkmqogy2qEM2ummccUthFEaaHvyYmYBYh3dbe9W4ebScxo",
        writable = false,
      },
      {
        kind = "https-git",
        route = "https://seed.radicle.garden/z2wsvXm5S2sJGvuV1k5JHiwi1PbKE.git",
        writable = false,
      },
    ],
    freeze = false,
    non_claims = [
      "publication observations do not prove indefinite availability",
      "mirror observations do not grant repository authority",
      "replica agreement does not prove source or CI correctness",
    ],
    mirrors = [{
      route = "https://seed.radicle.garden/z2wsvXm5S2sJGvuV1k5JHiwi1PbKE.git",
      authority = "read_only_projection",
    }],
  },
  cob = {
    max_profiles = MaxCobProfiles,
    max_heads = MaxCobHeads,
    max_payload_bytes = MaxCobPayloadBytes,
    max_actions = MaxCobActions,
    retry_limit = CobRetryLimit,
    unknown_type_policy = "deny",
    required_profile_types = [
      "xyz.radicle.issue",
      "xyz.radicle.patch",
      "xyz.radicle.ci-job",
      "xyz.onixresearch.cairn-change",
      "xyz.onixresearch.rfd",
      "xyz.onixresearch.adr",
      "xyz.onixresearch.verification-obligation",
      "xyz.onixresearch.release-decision",
      "xyz.onixresearch.policy-exception",
    ],
  },
}) | Contracts.Config
