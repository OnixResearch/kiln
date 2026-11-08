# An interval cadence must stay inside its admitted min/max window.
let make = import "../web-automation.ncl" in
make {
  max_records = 100,
  status_limit = 10,
  max_event_bytes = 65536,
  max_tick_effects = 2,
  triggers = [],
  schedules = [
    {
      id = "interval-fixture",
      project_id = "fixture",
      reference = "refs/heads/main",
      enabled = false,
      interval_ms = 5000,
      min_interval_ms = 1000,
      max_interval_ms = 2000,
      next_due_ms = null,
    }
  ],
  notifications = [],
}
