let input = {
  campaign = "rlm",
  tenant = "tenant",
  max_depth = 2,
  max_children = 4,
  root = { deadline_ms = 50000, quota = { model_requests = 40, cpu_ms = 60000 } },
  accounts = [
    { name = "child_a", parent = "root", deadline_ms = 40000, allocation = { model_requests = 12, cpu_ms = 20000 } },
    { name = "child_b", parent = "root", deadline_ms = 40000, allocation = { model_requests = 12, cpu_ms = 20000 } },
    { name = "grandchild_a1", parent = "child_a", deadline_ms = 45000, allocation = { model_requests = 4, cpu_ms = 5000 } },
  ],
}
in (import "../fair-admission-lineage.ncl") input
