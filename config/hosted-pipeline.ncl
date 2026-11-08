# Repository-owned hosted pipeline authoring. Export JSON with Nickel, then pass
# that JSON to pipeline.compile / pipeline.review through the hosted Iroh CLI.
# Rust admission enforces named expansion, byte, graph and deadline bounds.
let TextMap = { _ : String } in
let BoundedText = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= maximum
  )
in
let BranchReference =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 512
    && std.string.is_match "^refs/heads/[^ ~^:?*\\\\]+$" value
  )
in
let Command =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 32768
  )
in
let AtMost = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_array value && std.array.length value <= maximum
  )
in
let NonEmptyAtMost = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_array value && std.array.length value >= 1 && std.array.length value <= maximum
  )
in
let RecordAtMost = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.array.length (std.record.fields value) <= maximum
  )
in
let NonEmptyRecordAtMost = fun maximum =>
  std.contract.from_predicate (fun value =>
    let count = std.array.length (std.record.fields value) in
    count >= 1 && count <= maximum
  )
in
let UniqueStrings =
  std.contract.from_predicate (fun values =>
    let check = fun state value =>
      if std.array.elem value state.seen then
        { seen = state.seen, duplicate = true }
      else
        { seen = state.seen @ [value], duplicate = false }
    in
    !(std.array.fold_left check { seen = [], duplicate = false } values).duplicate
  )
in
let PipelineRelations =
  std.contract.from_predicate (fun pipeline =>
    let job_names = std.record.fields pipeline.jobs in
    let template_names = std.record.fields pipeline.templates in
    let step_names = std.record.fields pipeline.steps in
    let jobs = std.record.values pipeline.jobs in
    let templates = std.record.values pipeline.templates in
    let steps =
      std.array.fold_left
        (fun state job => state @ job.steps)
        []
        jobs
      @ std.array.fold_left (fun state template => state @ template.steps) [] templates
    in
    std.array.all
      (fun job => job.template == null || std.array.elem job.template template_names)
      jobs
    && std.array.all
      (fun job => std.array.all (fun need => std.array.elem need job_names) job.needs)
      jobs
    && std.array.all
      (fun step =>
        step.kind != "use" || std.array.elem step.step step_names
      )
      steps
    && std.array.all
      (fun trigger =>
        if std.is_string trigger then
          true
        else
          let has_schedule = std.record.has_field "schedule" trigger in
          let schedule_names =
            if std.record.has_field "schedules" pipeline then
              std.record.fields pipeline.schedules
            else
              []
          in
          if trigger.event == "schedule" then
            has_schedule && std.array.elem trigger.schedule schedule_names
          else
            !has_schedule
      )
      pipeline.triggers
    && std.array.all
      (fun job =>
        let has_kind = std.record.has_field "kind" job in
        let has_unikernel = std.record.has_field "unikernel" job in
        has_kind == has_unikernel
        && (!has_kind || job.kind == "unikernel-test")
        && (
          !has_kind
          || (
            job.steps == []
            && job.template == null
            && job.env == {}
            && job.parameters == {}
            && job.matrix == {}
            && job.runner != null
          )
        )
        && (
          if job.template != null then
            job.steps == [] && job.runner == null && job.timeout_ms == null
          else
            job.runner != null && (has_kind || job.steps != [])
        )
      )
      jobs
  )
in
let Matrix =
  std.contract.from_predicate (fun value =>
    std.array.all
      (fun values => std.array.length values >= 1 && std.array.length values <= 32)
      (std.record.values value)
  )
in
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
  run | Command,
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
          step | BoundedText 128,
          parameters | TextMap | default = {},
        }
        label
        value
    else
      'Error { message = "Step kind must be run or use" }
  )
in
let JobTemplate = {
  parameters | TextMap | RecordAtMost 64 | default = {},
  runner | BoundedText 128,
  steps | Array Step | NonEmptyAtMost 64,
  env | TextMap | RecordAtMost 128 | default = {},
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
  template | Nullable (BoundedText 128) | default = null,
  parameters | TextMap | RecordAtMost 64 | default = {},
  runner | Nullable (BoundedText 128) | default = null,
  steps | Array Step | AtMost 64 | default = [],
  env | TextMap | RecordAtMost 128 | default = {},
  needs | Array String | AtMost 64 | UniqueStrings | default = [],
  matrix | { _ : Array String } | RecordAtMost 8 | Matrix | default = {},
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
  cron | BoundedText 256,
  timezone | BoundedText 128 | default = "UTC",
  min_interval_ms | MinInterval | default = 300000,
  reference | BranchReference | default = "refs/heads/main",
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
      triggers | Array Trigger | NonEmptyAtMost 8,
      schedules | { _ : ScheduleEntry } | RecordAtMost 8 | optional,
      parameters | TextMap | RecordAtMost 64 | default = {},
      env | TextMap | RecordAtMost 128 | default = {},
      templates | { _ : JobTemplate } | RecordAtMost 64 | default = {},
      steps
        | {
          _ : {
            parameters | TextMap | RecordAtMost 64 | default = {},
            step | Run,
          }
        }
        | RecordAtMost 64
        | default
        = {},
      jobs | { _ : Job } | NonEmptyRecordAtMost 64,
    }
    | PipelineRelations
