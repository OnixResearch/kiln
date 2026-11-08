(import "../hosted-pipeline.ncl") {
  name = "Rejected",
  triggers = ["manual"],
  jobs.check = { runner = "linux-reviewed", steps = [{ kind = "run", run = "", shell = "bash" }] },
}
