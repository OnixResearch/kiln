# A declared environment variable whose name is not an identifier is refused.
let contracts = import "../build-input-contracts.ncl" in
{
  schema = "kiln.build-input.request.v1",
  profile = "cargo_compatibility.v1",
  targets = ["alpha"],
  command = ["cargo", "test", "--offline", "--locked", "--package", "alpha"],
  toolchain = { name = "rust", version = "1.88.0" },
  environment = { "1BAD" = "x" },
}
  | contracts.Request
