(import "../hosted-pipeline.ncl") {
  name = "Rejected",
  triggers = ["manual"],
  jobs.check = {
    runner = "linux-reviewed",
    steps = [{ kind = "run", run = "true", shell = "bash" }],
    matrix = { a = ["1"], b = ["1"], c = ["1"], d = ["1"], e = ["1"], f = ["1"], g = ["1"], h = ["1"], i = ["1"] },
  },
}
