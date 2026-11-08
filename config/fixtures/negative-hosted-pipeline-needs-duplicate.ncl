(import "../hosted-pipeline.ncl") {
  name = "Rejected",
  triggers = ["manual"],
  jobs.check = { runner = "linux-reviewed", steps = [{ kind = "run", run = "true", shell = "bash" }] },
  jobs.finish = { runner = "linux-reviewed", needs = ["check", "check"], steps = [{ kind = "run", run = "true", shell = "bash" }] },
}
