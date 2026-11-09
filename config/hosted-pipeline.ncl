# Repository-owned hosted pipeline authoring. Export JSON with Nickel, then pass
# that JSON to pipeline.compile / pipeline.review through the hosted Iroh CLI.
# Rust admission enforces named expansion, byte, graph and deadline bounds.
let TextValue =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 4096
    && !(std.string.is_match "\\x00" value)
  )
in
let MapKeys = fun value =>
  std.array.all
    (fun key =>
      std.string.length key > 0
      && std.string.length key <= 64
      && std.string.is_match "^[A-Za-z0-9_-]+$" key
    )
    (std.record.fields value)
in
let MapKeysContract = std.contract.from_predicate MapKeys in
let TextMap =
  std.contract.custom (fun label value =>
    let result = std.contract.check { _ | TextValue } label value in
    result
    |> match {
      'Ok fields =>
        if MapKeys fields then
          'Ok fields
        else
          'Error { message = "invalid text map key" },
      'Error error => 'Error error,
    }
  )
in
let BoundedText = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= maximum
  )
in
let PipelineName =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 4096
    && std.string.trim value != ""
    && !(std.string.is_match "\\x00" value)
  )
in
let BranchReference =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 512
    && std.string.is_match "^refs/heads/[^ ~^:?*\\\\]+$" value
    && !(std.string.is_match "[\\x00-\\x20\\x7f]" value)
    && !(std.string.is_match "\\[" value)
    && !(std.string.is_match "\\.\\.|//|@\\{|/$" value)
  )
in
let Command =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 32768
    && !(std.string.is_match "\\x00" value)
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
let ExactFields = (import "field-contracts.ncl").ExactFields in
let JobFields =
  std.contract.from_predicate (fun jobs =>
    std.array.all
      (
        ExactFields ["kind", "unikernel", "template", "parameters", "runner", "steps", "env", "needs", "matrix", "timeout_ms"]
      )
      (std.record.values jobs)
  )
in
let TemplateFields =
  std.contract.from_predicate (fun templates =>
    std.array.all
      (ExactFields ["parameters", "runner", "steps", "env", "timeout_ms"])
      (std.record.values templates)
  )
in
let ScheduleFields =
  std.contract.from_predicate (fun schedules =>
    std.array.all
      (ExactFields ["cron", "timezone", "min_interval_ms", "reference"])
      (std.record.values schedules)
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
      (fun name => !(std.array.elem name (std.record.get name pipeline.jobs).needs))
      job_names
    && std.array.all
      (fun name =>
        let job = std.record.get name pipeline.jobs in
        let declared =
          std.record.fields pipeline.parameters
          @ (
            if job.template == null then
              []
            else
              std.record.fields (std.record.get job.template pipeline.templates).parameters
          )
        in
        std.array.all (fun key => std.array.elem key declared) (std.record.fields job.parameters)
      )
      job_names
    && (
      let dedup = fun values =>
        std.array.fold_left
          (fun seen value => if std.array.elem value seen then seen else seen @ [value])
          []
          values
      in
      let cycles = fun name =>
        let step = fun state =>
          let next =
            dedup (
              std.array.fold_left
                (fun acc job => acc @ (std.record.get job pipeline.jobs).needs)
                []
                state.frontier
            )
          in
          { frontier = next, cycle = state.cycle || std.array.elem name next }
        in
        (
          std.array.fold_left
            (fun state _ => step state)
            { frontier = (std.record.get name pipeline.jobs).needs, cycle = false }
            (std.array.generate (fun _ => 0) 64)
        ).cycle
      in
      !(std.array.any cycles job_names)
    )
    && std.array.all
      (fun step =>
        step.kind != "use" || std.array.elem step.step step_names
      )
      steps
    && std.array.all
      (fun step =>
        step.kind != "use"
        || std.array.all
          (fun key =>
            std.array.elem
              key
              (std.record.fields (std.record.get step.step pipeline.steps).parameters)
          )
          (std.record.fields step.parameters)
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
            has_schedule
            && std.array.elem trigger.schedule schedule_names
            && !(std.record.has_field "branches" trigger)
            && !(std.record.has_field "tags" trigger)
            && !(std.record.has_field "types" trigger)
            && !(std.record.has_field "paths" trigger)
          else if trigger.event == "push" then
            !(std.record.has_field "types" trigger) && !has_schedule
          else if trigger.event == "pull_request" then
            !(std.record.has_field "tags" trigger) && !has_schedule
          else
            !(std.record.has_field "branches" trigger)
            && !(std.record.has_field "tags" trigger)
            && !(std.record.has_field "types" trigger)
            && !(std.record.has_field "paths" trigger)
            && !has_schedule
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
    && (
      let expansions =
        std.array.fold_left
          (fun total job =>
            total
            + std.array.fold_left
              (fun count values => count * std.array.length values)
              1
              (std.record.values job.matrix)
          )
          0
          jobs
      in
      expansions <= 256
    )
  )
in
let Matrix =
  std.contract.from_predicate (fun value =>
    let axes = std.record.values value in
    std.array.all
      (fun values =>
        std.array.length values >= 1
        && std.array.length values <= 32
        && std.array.all (fun item => std.string.length item <= 4096) values
      )
      axes
    && std.array.fold_left (fun total values => total * std.array.length values) 1 axes <= 256
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
let WorkingDirectory =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 4096
    && !(std.string.is_match "^/" value)
    && !(std.string.is_match "\\\\" value)
    && !(std.string.is_match "(^|/)\\.\\.(/|$)" value)
  )
in
let Shell = std.contract.from_predicate (fun value => value == "bash" || value == "sh") in
let Run = {
  name | TextValue | default = "",
  run | Command,
  shell | Shell | default = "bash",
  env | TextMap | default = {},
  working_directory | WorkingDirectory | default = ".",
}
in
let Step =
  std.contract.custom (fun label value =>
    let allowed =
      if value.kind == "run" then
        ["kind", "name", "run", "shell", "env", "working_directory"]
      else if value.kind == "use" then
        ["kind", "step", "parameters"]
      else
        []
    in
    if !(
      std.array.all
        (fun field => std.array.elem field allowed)
        (std.record.fields value)
    ) then
      'Error { message = "unknown step field" }
    else if value.kind == "run" then
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
let StepTemplateFields =
  std.contract.from_predicate (fun steps =>
    std.array.all (ExactFields ["parameters", "step"]) (std.record.values steps)
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
let TestPath =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 4096
    && std.string.is_match "^[\\x20-\\x7e]+$" value
    && !(std.string.is_match "(^|/)(\\.|\\.\\.)?(/|$)" value)
    && value != "TRAILER!!!"
    && !(std.string.is_match "^TRAILER!!!/" value)
  )
in
let Arg = fun value =>
  std.is_string value
  && std.string.length value > 0
  && value != "KILN_ARGV_END"
  && !(std.string.is_match "^--|--" value)
  && !(std.string.is_match "^-" value)
  && std.string.is_match "^[A-Za-z0-9_./:+,@%-]+$" value
in
let ArgList =
  std.contract.from_predicate (fun values =>
    std.array.length values <= 64
    && std.array.all Arg values
    && std.array.fold_left (fun total value => total + std.string.length value + 1) 0 values <= 1024
  )
in
let TestFile = { path | TestPath, blake3 | std.contract.from_predicate (fun v => std.is_string v && std.string.is_match "^[0-9a-fA-F]{64}$" v) } in
let PathUnder = (import "field-contracts.ncl").PathUnder in
let TestFiles =
  std.contract.from_predicate (fun files =>
    std.array.length files <= 128
    && std.array.length (std.array.map (fun file => file.path) files) == std.array.length (
      std.array.fold_left
        (fun seen value => if std.array.elem value seen then seen else seen @ [value])
        []
        (std.array.map (fun file => file.path) files)
    )
    && std.array.all
      (fun file =>
        std.array.all
          (fun other => file.path == other.path || (!PathUnder file.path other.path && !PathUnder other.path file.path))
          files
      )
      files
  )
in
let TestSpecShape = { binary | TestFile, argv | Array String | ArgList | default = [], fixtures | Array TestFile | TestFiles | default = [] } in
let TestSpec =
  std.contract.custom (fun label value =>
    let result = std.contract.check TestSpecShape label value in
    result
    |> match {
      'Ok spec =>
        if std.array.all
          (fun file => file.path != "test" && !PathUnder "test" file.path)
          spec.fixtures then
          'Ok spec
        else
          'Error { message = "unikernel fixture collides with the reserved test binary path" },
      'Error error => 'Error error,
    }
  )
in
let Job = {
  kind | std.contract.from_predicate (fun v => v == "unikernel-test") | optional,
  unikernel | TestSpec | optional,
  template | Nullable (BoundedText 128) | default = null,
  parameters | TextMap | RecordAtMost 64 | default = {},
  runner | Nullable (BoundedText 128) | default = null,
  steps | Array Step | AtMost 64 | default = [],
  env | TextMap | RecordAtMost 128 | default = {},
  needs | Array String | AtMost 64 | UniqueStrings | default = [],
  matrix | { _ : Array String } | RecordAtMost 8 | Matrix | MapKeysContract | default = {},
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
  branches | Patterns | Array Pattern | UniqueStrings | optional,
  tags | Patterns | Array Pattern | UniqueStrings | optional,
  types | Array PullRequestType | UniqueStrings | optional,
  paths | Patterns | Array Pattern | UniqueStrings | optional,
  schedule | BoundedText 128 | optional,
}
in
let Trigger =
  std.contract.custom (fun label value =>
    if std.is_string value then
      if std.array.elem value ["manual", "push", "pull_request"] then
        'Ok value
      else
        'Error { message = "bare triggers are manual, push or pull_request" }
    else if std.array.all
      (fun field =>
        std.array.elem field ["event", "branches", "tags", "types", "paths", "schedule"]
      )
      (std.record.fields value) then
      std.contract.check TriggerObject label value
    else
      'Error { message = "unknown trigger field" }
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
      name | PipelineName,
      triggers | Array Trigger | NonEmptyAtMost 8,
      schedules | { _ : ScheduleEntry } | RecordAtMost 8 | ScheduleFields | MapKeysContract | optional,
      parameters | TextMap | RecordAtMost 64 | default = {},
      env | TextMap | RecordAtMost 128 | default = {},
      templates | { _ : JobTemplate } | RecordAtMost 64 | TemplateFields | MapKeysContract | default = {},
      steps
        | {
          _ : {
            parameters | TextMap | RecordAtMost 64 | default = {},
            step | Run,
          }
        }
        | RecordAtMost 64
        | StepTemplateFields
        | MapKeysContract
        | default
        = {},
      jobs | { _ : Job } | NonEmptyRecordAtMost 64 | JobFields | MapKeysContract,
    }
    | PipelineRelations
    | std.contract.from_predicate (ExactFields ["schema", "name", "triggers", "schedules", "parameters", "env", "templates", "steps", "jobs"])
