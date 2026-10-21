# r[impl kiln.radicle_ci.distinct_host.authority]
# Pure admission only; remote effects are owned by the imperative shell.
let Base = import "radicle-ci-live.ncl" in
let RemoteRoot =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^/var/tmp/[^/\\x00]+$" value
    && !std.string.is_match "(^|/)\\.{1,2}(/|$)" value
  )
in
let SshDestination =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^[A-Za-z0-9][A-Za-z0-9_.@:-]*$" value
    && !std.string.is_match "[[:space:];&|`$()<>]" value
  )
in
let TailnetAddress =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^100\\.(6[4-9]|[7-9][0-9]|1[01][0-9]|12[0-7])\\.[0-9]{1,3}\\.[0-9]{1,3}$" value
  )
in
let Port =
  std.contract.from_predicate (fun value =>
    std.is_number value
    && std.number.is_integer value
    && value >= 1024
    && value <= 65535
  )
in
let Readiness =
  std.contract.from_predicate (fun value =>
    std.is_number value
    && std.number.is_integer value
    && value >= 1
    && value <= 300
  )
in
fun inputs =>
  let base = Base (std.record.remove "remote" inputs) in
  let selected
    | {
      remote
        | {
          host | SshDestination,
          root | RemoteRoot,
          address | TailnetAddress,
          port | Port,
          readiness_seconds | Readiness,
        },
      ..
    }
    = inputs
  in
  base
  & {
    remote = {
      host = selected.remote.host,
      root = selected.remote.root,
      address = selected.remote.address,
      port = selected.remote.port,
      readiness_seconds = selected.remote.readiness_seconds,
    },
  }
