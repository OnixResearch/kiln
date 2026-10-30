# Cron entries are closed records: `tz` is not `timezone`.
let make = import "../web-automation.ncl" in
make {
  max_records = 100,
  status_limit = 10,
  max_event_bytes = 65536,
  max_tick_effects = 2,
  triggers = [],
  schedules = [
    {
      id = "nightly-fixture",
      project_id = "fixture",
      reference = "refs/heads/main",
      enabled = false,
      cron = { expression = "0 2 * * *", tz = "UTC" },
      min_interval_ms = 300000,
      max_interval_ms = 31536000000,
      next_due_ms = null,
    }
  ],
  notifications = [],
}
