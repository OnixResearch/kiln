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
# Explicit, default-off swarm mirror profile. Connection fields are present in a
# complete profile; `enabled` must be true before any client process starts.
{
  schema = "kiln.swarm-mirror.v1",
  enabled | Bool | default = false,
  executable | String | NonEmptyString,
  state_dir | String | NonEmptyString,
  group | String | NonEmptyString | default = "bots.claims",
  from | String | NonEmptyString,
  timeout_ms | Number | Positive | default = 30000,
  read_limit | Number | Positive | default = 50,
  no_relays | Bool | default = true,
}
