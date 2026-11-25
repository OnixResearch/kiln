# r[impl kiln.implementation.contract]
# Typed builder for one implementation intake policy. Export to the JSON policy
# consumed by `kiln-implementer drain`; the JSON file remains operator input.
let NonEmpty =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.length value > 0
  )
in
let Positive =
  std.contract.from_predicate (fun value =>
    std.is_number value && value > 0 && value == std.number.truncate value
  )
in
let Blake3 =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
  )
in
let RelativePath =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 512
    && !std.string.is_match "^/" value
    && !std.string.is_match "[\\\\\\x00]" value
    && std.array.all
      (fun segment => segment != "" && segment != "." && segment != "..")
      (std.string.split "/" value)
  )
in
let Acceptance =
  std.contract.from_predicate (fun value =>
    std.is_array value
    && std.array.length value > 0
    && std.array.length value <= 64
    && std.array.all
      (fun item =>
        std.is_record item
        && std.record.has_field "id" item
        && std.record.has_field "statement" item
        && std.is_string item.id
        && std.string.length item.id > 0
        && std.is_string item.statement
        && std.string.length item.statement > 0
        && std.string.length item.statement <= 4096
      )
      value
    && std.array.length (
      std.array.fold_left
        (fun seen item =>
          if std.array.elem item.id seen then seen else seen @ [item.id]
        )
        []
        value
    ) == std.array.length value
  )
in
let TerminalPolicy =
  std.contract.from_predicate (fun value =>
    value == "verified_pull_request" || value == "verified_or_already_satisfied" || value == "verified_integrated"
  )
in
let Identifier =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 128
    && !std.string.is_match "[\\x00-\\x1f\\x7f]" value
  )
in
# r[impl kiln.implementation.intake_selection]
# Optional explicit per-issue selection: an open issue becomes work only when
# its number is listed or it carries one of these exact label names.
let Selection =
  std.contract.from_predicate (fun value =>
    std.is_record value
    && std.array.all (fun key => key == "issues" || key == "labels") (std.record.fields value)
    && std.is_array (std.record.get_or "issues" [] value)
    && std.is_array (std.record.get_or "labels" [] value)
    && std.array.all
      (fun n => std.is_number n && n >= 1 && n == std.number.truncate n)
      (std.record.get_or "issues" [] value)
    && std.array.all
      (fun label =>
        std.is_string label
        && std.string.trim label != ""
        && std.string.length label <= 256
        && !std.string.is_match "[\\x00-\\x1f\\x7f]" label
      )
      (std.record.get_or "labels" [] value)
    && std.array.length (std.record.get_or "issues" [] value)
    + std.array.length (std.record.get_or "labels" [] value) >= 1
    && std.array.length (std.record.get_or "issues" [] value)
    + std.array.length (std.record.get_or "labels" [] value) <= 256
  )
in
let unique_values = fun values =>
  let check = fun state value =>
    if std.array.elem value state.seen then
      { seen = state.seen, duplicate = true }
    else
      { seen = state.seen @ [value], duplicate = state.duplicate }
  in
  !(std.array.fold_left check { seen = [], duplicate = false } values).duplicate
in
let is_int = fun value =>
  std.is_number value && value == std.number.truncate value
in
let IntBetween = fun minimum maximum =>
  std.contract.from_predicate (fun value =>
    is_int value && value >= minimum && value <= maximum
  )
in
let RunbookId =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^[a-z][a-z0-9-]{0,63}$" value
  )
in
let LabelName =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.trim value != ""
    && std.string.length value <= 256
    && !std.string.is_match "[\\x00-\\x1f\\x7f]" value
  )
in
let ActorLogin =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 128
    && !std.string.is_match "[\\x00-\\x20\\x7f*]" value
    && !std.string.is_match "\\[[bB][oO][tT]\\]$" value
  )
in
let UniqueActors =
  std.contract.from_predicate (fun values =>
    std.is_array values
    && std.array.length values >= 1
    && std.array.length values <= 256
    && unique_values values
  )
in
let HttpsOrigin =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 8
    && std.string.length value <= 512
    && !std.string.is_match "[\\x00-\\x20\\x7f@#?]" value
    && std.string.is_match "^https://[^/]+(/[^/]+)*$" value
  )
in
let Reference =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 1024
    && !std.string.is_match "^-" value
    && !std.string.is_match "[\\x00-\\x20\\x7f]" value
    && !std.string.is_match "\\.\\." value
  )
in
let TargetSpec =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 1280
    && !std.string.is_match "[\\x00-\\x20\\x7f]" value
    && !std.string.is_match "\\.\\." value
    && std.string.is_match "^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+@[^-][^@]*$" value
  )
in
let TargetGlob =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 512
    && !std.string.is_match "[\\x00-\\x20\\x7f]" value
    && !std.string.is_match "\\.\\." value
  )
in
let WriteScopePath =
  std.contract.from_predicate (fun value =>
    value == "."
    || (
      std.is_string value
      && std.string.length value > 0
      && std.string.length value <= 256
      && std.string.is_match "^[\\x21-\\x7e]+$" value
      && !std.string.is_match "\\\\" value
      && !std.string.is_match "^/|/$|//" value
      && !std.string.is_match "(^|/)\\.\\.?($|/)" value
    )
  )
in
let ParamLiteral =
  std.contract.from_predicate (fun value =>
    std.is_bool value
    || is_int value
    || (
      std.is_string value
      && std.string.length value <= 256
      && !std.string.is_match "[\\x00-\\x1f\\x7f]" value
    )
  )
in
let ParamMap =
  std.contract.from_predicate (fun value =>
    std.is_record value
    && std.array.length (std.record.fields value) <= 16
    && std.array.all
      (fun key =>
        std.string.is_match "^[a-z][a-z0-9_]{0,63}$" key
      )
      (std.record.fields value)
  )
in
let FanoutOptions = {
  targets | Array TargetSpec | default = [],
  target_globs | Array TargetGlob | default = [],
  max_concurrency | IntBetween 1 32 | optional,
  budget_cap | IntBetween 1 10000 | optional,
  claim_policy | [| 'refuse, 'skip, 'serialize |] | optional,
  scope_path | WriteScopePath | optional,
}
in
let LabelRule = {
  label | LabelName,
  runbook_id | RunbookId,
  target | TargetSpec | optional,
  target_ref | Reference | optional,
  params | { _ : ParamLiteral } | ParamMap | default = {},
  mode | [| 'dispatch, 'fanout |] | default = 'dispatch,
  fanout | FanoutOptions | optional,
}
in
let CommandRule = {
  command | RunbookId,
  runbook_id | RunbookId,
  target | TargetSpec | optional,
  target_ref | Reference | optional,
  params | { _ : ParamLiteral } | ParamMap | default = {},
  mode | [| 'dispatch, 'fanout |] | default = 'dispatch,
  fanout | FanoutOptions | optional,
}
in
let valid_rule_target = fun rule =>
  let has_target = std.record.has_field "target" rule in
  let has_target_ref = std.record.has_field "target_ref" rule in
  let has_fanout = std.record.has_field "fanout" rule in
  !(has_target && has_target_ref)
  && (
    if rule.mode == 'dispatch then
      !has_fanout && (has_target || has_target_ref)
    else
      has_target
      || has_target_ref
      || (
        has_fanout
        && (
          std.array.length rule.fanout.targets > 0
          || std.array.length rule.fanout.target_globs > 0
        )
      )
  )
in
let RunbookRoutingRelations =
  std.contract.from_predicate (fun routing =>
    let label_names = std.array.map (fun r => r.label) routing.labels in
    let command_names = std.array.map (fun r => r.command) routing.commands in
    routing.require_push_association == true
    && std.array.length routing.labels <= 128
    && std.array.length routing.commands <= 128
    && std.array.length routing.labels
    + std.array.length routing.commands >= 1
    && unique_values label_names
    && unique_values command_names
    && std.array.all valid_rule_target routing.labels
    && std.array.all valid_rule_target routing.commands
  )
in
let RunbookRouting =
  std.contract.Sequence [
    {
      allowed_actors | Array ActorLogin | UniqueActors,
      allow_issue_comments | Bool | default = true,
      allow_pull_request_comments | Bool | default = true,
      require_push_association | Bool | default = true,
      console_base_url | HttpsOrigin | optional,
      labels | Array LabelRule | default = [],
      commands | Array CommandRule | default = [],
    },
    RunbookRoutingRelations,
  ]
in
fun input =>
  let settings =
    input
      | {
        owner | NonEmpty,
        repository | NonEmpty,
        repository_id | Positive,
        project | Identifier,
        requested_outcome | NonEmpty,
        acceptance | Acceptance,
        dependencies | Array Blake3,
        permitted_paths | Array RelativePath,
        permitted_effects | Array Identifier,
        verification_profile | Blake3,
        attempts
          | std.contract.from_predicate (fun value =>
            std.is_number value && value >= 1 && value <= 10 && value == std.number.truncate value
          ),
        minutes_per_attempt | Positive,
        terminal_policy | TerminalPolicy,
        priority
          | std.contract.from_predicate (fun value =>
            std.is_number value && value >= 0 && value <= 255 && value == std.number.truncate value
          ),
        selection | Selection | optional,
        runbook_routing | RunbookRouting | optional,
      }
  in
  {
    repository = {
      owner = settings.owner,
      repository = settings.repository,
      repository_id = settings.repository_id,
      allow_fork_pull_requests = false,
    },
    project = settings.project,
    requested_outcome = settings.requested_outcome,
    acceptance = settings.acceptance,
    dependencies = settings.dependencies,
    permitted_paths = settings.permitted_paths,
    permitted_effects = settings.permitted_effects,
    verification_profile = settings.verification_profile,
    budget = {
      attempts = settings.attempts,
      minutes_per_attempt = settings.minutes_per_attempt,
    },
    terminal_policy = settings.terminal_policy,
    priority = settings.priority,
  }
  & (
    if std.record.has_field "selection" settings then
      {
        selection = {
          issues = std.record.get_or "issues" [] settings.selection,
          labels = std.record.get_or "labels" [] settings.selection,
        },
      }
    else
      {}
  )
  & (
    if std.record.has_field "runbook_routing" settings then
      {
        runbook_routing = settings.runbook_routing,
      }
    else
      {}
  )
