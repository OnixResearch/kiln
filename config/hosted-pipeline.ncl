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
let TestFile = { path | String, blake3 | std.contract.from_predicate (fun v => std.is_string v && std.string.is_match "^[0-9a-f]{64}$" v) } in
let TestSpec = { binary | TestFile, argv | Array String | default = [], fixtures | Array TestFile | default = [] } in
let Job = {
  kind | std.contract.from_predicate (fun v => v == "unikernel-test") | optional,
  unikernel | TestSpec | optional,
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
# Filter globs: `*` and `?` within one `/` segment and whole-segment `**`;
# no negation, classes or alternation. Rust admission is authoritative.
let Pattern =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value >= 1
    && std.string.length value <= 256
    && !(std.string.is_match "^!|[\\[\\]{}+\\\\]|(^|/)\\.{0,2}(/|$)|[^/]\\*\\*|\\*\\*[^/]" value)
  )
in
let Patterns = std.contract.from_predicate (fun value => std.is_array value && std.array.length value <= 32) in
let PullRequestType =
  std.contract.from_predicate (fun value =>
    std.array.elem value ["opened", "reopened", "synchronize", "closed", "merged"]
  )
in
let Event =
  std.contract.from_predicate (fun value =>
    std.array.elem value ["manual", "push", "pull_request", "schedule"]
  )
in
# A bare name keeps today's unfiltered trigger; an object adds filters
# (push: branches/tags/paths; pull_request: branches/types/paths) or names a
# reviewed `schedules` entry (schedule).
let TriggerObject = {
  event | Event,
  branches | Patterns | Array Pattern | optional,
  tags | Patterns | Array Pattern | optional,
  types | Array PullRequestType | optional,
  paths | Patterns | Array Pattern | optional,
  schedule | String | optional,
}
in
let Trigger =
  std.contract.custom (fun label value =>
    if std.is_string value then
      if std.array.elem value ["manual", "push", "pull_request"] then
        'Ok value
      else
        'Error { message = "bare triggers are manual, push or pull_request" }
    else
      std.contract.check TriggerObject label value
  )
in
let MinInterval =
  std.contract.from_predicate (fun value =>
    std.is_number value
    && value >= 60000
    && value <= 31536000000
    && std.number.floor value == value
  )
in
let ScheduleEntry = {
  cron | String,
  timezone | String | default = "UTC",
  min_interval_ms | MinInterval | default = 300000,
  reference | String | default = "refs/heads/main",
}
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
      schedules | { _ : ScheduleEntry } | optional,
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
