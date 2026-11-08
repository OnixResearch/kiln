# An enabled interval schedule must retain its next due time.
let make = import "../web-automation.ncl" in
make {
  max_records = 100,
  status_limit = 10,
  max_event_bytes = 65536,
  max_tick_effects = 2,
  triggers = [],
  schedules = [
    {
      id = "due-fixture",
      project_id = "fixture",
      reference = "refs/heads/main",
      enabled = true,
      interval_ms = 60000,
      min_interval_ms = 1000,
      max_interval_ms = 3600000,
      next_due_ms = null,
    }
  ],
  notifications = [],
}
