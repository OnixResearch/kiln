{
  schema = "kiln.mantle-build-policy.v1",
  platform = "x86_64-linux",
  required_products = ["out"],
  cache_observations_are_advisory = true,
  unknown_dispatch_policy = "reconcile_without_redispatch",
}
