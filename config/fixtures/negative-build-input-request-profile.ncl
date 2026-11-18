# A request for a build profile Kiln does not define is refused.
let contracts = import "../build-input-contracts.ncl" in
{
  schema = "kiln.build-input.request.v1",
  profile = "bazel_compatibility.v1",
  targets = ["alpha"],
  command = ["bazel", "test", "//projects/alpha/..."],
  toolchain = { name = "bazel", version = "8.0.0" },
  environment = {},
}
  | contracts.Request
