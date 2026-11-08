(import "../hosted-pipeline.ncl") {
  name = "Rejected",
  triggers = ["manual"],
  templates.check = { runner = "linux-reviewed", steps = [{ kind = "use", step = "missing" }] },
  jobs.check = { template = "check" },
}
