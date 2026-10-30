# Filtered source triggers and a reviewed schedule entry.
(import "../hosted-pipeline.ncl") {
  name = "Filtered checks",
  triggers = [
    "manual",
    { event = "push", branches = ["main", "release/**"], tags = ["v*"], paths = ["src/**", "Cargo.toml"] },
    { event = "pull_request", branches = ["main"], types = ["opened", "synchronize", "merged"], paths = ["src/**"] },
    { event = "schedule", schedule = "nightly" },
  ],
  schedules.nightly = { cron = "H 2 * * 1-5", timezone = "America/New_York" },
  jobs.check = {
    runner = "linux-reviewed",
    steps = [{ kind = "run", run = "printf 'checks\\n'", shell = "bash" }],
  },
}
