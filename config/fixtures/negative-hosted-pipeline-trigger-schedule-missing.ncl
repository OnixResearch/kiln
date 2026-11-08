(import "../hosted-pipeline.ncl") {
  name = "Rejected",
  triggers = [{ event = "schedule" }],
  schedules.nightly = { cron = "H 2 * * *" },
  jobs.check = { runner = "linux-reviewed", steps = [{ kind = "run", run = "true", shell = "bash" }] },
}
