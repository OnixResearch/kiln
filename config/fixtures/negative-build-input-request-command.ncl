# A request whose command names no program is refused.
let contracts = import "../build-input-contracts.ncl" in
{
  schema = "kiln.build-input.request.v1",
  profile = "cargo_compatibility.v1",
  targets = ["alpha"],
  command = [""],
  toolchain = { name = "rust", version = "1.88.0" },
  environment = { CARGO_TERM_COLOR = "never" },
}
  | contracts.Request
