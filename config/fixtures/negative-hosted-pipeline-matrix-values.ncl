(import "../hosted-pipeline.ncl") {
  name = "Rejected",
  triggers = ["manual"],
  jobs.check = {
    runner = "linux-reviewed",
    steps = [{ kind = "run", run = "true", shell = "bash" }],
    matrix = { version = std.array.generate (fun index => "v" ++ std.string.from_number index) 33 },
  },
}
