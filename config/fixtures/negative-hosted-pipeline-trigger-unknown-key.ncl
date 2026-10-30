# Trigger objects are closed: `branch` is not `branches`.
(import "../hosted-pipeline.ncl") {
  name = "Rejected",
  triggers = [{ event = "push", branch = ["main"] }],
  jobs.check = {
    runner = "linux-reviewed",
    steps = [{ kind = "run", run = "true", shell = "bash" }],
  },
}
