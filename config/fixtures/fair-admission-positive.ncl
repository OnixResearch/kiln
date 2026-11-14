let input = {
  accounting_basis = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  max_items = 32,
  max_bytes = 1048576,
  max_wait_ms = 60000,
  max_observation_age_ms = 10000,
  max_retry_after_ms = 30000,
  max_children = 8,
  max_active = 4,
  strategy = "dominant_share",
  weights = { requests = 1 },
  shares = { campaign_a = 1, campaign_b = 2 },
  group_limits = [{ scope = "tenant", group = "tenant_a", max_items = 16, max_bytes = 524288, max_active = 2 }],
}
in (import "../fair-admission-policy.ncl") input
