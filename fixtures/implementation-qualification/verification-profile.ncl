# Seed for native `kiln-implementer prepare-verification-profile`, never admitted
# directly. Native preparation replaces both derived identities before intake.
let Digest = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
) in
let Kind = std.contract.from_predicate (fun value =>
  std.array.elem value ["bug", "feature", "already-satisfied"]
) in
fun raw =>
  let input = raw | {
    repository | String,
    kind | Kind,
    python | { path | String, blake3 | Digest },
    seed_identity | Digest,
  } in
  {
    schema = "kiln.verification-profile.v1",
    repository = input.repository,
    # Actual reviewed seed-cohort identity supplies the mandatory seed wire
    # fields. These are NOT final toolchain/environment claims; native prepare
    # derives and freezes the correct identities from program/environment maps.
    toolchain = input.seed_identity,
    environment_identity = input.seed_identity,
    environment = { LANG = "C.UTF-8", LC_ALL = "C.UTF-8", PYTHONDONTWRITEBYTECODE = "1" },
    checks = [{
      id = "acceptance",
      kind = "test",
      program = input.python,
      arguments = ["-I", "-B", "acceptance.py"],
      prerequisites = [],
      acceptance = ["accepted"],
      artifacts = ["artifacts/acceptance.json"],
      timeout_ms = 10000,
    }],
    reproducer = if input.kind == "bug" || input.kind == "already-satisfied" then "acceptance" else null,
    protected_paths = ["acceptance.py", "requirements.txt"],
    allow_already_satisfied = input.kind == "already-satisfied",
    cairn = null,
  }
