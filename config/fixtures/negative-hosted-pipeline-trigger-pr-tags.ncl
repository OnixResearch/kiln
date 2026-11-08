(import "../hosted-pipeline.ncl") {
  name = "Rejected",
  triggers = [{ event = "pull_request", tags = ["v*"] }],
  jobs.check = { runner = "linux-reviewed", steps = [{ kind = "run", run = "true", shell = "bash" }] },
}
