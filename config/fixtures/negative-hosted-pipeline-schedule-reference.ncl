(import "../hosted-pipeline.ncl") {
  name = "Rejected",
  triggers = ["manual"],
  schedules.nightly = { cron = "H 2 * * *", reference = "refs/tags/v1" },
  jobs.check = { runner = "linux-reviewed", steps = [{ kind = "run", run = "true", shell = "bash" }] },
}
