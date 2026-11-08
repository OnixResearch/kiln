# The minimum interval cannot exceed the maximum interval.
let make = import "../web-automation.ncl" in
make {
  max_records = 100,
  status_limit = 10,
  max_event_bytes = 65536,
  max_tick_effects = 2,
  triggers = [],
  schedules = [
    {
      id = "order-fixture",
      project_id = "fixture",
      reference = "refs/heads/main",
      enabled = false,
      cron = { expression = "0 2 * * *" },
      min_interval_ms = 10000,
      max_interval_ms = 1000,
      next_due_ms = null,
    }
  ],
  notifications = [],
}
