# An inventory kind outside agents, workers, compute, serving and policy.
(import "../inventory-profile.ncl") {
  principal = "operator",
  sources = [
    {
      source = "aspen-compute",
      producer = "aspen",
      scope = "tenant-pilot",
      kind = "accelerators",
      minimum_generation = 1,
    },
  ],
}
