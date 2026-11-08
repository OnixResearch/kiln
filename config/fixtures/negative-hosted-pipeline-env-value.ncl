(import "../hosted-pipeline.ncl") {
  name = "Rejected",
  triggers = ["manual"],
  jobs.check = { runner = "linux-reviewed", steps = [{ kind = "run", run = "true", shell = "bash", env = { VALUE = std.array.fold_left (fun acc _ => acc ++ "a") "" (std.array.generate (fun _ => 0) 4097) } }] },
}
