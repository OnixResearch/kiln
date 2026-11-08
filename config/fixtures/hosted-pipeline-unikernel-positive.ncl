(import "../hosted-pipeline.ncl") {
  name = "Unikernel",
  triggers = ["manual"],
  jobs.check = {
    kind = "unikernel-test",
    runner = "unikernel-reviewed",
    unikernel = {
      binary = { path = "bin/test", blake3 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" },
      argv = ["pass"],
      fixtures = [{ path = "data/input", blake3 = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" }],
    },
  },
}
