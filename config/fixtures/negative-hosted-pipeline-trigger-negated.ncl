# Actions-style negated patterns have no native equivalent.
(import "../hosted-pipeline.ncl") {
  name = "Rejected",
  triggers = [{ event = "push", paths = ["!docs/**"] }],
  jobs.check = {
    runner = "linux-reviewed",
    steps = [{ kind = "run", run = "true", shell = "bash" }],
  },
}
