# Per-work amounts larger than the declared host capacity.
let input = {
  provider = "onix-local-pilot-host",
  host = "kiln-pilot-guest",
  generation = 1,
  capacity = { worker_slots = 1 },
  observation_ttl_ms = 600000,
  reservation_ttl_ms = 3600000,
  campaign = "implementer_pilot",
  tenant = "onix_local",
  deadline_ms = 1830297600000,
  quota = { worker_slots = 4 },
  per_work = { worker_slots = 2 },
  limits = {
    max_items = 8,
    max_bytes = 1048576,
    max_wait_ms = 86400000,
    max_observation_age_ms = 900000,
    max_retry_after_ms = 60000,
    max_children = 1,
    max_active = 1,
  },
  repositories = [
    {
      repository = "onixresearch/implementer-sandbox",
      classes = [
        { name = "bug", project = "implementer-sandbox", verification_profile = "1111111111111111111111111111111111111111111111111111111111111111" },
        { name = "bounded-feature", project = "implementer-sandbox", verification_profile = "2222222222222222222222222222222222222222222222222222222222222222" },
      ],
    },
  ],
}
in
(import "../fair-admission-host-owner.ncl") input
