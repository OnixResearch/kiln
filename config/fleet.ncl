let Nullable = fun Contract =>
  std.contract.custom (fun label value =>
    if value == null then 'Ok value else 'Ok (std.contract.apply Contract label value)
  )
in
let NonEmptyString =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.length value > 0
  )
in
let is_absolute_path = fun value =>
  std.is_string value
  && std.string.is_match "^/" value
  && !(std.string.is_match "(^|/)\\.\\.(/|$)" value)
in
let AbsolutePath = std.contract.from_predicate is_absolute_path in
let NullableAbsolutePath =
  std.contract.from_predicate (fun value =>
    value == null || is_absolute_path value
  )
in
let NullableNonEmptyString =
  std.contract.from_predicate (fun value =>
    value == null || (std.is_string value && std.string.length value > 0)
  )
in
let is_machine_label = fun value =>
  std.is_string value
  && std.string.length value <= 63
  && std.string.is_match "^[a-z0-9][a-z0-9-]*$" value
in
let MachineLabel = std.contract.from_predicate is_machine_label in
let GroupName =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 64
    && std.string.is_match "^[A-Za-z0-9_-][A-Za-z0-9._-]*[A-Za-z0-9_-]$" value
  )
in
let Count =
  std.contract.from_predicate (fun value =>
    std.is_number value && std.number.is_integer value && value >= 0
  )
in
let Positive =
  std.contract.from_predicate (fun value =>
    std.is_number value && std.number.is_integer value && value > 0
  )
in
let is_ipv4_listen = fun value =>
  std.string.is_match "^(127\\.[0-9]+\\.[0-9]+\\.[0-9]+|100\\.(6[4-9]|[7-9][0-9]|1[01][0-9]|12[0-7])\\.[0-9]+\\.[0-9]+):[0-9]+$" value
in
let is_ipv6_listen = fun value =>
  std.string.is_match "^\\[(::1|fd7a:115c:a1e0:[0-9a-f:]*)\\]:[0-9]+$" value
in
# Loopback and tailnet (100.64.0.0/10, fd7a:115c:a1e0::/48) listen addresses only.
let ListenAddress =
  std.contract.from_predicate (fun value =>
    std.is_string value && (is_ipv4_listen value || is_ipv6_listen value)
  )
in
let NodeName =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 253
    && std.string.is_match "^[A-Za-z0-9._-]+$" value
  )
in
let Peers =
  std.contract.from_predicate (fun value =>
    std.is_record value
    && std.array.all
      (fun node => std.string.is_match "^[A-Za-z0-9._-]+$" node)
      (std.record.fields value)
    && std.array.all is_machine_label (std.record.values value)
  )
in
let Repository =
  std.contract.from_predicate (fun name =>
    std.is_string name
    && std.string.is_match "^[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$" name
  )
in
let CaptureRepositories =
  std.contract.from_predicate (fun value =>
    std.is_record value
    && std.array.all
      (fun name => std.string.is_match "^[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$" name)
      (std.record.fields value)
  )
in
let placeholders = [
  "{attempt}",
  "{fence}",
  "{database}",
  "{enrollment}",
  "{verification_profile}",
  "{authority}",
  "{handoff}",
  "{repository}",
]
in
# An absolute program followed by arguments; braces only as whole known
# placeholders, which the service substitutes.
let NullableArgv =
  std.contract.from_predicate (fun value =>
    value == null
    || (
      std.is_array value
      && std.array.length value > 0
      && std.array.length value <= 64
      && is_absolute_path (std.array.at 0 value)
      && std.array.all
        (fun argument =>
          std.is_string argument
          && (
            !(std.string.is_match "[{}]" argument)
            || std.array.elem argument placeholders
          )
        )
        value
    )
  )
in
# herdr agent kinds Kiln admits.
let AgentKind = [| 'omp, 'claude, 'codex, 'agy |] in
# The kiln-adapter-swarm `SwarmConfig`, field for field.
let Swarm = {
  enabled | Bool | default = true,
  executable | String | AbsolutePath,
  state_dir | String | AbsolutePath,
  config_file | NullableAbsolutePath | default = null,
  server | NullableNonEmptyString | default = null,
  group | String | GroupName | default = "bots.claims",
  from | String | NonEmptyString,
  timeout_ms | Number | Positive | default = 30000,
  read_limit | Number | Positive | default = 50,
  max_output_bytes | Number | Positive | default = 1048576,
  no_relays | Bool | default = true,
  environment | { _ : String } | default = {},
}
in
let from_address = fun from =>
  let found = std.string.find "<([^>]*)>" from in
  if found.index < 0 then std.string.trim from else std.array.at 0 found.groups
in
# Kiln posts with its own From: never `<machine>@swarm.local` of a listed machine.
let OwnFrom = fun machines =>
  std.contract.from_predicate (fun swarm =>
    swarm == null
    || (
      let address = std.string.lowercase (from_address swarm.from) in
      std.array.all (fun machine => address != "%{machine}@swarm.local") machines
    )
  )
in
let Capture = {
  git | String | AbsolutePath,
  tar | String | AbsolutePath,
  export_root | String | AbsolutePath,
  # Files named `<verification profile content id>.json`.
  verification_profiles | String | AbsolutePath,
  repositories | CaptureRepositories | { _ : { authority | String | AbsolutePath } },
  verify_argv | NullableArgv | default = null,
  publish_argv | NullableArgv | default = null,
}
in
let Policy = {
  tick_ms | Number | Positive | default = 15000,
  attempt_lease_ms | Number | Positive | default = 3600000,
  renew_before_ms | Number | Positive | default = 600000,
  blocked_decision_after_ms | Number | Positive | default = 300000,
  nudge_after_ms | Number | Positive | default = 600000,
  max_nudges | Number | Count | default = 2,
  claim_lease_ms | Number | Positive | default = 14400000,
  max_claim_lease_ms | Number | Positive | default = 86400000,
}
in
# `kiln-fleet` service configuration, schema kiln.fleet.v1. The service
# repeats every check at load and also refuses unknown fields.
{
  schema = "kiln.fleet.v1",
  # Coordinator machine label.
  machine | String | MachineLabel,
  listen | Array ListenAddress,
  # Tailscale node name (as `tailscale whois` reports it) to machine label.
  peers | Peers | default = {},
  # Machines that may cancel attempts and answer decisions.
  operators
    | Array MachineLabel
    | std.contract.from_predicate (fun operators =>
      std.array.all
        (fun operator => operator == machine || std.array.elem operator (std.record.values peers))
        operators
    )
    | default
    = [],
  tailscale | String | AbsolutePath,
  implementer | { database | String | AbsolutePath, enrollment | String | AbsolutePath },
  # kiln.fleet-inventory.v1 JSON (config/fleet-inventory.ncl).
  inventory | String | AbsolutePath,
  swarm
    | OwnFrom ([machine] @ std.record.values peers @ operators)
    | Nullable Swarm
    | default
    = null,
  decision_group | String | GroupName | default = "bots.general",
  capture | Nullable Capture | default = null,
  policy | Policy | default = {},
  dispatch
    | {
      enabled | Bool,
      kinds | Array AgentKind,
    }
    | std.contract.from_predicate (fun dispatch =>
      !dispatch.enabled
      || (std.array.length dispatch.kinds > 0 && capture != null)
    ),
}
