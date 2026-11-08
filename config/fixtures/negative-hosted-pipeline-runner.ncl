(import "../hosted-pipeline.ncl") {
  name = "Rejected",
  triggers = ["manual"],
  jobs.check = { steps = [{ kind = "run", run = "true", shell = "bash" }] },
}
