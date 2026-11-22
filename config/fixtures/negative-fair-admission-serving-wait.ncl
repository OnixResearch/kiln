let input = {
  accounting_basis = "8f3b0c1a6d2e4f5a7b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a",
  max_items = 32,
  max_bytes = 1048576,
  max_wait_ms = 60000,
  max_observation_age_ms = 10000,
  max_retry_after_ms = 30000,
  max_children = 4,
  max_active = 4,
  strategy = "dominant_share",
  weights = { model_requests = 1 },
  shares = { rlm = 1, peer = 1 },
  group_limits = [],
  serving = { max_snapshot_age_ms = 60001, capacity_rule = "known_required" },
}
in (import "../fair-admission-policy.ncl") input
