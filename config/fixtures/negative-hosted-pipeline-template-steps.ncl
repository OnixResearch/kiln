(import "../hosted-pipeline.ncl") {
  name = "Rejected",
  triggers = ["manual"],
  templates.check = { runner = "linux-reviewed", steps = [] },
  jobs.check = { template = "check" },
}
