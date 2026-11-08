(import "../hosted-pipeline.ncl") {
  name = "Rejected",
  triggers = ["manual"],
  jobs.check = { kind = "unikernel-test", runner = "linux-reviewed" },
}
