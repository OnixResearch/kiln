# Inventory v2 profile admitting the producer collection fixtures under
# fixtures/inventory/producers/. The committed export is
# fixtures/inventory/producers/profile.json.
(import "../inventory-profile.ncl") {
  principal = "operator",
  sources = [
    {
      source = "animus-serving",
      producer = "animus",
      scope = "tenant-pilot",
      kind = "serving",
      minimum_generation = 1,
    },
    {
      source = "aspen-compute",
      producer = "aspen",
      scope = "tenant-pilot",
      kind = "compute",
      minimum_generation = 1,
    },
    {
      source = "kiln-policy",
      producer = "kiln",
      scope = "tenant-pilot",
      kind = "policy",
      minimum_generation = 1,
    },
    {
      source = "lattice-agents",
      producer = "lattice",
      scope = "tenant-pilot",
      kind = "agents",
      minimum_generation = 1,
    },
    {
      source = "lattice-workers",
      producer = "lattice",
      scope = "tenant-pilot",
      kind = "workers",
      minimum_generation = 1,
    },
    {
      source = "lattice-workflows",
      producer = "lattice",
      scope = "tenant-pilot",
      kind = "workflow_revisions",
      minimum_generation = 1,
    },
  ],
  max_age_ms = 60000,
}
