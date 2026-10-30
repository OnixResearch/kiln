# Schedule minimum intervals below the one-minute cron floor are rejected.
(import "../hosted-pipeline.ncl") {
  name = "Rejected",
  triggers = [{ event = "schedule", schedule = "fast" }],
  schedules.fast = { cron = "* * * * *", min_interval_ms = 1000 },
  jobs.check = {
    runner = "linux-reviewed",
    steps = [{ kind = "run", run = "true", shell = "bash" }],
  },
}
