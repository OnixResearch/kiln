(import "../hosted-pipeline.ncl") {
  name = "Rejected",
  triggers = [{ event = "manual", branches = ["main"] }],
  jobs.check = { runner = "linux-reviewed", steps = [{ kind = "run", run = "true", shell = "bash" }] },
}
