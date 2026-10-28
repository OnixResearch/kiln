# Repository-owned hosted pipeline authoring. Export JSON with Nickel, then pass
# that JSON to pipeline.compile / pipeline.review through the hosted Iroh CLI.
# Rust admission enforces named expansion, byte, graph and deadline bounds.
let TextMap = { _ : String } in
let Timeout =
  std.contract.from_predicate (fun value =>
    std.is_number value
    && value >= 1
    && value <= 86400000
    && std.number.floor value == value
  )
in
let Shell = std.contract.from_predicate (fun value => value == "bash" || value == "sh") in
let Run = {
  name | String | default = "",
  run | String,
  shell | Shell | default = "bash",
  env | TextMap | default = {},
  working_directory | String | default = ".",
}
in
let Step =
  std.contract.custom (fun label value =>
    if value.kind == "run" then
      std.contract.check ({ kind | std.contract.from_predicate (fun v => v == "run") } & Run) label value
    else if value.kind == "use" then
      std.contract.check
        {
          kind | std.contract.from_predicate (fun v => v == "use"),
          step | String,
          parameters | TextMap | default = {},
        }
        label
        value
    else
      'Error { message = "Step kind must be run or use" }
  )
in
let JobTemplate = {
  parameters | TextMap | default = {},
  runner | String,
  steps | Array Step,
  env | TextMap | default = {},
  timeout_ms | Timeout | default = 3600000,
}
in
let Nullable = fun Contract =>
  std.contract.custom (fun label value =>
    if value == null then 'Ok value else std.contract.check Contract label value
  )
in
let Job = {
  template | Nullable String | default = null,
  parameters | TextMap | default = {},
  runner | Nullable String | default = null,
  steps | Array Step | default = [],
  env | TextMap | default = {},
  needs | Array String | default = [],
  matrix | { _ : Array String } | default = {},
  timeout_ms | Nullable Timeout | default = null,
}
in
let Trigger =
  std.contract.from_predicate (fun value =>
    value == "manual" || value == "push" || value == "pull_request"
  )
in
fun input =>
  input
    | {
      schema
        | std.contract.from_predicate (fun value => value == "kiln.hosted.pipeline.v1")
        | default
        = "kiln.hosted.pipeline.v1",
      name | String,
      triggers | Array Trigger,
      parameters | TextMap | default = {},
      env | TextMap | default = {},
      templates | { _ : JobTemplate } | default = {},
      steps
        | {
          _ : {
            parameters | TextMap | default = {},
            step | Run,
          }
        }
        | default
        = {},
      jobs | { _ : Job },
    }
