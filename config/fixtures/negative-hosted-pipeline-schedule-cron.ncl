(import "../hosted-pipeline.ncl") {
  name = "Rejected",
  triggers = ["manual"],
  schedules.nightly = { cron = "" },
  jobs.check = { runner = "linux-reviewed", steps = [{ kind = "run", run = "true", shell = "bash" }] },
}
