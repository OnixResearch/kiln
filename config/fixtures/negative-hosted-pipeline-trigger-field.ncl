(import "../hosted-pipeline.ncl") {
  name = "Rejected",
  triggers = [{ event = "push", extra = 1 }],
  jobs.check = { runner = "linux-reviewed", steps = [{ kind = "run", run = "true", shell = "bash" }] },
}
