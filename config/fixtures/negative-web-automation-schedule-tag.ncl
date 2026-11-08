# Schedules are branch-bound.
let make = import "../web-automation.ncl" in
make {
  max_records = 100,
  status_limit = 10,
  max_event_bytes = 65536,
  max_tick_effects = 2,
  triggers = [],
  schedules = [
    {
      id = "tag-fixture",
      project_id = "fixture",
      reference = "refs/tags/v1",
      enabled = false,
      interval_ms = 60000,
      min_interval_ms = 1000,
      max_interval_ms = 3600000,
      next_due_ms = null,
    }
  ],
  notifications = [],
}
