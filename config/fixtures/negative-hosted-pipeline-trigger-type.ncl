# Unknown pull request activity types are rejected.
(import "../hosted-pipeline.ncl") {
  name = "Rejected",
  triggers = [{ event = "pull_request", types = ["edited"] }],
  jobs.check = {
    runner = "linux-reviewed",
    steps = [{ kind = "run", run = "true", shell = "bash" }],
  },
}
