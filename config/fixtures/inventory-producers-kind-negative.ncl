# An inventory kind outside agents, workers, compute, serving, policy and workflow_revisions.
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
