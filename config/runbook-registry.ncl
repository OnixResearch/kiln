# Optional reviewed runbook registry for `kiln-web-service serve --runbooks`.
# Export to JSON; Rust admission remains authoritative.
# Grants are exact principals narrowed to mutating runbooks with optional expiry.
let c = import "github-contracts.ncl" in

let unique = fun values =>
  let check = fun state value =>
    if std.array.elem value state.seen then
      { seen = state.seen, duplicate = true }
    else
      { seen = state.seen @ [value], duplicate = state.duplicate }
  in
  !(std.array.fold_left check { seen = [], duplicate = false } values).duplicate
in

let Unique = std.contract.from_predicate unique in

let Between = fun minimum maximum =>
  std.contract.from_predicate (fun values =>
    std.array.length values >= minimum && std.array.length values <= maximum
  )
in

let RunbookId =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^[a-z][a-z0-9-]{0,63}$" value
  )
in

let DisplayText =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 256
    && std.string.trim value != ""
    && !(std.string.is_match "[\\x00-\\x1f\\x7f]" value)
  )
in

let DescriptionText =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 512
    && !(std.string.is_match "[\\x00-\\x1f\\x7f]" value)
  )
in

let RunbookMode =
  std.contract.from_predicate (fun value =>
    value == "read_only" || value == "mutating"
  )
in

let LatticeLabel =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 128
    && std.string.is_match "^[a-z0-9_.:-]+$" value
  )
in

let NodeKind =
  std.contract.from_predicate (fun value =>
    std.array.elem value ["build", "review", "test"]
  )
in

let EffectClass =
  std.contract.from_predicate (fun value =>
    std.array.elem value ["build", "test", "custom"]
  )
in

let AdapterKind =
  std.contract.from_predicate (fun value =>
    std.array.elem value ["local", "remote"]
  )
in

let ProjectId =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 100
    && std.string.is_match "^[A-Za-z0-9_.-]+$" value
  )
in

let RepositoryName =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 256
    && !(std.string.is_match "[\\x00-\\x20\\x7f*@]" value)
  )
in

let Reference =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 1024
    && !(std.string.is_match "^-" value)
    && !(std.string.is_match "[\\x00-\\x20\\x7f]" value)
  )
in

let PrincipalId =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 256
    && !(std.string.is_match "[\\x00-\\x20\\x7f*]" value)
  )
in

let ParamName =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^[a-z][a-z0-9_]{0,63}$" value
  )
in

let ParamEnvKey =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 64
    && std.string.is_match "^LATTICE_PARAM_[A-Z0-9_]{1,50}$" value
  )
in

let is_enum_token = fun value =>
  std.is_string value
  && std.string.length value > 0
  && std.string.length value <= 64
  && std.string.is_match "^[a-z0-9][a-z0-9._-]{0,63}$" value
in

let is_int = fun value =>
  std.is_number value && std.number.floor value == value
in

let is_valid_relative_path = fun max_bytes value =>
  std.is_string value
  && std.string.length value > 0
  && std.string.length value <= max_bytes
  && std.string.is_match "^[A-Za-z0-9._/-]+$" value
  && !(std.string.is_match "^/|/$|//" value)
  && !(std.string.is_match "(^|/)(\\.\\.|-)($|/)" value)
  && !(std.string.is_match "(^|/)-" value)
in

let ParamType =
  std.contract.from_predicate (fun value =>
    std.is_record value
    && std.record.has_field "kind" value
    && (
      if value.kind == "bool" then
        std.record.fields value == ["kind"]
      else if value.kind == "integer" then
        std.record.has_field "min" value
        && std.record.has_field "max" value
        && std.array.length (std.record.fields value) == 3
        && is_int value.min
        && is_int value.max
        && value.min <= value.max
      else if value.kind == "enum" then
        std.record.has_field "values" value
        && std.array.length (std.record.fields value) == 2
        && std.is_array value.values
        && std.array.length value.values >= 1
        && std.array.length value.values <= 32
        && unique value.values
        && std.array.all is_enum_token value.values
      else if value.kind == "relative_path" then
        std.record.has_field "max_bytes" value
        && std.array.length (std.record.fields value) == 2
        && is_int value.max_bytes
        && value.max_bytes >= 1
        && value.max_bytes <= 256
      else
        false
    )
  )
in

let valid_value_for_type = fun param_type value =>
  if param_type.kind == "bool" then
    std.is_bool value
  else if param_type.kind == "integer" then
    is_int value && value >= param_type.min && value <= param_type.max
  else if param_type.kind == "enum" then
    std.is_string value && std.array.elem value param_type.values
  else if param_type.kind == "relative_path" then
    is_valid_relative_path param_type.max_bytes value
  else
    false
in

let ParamBinding = {
  node | LatticeLabel,
  shell_env | ParamEnvKey,
}
in

let valid_param_default = fun param =>
  if param.required then
    !(std.record.has_field "default" param)
  else
    std.record.has_field "default" param
    && valid_value_for_type param.type param.default
in

let ParamDeclaration = {
  name | ParamName,
  type | ParamType,
  required | Bool,
  default | optional,
  bindings | Array ParamBinding | Between 1 8,
}
in

let LatticeNode = {
  id | LatticeLabel,
  kind | NodeKind,
  effect_class | EffectClass,
  adapter_kind | AdapterKind,
  operation_identity | c.Blake3,
}
in

let LatticeEdge = {
  source | LatticeLabel,
  target | LatticeLabel,
}
in

let LatticeSpec = {
  bundle_id | RunbookId | optional,
  bundle_identity | c.Blake3,
  workflow_name | LatticeLabel,
  workflow_revision | c.Blake3,
  nodes | Array LatticeNode | Between 1 32,
  edges | Array LatticeEdge | Between 0 64 | default = [],
}
in

let AllowedTarget = {
  repository | RepositoryName,
  project_id | ProjectId,
  references | Array Reference | Between 1 64 | Unique,
  allow_commit_sha | Bool,
}
in

let is_dag = fun node_ids edges =>
  let peel = fun remaining _ =>
    if remaining == [] then
      []
    else
      std.array.filter
        (fun node_id =>
          std.array.any
            (fun edge => edge.target == node_id && std.array.elem edge.source remaining)
            edges
        )
        remaining
  in
  std.array.fold_left peel node_ids node_ids == []
in

let valid_runbook_relations = fun runbook =>
  let node_ids = std.array.map (fun node => node.id) runbook.lattice.nodes in
  let edge_keys = std.array.map (fun edge => "%{edge.source}->%{edge.target}") runbook.lattice.edges in
  let param_names = std.array.map (fun param => param.name) runbook.params in
  let all_bindings =
    std.array.flatten (std.array.map (fun param => param.bindings) runbook.params)
  in
  let binding_keys =
    std.array.map (fun binding => "%{binding.node}:%{binding.shell_env}") all_bindings
  in
  let target_repos = std.array.map (fun target => target.repository) runbook.allowed_targets in
  let target_projects = std.array.map (fun target => target.project_id) runbook.allowed_targets in
  unique node_ids
  && unique edge_keys
  && std.array.all
    (fun edge =>
      edge.source != edge.target
      && std.array.elem edge.source node_ids
      && std.array.elem edge.target node_ids
    )
    runbook.lattice.edges
  && is_dag node_ids runbook.lattice.edges
  && unique param_names
  && std.array.all valid_param_default runbook.params
  && std.array.all (fun binding => std.array.elem binding.node node_ids) all_bindings
  && unique binding_keys
  && unique target_repos
  && unique target_projects
in

let RunbookDefinition = {
  id | RunbookId,
  display_name | DisplayText,
  description | DescriptionText,
  mode | RunbookMode,
  lattice | LatticeSpec,
  params | Array ParamDeclaration | Between 0 16 | default = [],
  allowed_targets | Array AllowedTarget | Between 1 64,
}
in

let Grant = {
  principal | PrincipalId,
  runbooks | Array RunbookId | Between 1 64 | Unique,
  not_after_ms | c.Positive | optional,
}
in

let RegistryRelations =
  std.contract.from_predicate (fun registry =>
    let runbook_ids = std.array.map (fun runbook => runbook.id) registry.runbooks in
    let grant_principals = std.array.map (fun grant => grant.principal) registry.grants in
    unique runbook_ids
    && unique grant_principals
    && std.array.all valid_runbook_relations registry.runbooks
    && std.array.all
      (fun grant =>
        std.array.all (fun id => std.array.elem id runbook_ids) grant.runbooks
      )
      registry.grants
  )
in

fun input =>
  let admitted =
    input
      | {
        runbooks | Array RunbookDefinition | Between 1 64,
        grants | Array Grant | Between 0 128 | default = [],
      }
      | RegistryRelations
  in
  {
    schema = "kiln.runbook-registry.v1",
    runbooks = admitted.runbooks,
    grants = admitted.grants,
  }
