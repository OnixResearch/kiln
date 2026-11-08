(import "../hosted-pipeline.ncl") {
  name = "Rejected",
  triggers = ["manual"],
  templates.check = { runner = "linux-reviewed", steps = [{ kind = "run", run = "true", shell = "bash" }] },
  jobs.check = { template = "check", runner = "linux-reviewed" },
}
