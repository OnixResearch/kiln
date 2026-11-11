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
    value == "verified_pull_request" || value == "verified_or_already_satisfied"
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
