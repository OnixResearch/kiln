# r[impl kiln.radicle_ci.distinct_status.authority]
# Pure admission only; remote effects are owned by the imperative shell.
let Base = import "radicle-ci-live-distinct-host.ncl" in
let TailnetAddress =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.is_match "^100\\.(6[4-9]|[7-9][0-9]|1[01][0-9]|12[0-7])\\.[0-9]{1,3}\\.[0-9]{1,3}$" value
  )
in
fun inputs =>
  let base = Base (std.record.remove "local_address" inputs) in
  let selected
    | {
      local_address | TailnetAddress,
      ..
    }
    = inputs
  in
  base
  & {
    local_address = selected.local_address,
  }
