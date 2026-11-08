(import "../hosted-pipeline.ncl") {
  name = "Rejected",
  triggers = ["manual"],
  jobs.check = { runner = std.array.fold_left (fun acc _ => acc ++ "a") "" (std.array.generate (fun _ => 0) 129), steps = [{ kind = "run", run = "true", shell = "bash" }] },
}
