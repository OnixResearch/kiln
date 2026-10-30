# A bare schedule trigger names no reviewed entry.
(import "../hosted-pipeline.ncl") {
  name = "Rejected",
  triggers = ["schedule"],
  jobs.check = {
    runner = "linux-reviewed",
    steps = [{ kind = "run", run = "true", shell = "bash" }],
  },
}
