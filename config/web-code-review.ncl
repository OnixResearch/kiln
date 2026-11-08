# Optional interactive review policy. Export JSON and pass --code-review explicitly.
# Rust admission binds repository identity, all targets and CI project to the service.
let c = import "github-contracts.ncl" in
let Text = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= maximum
    && !(std.string.is_match "[\\x00-\\x1f\\x7f]" value)
  )
in
let Principal =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 128
    && std.string.is_match "^[A-Za-z0-9_.:@/-]+$" value
  )
in
let PrincipalBound =
  std.contract.from_predicate (fun values =>
    std.array.length values > 0 && std.array.length values <= 128
  )
in
let Target =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 256
    && std.string.is_match "^refs/heads/[A-Za-z0-9_./-]+$" value
    && !(std.string.is_match "(\\.\\.|//|/\\.|[.]lock$|[./]$)" value)
  )
in
let Capability =
  std.contract.from_predicate (fun value =>
    std.array.elem value ["inspect", "compare", "register", "submit", "rebase", "revert", "cherry_pick", "apply_suggestions", "build_integration", "speculative_integration"]
  )
in
let OwnershipEvidence = {
  kind | std.contract.from_predicate (fun value => value == "project_ci"),
  project | Text 128,
}
in
let ChangeClass = {
  id | std.contract.from_predicate (fun value => value != "logic") | Text 128,
  paths | Array (Text 1024),
}
in
let ApprovalMode =
  std.contract.from_predicate (fun value =>
    value == "owner_approval" || value == "evidence_only"
  )
in
let AgentVotes =
  std.contract.from_predicate (fun value =>
    value == "advisory" || value == "binding"
  )
in
let AgentText =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.trim value == value
  )
in
let AgentIdentity = {
  harness | Text 128 | AgentText,
  model | Text 128 | AgentText | optional,
}
in
let Ownership = {
  teams | { _ | Array Principal },
  classes | Array ChangeClass | optional,
  dependents | { _ | Array (Text 128) } | optional,
  rules
    | Array {
      id | Text 128,
      paths | Array (Text 1024),
      owners | Array Principal,
      teams | Array (Text 128),
      target_refs | Array Target,
      project | Text 128 | optional,
      interface | Array (Text 1024) | optional,
      required_evidence | Array OwnershipEvidence | optional,
      approval | { _ | ApprovalMode } | optional,
    },
}
in
# Optional change-ID intake (jj change IDs and the git `change-id` header).
# Absent exports nothing, so existing profiles stay byte-identical. The
# workspace and executable come together; without them only header intake
# is possible, so it must be enabled.
let Jujutsu = {
  workspace_path | c.AbsolutePath | optional,
  executable | c.AbsolutePath | optional,
  change_id_header | Bool | default = false,
  max_stack | c.BoundedInteger 1 32,
}
in
let JujutsuShape =
  std.contract.from_predicate (fun value =>
    (std.record.has_field "workspace_path" value) == (std.record.has_field "executable" value)
    && (std.record.has_field "workspace_path" value || value.change_id_header)
  )
in
let IntakeBound =
  std.contract.from_predicate (fun value =>
    std.is_array value && std.array.length value <= 128
  )
in
let Intake = {
  default_target_ref | Target,
  mappings
    | Array { source_ref | Target, target_ref | Target, principal | Principal }
    | IntakeBound,
  jujutsu | Jujutsu | JujutsuShape | optional,
}
in
# Absent `mode`/`window` export nothing, so existing serial profiles stay
# byte-identical. Speculative windows stack at most 16 candidates per target.
let Window = {
  floor | c.BoundedInteger 1 16,
  ceiling | c.BoundedInteger 1 16,
  increase | c.BoundedInteger 1 16,
  decrease | c.BoundedInteger 1 16,
}
in
let Queue = {
  schema | std.contract.from_predicate (fun value => value == "kiln.review-queue.v1"),
  max_entries | c.BoundedInteger 1 1000,
  max_candidate_builds | c.BoundedInteger 1 128,
  mode | std.contract.from_predicate (fun value => std.array.elem value ["serial", "speculative"]) | optional,
  window | Window | optional,
}
in
let QueueMode =
  std.contract.from_predicate (fun queue =>
    queue == null
    || (
      if std.record.has_field "mode" queue && queue.mode == "speculative" then
        std.record.has_field "window" queue
        && queue.window.floor <= queue.window.ceiling
        && queue.window.ceiling <= queue.max_entries
        && queue.window.increase <= queue.window.ceiling
        && queue.window.decrease <= queue.window.ceiling
      else
        !(std.record.has_field "window" queue)
    )
  )
in
let SpeculativeCapability =
  std.contract.from_predicate (fun profile =>
    profile.queue == null
    || !(std.record.has_field "mode" profile.queue)
    || profile.queue.mode == "serial"
    || std.array.elem "speculative_integration" profile.provider.capabilities
  )
in
let Unique = fun values =>
  std.array.length values == std.array.length (
    std.array.fold_left
      (fun seen value => if std.array.elem value seen then seen else seen @ [value])
      []
      values
  )
in
let SignatureName =
  std.contract.from_predicate (fun value => !(std.string.is_match "[<>]" value))
in
let SignatureEmail =
  std.contract.from_predicate (fun value =>
    std.string.is_match "^[^<>\\x00-\\x1f\\x7f]+@[^<>\\x00-\\x1f\\x7f]+$" value
  )
in
let ReviewRelations =
  std.contract.from_predicate (fun profile =>
    let readers = profile.read_principals in
    let writers = profile.write_principals in
    Unique readers
    && Unique writers
    && std.array.all (fun principal => std.array.elem principal readers) writers
    && (
      !(std.record.has_field "agent_principals" profile)
      || (
        Unique profile.agent_principals
        && std.array.all (fun principal => std.array.elem principal writers) profile.agent_principals
        && (
          !(std.record.has_field "agent_metadata" profile)
          || std.array.all
            (fun principal => std.array.elem principal profile.agent_principals)
            (std.record.fields profile.agent_metadata)
        )
      )
    )
    && std.array.all
      (fun principal => std.array.elem principal readers && std.array.elem principal writers)
      profile.policy.eligible_reviewers
    && std.array.all
      (fun principal => std.array.elem principal readers && std.array.elem principal writers)
      profile.policy.submitters
    && profile.policy.minimum_approvals <= std.array.length profile.policy.eligible_reviewers
    && std.array.elem "inspect" profile.provider.capabilities
    && std.array.elem "compare" profile.provider.capabilities
    && (
      profile.delivery == null
      || (
        std.array.all
          (fun recipient => std.array.elem recipient.principal readers)
          profile.delivery.recipients
        && std.array.all
          (fun recipient =>
            std.array.elem
              recipient.destination
              (std.array.map (fun destination => destination.id) profile.delivery.destinations)
          )
          profile.delivery.recipients
      )
    )
    && (
      profile.intake == null
      || (
        std.array.elem profile.intake.default_target_ref profile.provider.target_refs
        && Unique (std.array.map (fun mapping => mapping.source_ref) profile.intake.mappings)
        && std.array.all
          (fun mapping =>
            !std.array.elem mapping.source_ref profile.provider.target_refs
            && std.array.elem mapping.target_ref profile.provider.target_refs
            && std.array.elem mapping.principal writers
          )
          profile.intake.mappings
      )
    )
  )
in
let AtMost = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_array value && std.array.length value <= maximum
  )
in
let UniqueDestinationIds =
  std.contract.from_predicate (fun destinations =>
    let check = fun state destination =>
      if std.array.elem destination.id state.seen then
        { seen = state.seen, duplicate = true }
      else
        { seen = state.seen @ [destination.id], duplicate = false }
    in
    !(std.array.fold_left check { seen = [], duplicate = false } destinations).duplicate
  )
in
let UniqueRecipientPrincipals =
  std.contract.from_predicate (fun recipients =>
    let check = fun state recipient =>
      if std.array.elem recipient.principal state.seen then
        { seen = state.seen, duplicate = true }
      else
        { seen = state.seen @ [recipient.principal], duplicate = false }
    in
    !(std.array.fold_left check { seen = [], duplicate = false } recipients).duplicate
  )
in
let DestinationAddresses =
  std.contract.from_predicate (fun destinations =>
    std.array.all
      (fun destination =>
        std.array.length destination.addresses >= 1
        && std.array.length destination.addresses <= destination.max_resolver_candidates
      )
      destinations
  )
in
let Delivery = {
  schema | std.contract.from_predicate (fun value => value == "kiln.review-delivery-profile.v1"),
  destinations
    | Array {
      id | Text 128,
      project_id | Text 128,
      enabled | std.contract.from_predicate (fun value => value == true),
      on_success | std.contract.from_predicate (fun value => value == false),
      on_failure | std.contract.from_predicate (fun value => value == false),
      host | Text 253,
      port | c.BoundedInteger 1 65535,
      path | Text 2048,
      addresses | Array String,
      bearer_secret_file | c.Nullable c.AbsolutePath,
      ca_certificate_file | c.Nullable c.AbsolutePath,
      allow_loopback | Bool,
      allow_private | Bool,
      timeout_ms | c.BoundedInteger 1 30000,
      max_request_bytes | c.BoundedInteger 1 1048576,
      max_response_bytes | c.BoundedInteger 1 1048576,
      max_resolver_candidates | c.BoundedInteger 1 16,
    }
    | AtMost 128
    | UniqueDestinationIds
    | DestinationAddresses,
  recipients
    | Array {
      principal | Principal,
      destination | Text 128,
      address | Text 256,
      channel | std.contract.from_predicate (fun value => std.array.elem value ["webhook", "email_adapter"]),
    }
    | AtMost 128
    | UniqueRecipientPrincipals,
  max_records | c.BoundedInteger 1 100000,
  max_attempts | c.BoundedInteger 1 4,
  retry_delay_ms | c.BoundedInteger 1000 3600000,
  max_tick_effects | c.BoundedInteger 1 16,
}
in
# Display-only noise globs (`review.noise`). Same glob grammar as ownership
# patterns (kiln_core::paths::valid_pattern), with explicit count and length
# limits; never consulted for ownership, approvals, threads, CI or queueing.
let NoisePattern =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 256
    && !(std.string.is_match "[\\x00-\\x1f\\x7f\\\\\\[\\]{}]" value)
    && !(std.string.is_match "(^|/)(\\.|\\.\\.)?(/|$)" value)
    && !(std.string.is_match "[^/]\\*\\*|\\*\\*[^/]" value)
  )
in
let NoiseId =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 128
    && !(std.string.is_match "[\\s*\\x00-\\x1f\\x7f]" value)
  )
in
let NoiseReason =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.trim value != ""
    && std.string.length value <= 256
    && !(std.string.is_match "[\\x00-\\x1f\\x7f]" value)
  )
in
let NoiseBound =
  std.contract.from_predicate (fun rules =>
    let ids = std.array.map (fun rule => rule.id) rules in
    std.array.length rules <= 64
    && std.array.length ids == std.array.length (std.array.fold_left (fun seen id => if std.array.elem id seen then seen else seen @ [id]) [] ids)
  )
in
let Review = {
  noise | Array { id | NoiseId, pattern | NoisePattern, reason | NoiseReason } | NoiseBound,
}
in
fun input =>
  let admitted =
    input
      | {
        database | c.AbsolutePath,
        project | Text 128,
        provider
          | {
            repository_id | c.Blake3,
            repository_path | c.AbsolutePath,
            target_refs
              | Array Target
              | std.contract.from_predicate (fun values => std.array.length values > 0 && std.array.length values <= 32),
            capabilities
              | Array Capability
              | std.contract.from_predicate (fun values => std.array.length values > 0 && std.array.length values <= 10),
          },
        policy
          | {
            id | Text 128,
            eligible_reviewers | Array Principal | PrincipalBound,
            submitters | Array Principal | PrincipalBound,
            minimum_approvals | c.BoundedInteger 1 128,
            require_ci | Bool,
            ownership | Ownership,
            # Required checks a trigger `paths` filter may skip: waived only
            # when every exact changed file matches `paths` (Rust admits globs).
            optional_ci
              | Array {
                project | String,
                paths | Array String,
              }
              | optional,
            agent_votes | AgentVotes | optional,
          },
        read_principals | Array Principal | PrincipalBound,
        agent_principals | Array Principal | PrincipalBound | optional,
        agent_metadata | { _ | AgentIdentity } | optional,
        write_principals | Array Principal | PrincipalBound,
        signature_name | Text 128 | SignatureName,
        signature_email | Text 254 | SignatureEmail,
        intake | c.Nullable Intake,
        delivery | c.Nullable Delivery,
        queue | c.Nullable Queue | QueueMode,
        review | Review | optional,
      }
      | SpeculativeCapability
      | ReviewRelations
  in
  {
    schema = "kiln.code-review-profile.v1",
    database = admitted.database,
    project = admitted.project,
    provider = admitted.provider,
    policy = admitted.policy,
    read_principals = admitted.read_principals,
    write_principals = admitted.write_principals,
    signature_name = admitted.signature_name,
    signature_email = admitted.signature_email,
    intake = admitted.intake,
    delivery = admitted.delivery,
    queue = admitted.queue,
  }
  # Absent optional fields export nothing, so existing profiles stay byte-identical.
  & (if std.record.has_field "review" admitted then { review = admitted.review } else {})
  & (if std.record.has_field "agent_principals" admitted then { agent_principals = admitted.agent_principals } else {})
  & (if std.record.has_field "agent_metadata" admitted then { agent_metadata = admitted.agent_metadata } else {})
