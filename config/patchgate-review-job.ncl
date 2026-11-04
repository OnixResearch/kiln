# Build one reviewed worker step for the Patchgate receipt job.
# The target is an operator-provisioned local Pijul repository; it need not be
# the Git worker checkout. The checkout supplies this script, while the target
# supplies patchgate.json, its policy and the native channels. The step invokes
# the script once; the script builds a live intent, calls the adapter's atomic
# `run` command and leaves the receipt at the declared path. `reports` makes the
# existing worker upload the exact receipt as run-bound review evidence. This
# does not grant review, merge, gate or release authority.
let Component =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 128
    && std.string.is_match "^[a-zA-Z0-9_.-]+$" value
    && value != "."
    && value != ".."
  )
in
let AbsolutePath =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^/[^\\x00-\\x1f\\x7f]+$" value
    && !(std.string.is_match "(^|/)\\.\\.?(/|$)" value)
  )
in
let RelativeFile =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 4096
    && std.string.is_match "^[^/\\\\\\x00-\\x1f\\x7f]+(/[^/\\\\\\x00-\\x1f\\x7f]+)*$" value
    && !(std.string.is_match "(^|/)\\.\\.?(/|$)" value)
  )
in
let EnvironmentValue =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && !(std.string.is_match "\\x00" value)
  )
in
fun input =>
  let settings =
    input
      | {
        node_id | Component,
        python | AbsolutePath,
        script | RelativeFile,
        target | AbsolutePath,
        adapter | AbsolutePath,
        patchgate | AbsolutePath,
        pijul | AbsolutePath,
        home | AbsolutePath,
        pipeline
          | std.contract.from_predicate (fun value =>
            std.is_string value && std.string.length value > 0 && !(std.string.is_match "\\x00" value)
          ),
        output | RelativeFile,
        path | EnvironmentValue,
      }
  in
  {
    node_id = settings.node_id,
    program = settings.python,
    argv = [
      settings.script,
      "--target",
      settings.target,
      "--patchgate",
      settings.patchgate,
      "--adapter",
      settings.adapter,
      "--pijul",
      settings.pijul,
      "--home",
      settings.home,
      "--pipeline",
      settings.pipeline,
      "--out",
      settings.output,
    ],
    environment = { PATH = settings.path },
    artifacts = [],
    reports = [{ kind = "review", path = settings.output }],
  }
