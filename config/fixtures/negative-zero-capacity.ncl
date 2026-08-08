let Contracts = import "../contracts.ncl" in
({
  version = 1,
  domain = { max_identifier_bytes = 1, max_collection_items = 1, max_transition_items = 1, max_diagnostic_bytes = 1 },
  provider = { contract_version = 1, max_request_bytes = 1, max_result_bytes = 1, max_in_flight = 1, required_outcomes = [], no_silent_fallback = true },
  journal = { max_events_per_commit = 1, lease_ticks = 1, retention_ticks = 1, replay_versions = [1] },
  fairness = { strategy = "weighted_scalar", accounting_window_ticks = 1, decay_numerator = 1, decay_denominator = 1, rounding = "exact", stale_usage_policy = "deny", tie_break = "id", resource_weights = { cpu = 1 } },
  task_operations = {
    status = { max_records = 1 },
    capacity = { max_classes = 1, default_in_flight_limit = 0, observation_max_age_ticks = 1, ordering = "id", hold_policy = "new", unsupported_fact_policy = "deny", affects_tenant_fairness = false },
    logs = { max_streams = 1, max_segments_per_stream = 1, max_segment_bytes = 1, max_total_bytes = 1, max_read_bytes = 1, retention_ticks = 1, redaction_marker = "x" },
    follow = { heartbeat_ticks = 1, inactivity_ticks = 1, deadline_ticks = 1 },
    wait = { max_selection_items = 1, deadline_ticks = 1, predicates = [] },
    controls = { timeout_ticks = 1, operations = [] },
  },
  hosting = { state = "planned", canonical_branch = "main", rid = "", delegates = ["unassigned"], threshold = 1, required_replicas = 1, license = "MIT", rollback_revision = "main", mirrors = [] },
  cob = { max_profiles = 1, max_heads = 1, max_payload_bytes = 1, max_actions = 1, retry_limit = 1, unknown_type_policy = "deny", required_profile_types = [] },
}) | Contracts.Config
