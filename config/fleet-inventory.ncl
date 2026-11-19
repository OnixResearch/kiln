let NonEmptyString =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.length value > 0
  )
in
let Positive =
  std.contract.from_predicate (fun value =>
    std.is_number value && value > 0
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
let MachineLabel =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[a-z0-9][a-z0-9-]*$" value
  )
in
let SessionName =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[A-Za-z0-9_][A-Za-z0-9_-]*$" value
  )
in
# Slots per agent kind on one machine: 1 to MAX_SLOTS_PER_KIND (16).
let SlotCount =
  std.contract.from_predicate (fun value =>
    std.is_number value
    && std.number.is_integer value
    && value >= 1
    && value <= 16
  )
in
# `owner/name` to an absolute checkout path on that machine.
let Repositories =
  std.contract.from_predicate (fun value =>
    std.is_record value
    && std.array.all
      (fun name => std.string.is_match "^[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$" name)
      (std.record.fields value)
    && std.array.all is_absolute_path (std.record.values value)
  )
in
# herdr agent kinds Kiln admits.
let AgentKind = [| 'omp, 'claude, 'codex, 'agy |] in
let AgentSlots = {
  kind | AgentKind,
  slots | Number | SlotCount,
  args | Array String | default = [],
}
in
let Machine = {
  label | String | MachineLabel,
  # Saved herdr machine label; null uses the local headless session.
  herdr_machine | NullableNonEmptyString | default = null,
  # Where Kiln fetches attempt commits over ssh. Only the coordinator may
  # leave it null; Kiln then reads that machine's worktrees directly.
  ssh_target | NullableNonEmptyString | default = null,
  worktree_root | String | AbsolutePath,
  repositories | Repositories | default = {},
  agents | Array AgentSlots | default = [],
}
in
# Machines and agent slots the herdr adapter may place interactive attempts on.
# Every herdr child runs with a cleared environment plus `environment`.
{
  schema = "kiln.fleet-inventory.v1",
  executable | String | AbsolutePath,
  session | String | SessionName | default = "kiln",
  environment
    | {
      home | String | AbsolutePath,
      path | String | NonEmptyString,
      xdg_runtime_dir | NullableAbsolutePath | default = null,
      xdg_config_home | NullableAbsolutePath | default = null,
      ssh_auth_sock | NullableAbsolutePath | default = null,
    },
  timeout_ms | Number | Positive | default = 30000,
  start_timeout_ms | Number | Positive | default = 60000,
  max_output_bytes | Number | Positive | default = 1048576,
  machines | Array Machine,
}
