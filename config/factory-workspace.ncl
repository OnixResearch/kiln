# Single typed workspace manifest for Kiln Factory (`kiln.factory-workspace.v1`).
# Export as JSON (e.g. `nickel export config/factory-workspace.ncl > kiln.json`)
# and pass `--config kiln.json` to `kiln-web-service check` or `serve`.
let MaxPathBytes = 4096 in
let MaxRepositoryRoots = 64 in

let is_valid_path = fun value =>
  std.is_string value
  && std.string.length value > 0
  && std.string.length value <= MaxPathBytes
  && !std.string.is_match "[\\x00-\\x1f\\x7f]" value
in

let SchemaVersion =
  std.contract.from_predicate (fun value =>
    std.is_string value && value == "kiln.factory-workspace.v1"
  )
in

let ConfigPath =
  std.contract.from_predicate is_valid_path
in

let NullableConfigPath =
  std.contract.from_predicate (fun value =>
    value == null || is_valid_path value
  )
in

let BoundedRepositoryRoots =
  std.contract.from_predicate (fun roots =>
    std.is_array roots && std.array.length roots <= MaxRepositoryRoots
  )
in

let CiSection = {
  automation | NullableConfigPath,
  workers | NullableConfigPath,
  cache | NullableConfigPath,
  runbooks | NullableConfigPath,
  runbooks_dir | NullableConfigPath,
  execution_artifacts | NullableConfigPath,
}
in

let ReviewSection = {
  reviews | NullableConfigPath,
  code_review | NullableConfigPath,
  work | NullableConfigPath,
  cobs | NullableConfigPath,
  artifact_previews | NullableConfigPath,
  preview_observations | NullableConfigPath,
}
in

let CairnSection = {
  authoring | NullableConfigPath,
  lattice_authoring | NullableConfigPath,
  lattice_drafts | NullableConfigPath,
}
in

let ObservationsSection = {
  inventory | NullableConfigPath,
  implementation_work | NullableConfigPath,
  campaign_controls | NullableConfigPath,
  dispatch_recovery | NullableConfigPath,
  native_runtime_evidence | NullableConfigPath,
  delivery_observations | NullableConfigPath,
}
in

let FactoryWorkspace = {
  schema | SchemaVersion,
  profile | ConfigPath,
  access | NullableConfigPath,
  sources | NullableConfigPath,
  fleet | NullableConfigPath,
  ci | CiSection,
  review | ReviewSection,
  cairn | CairnSection,
  observations | ObservationsSection,
  repository_roots | Array ConfigPath | BoundedRepositoryRoots,
}
in

({
  schema = "kiln.factory-workspace.v1",
  profile = "service.json",
  access = "access.json",
  sources = "sources.json",
  fleet = null,
  ci = {
    automation = null,
    workers = null,
    cache = null,
    runbooks = null,
    runbooks_dir = null,
    execution_artifacts = null,
  },
  review = {
    reviews = null,
    code_review = null,
    work = null,
    cobs = null,
    artifact_previews = null,
    preview_observations = null,
  },
  cairn = {
    authoring = null,
    lattice_authoring = null,
    lattice_drafts = null,
  },
  observations = {
    inventory = null,
    implementation_work = null,
    campaign_controls = null,
    dispatch_recovery = null,
    native_runtime_evidence = null,
    delivery_observations = null,
  },
  repository_roots = [],
} | FactoryWorkspace)
