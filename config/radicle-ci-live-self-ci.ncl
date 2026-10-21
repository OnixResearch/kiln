# r[impl kiln.self_ci.authority] r[impl kiln.self_ci.source]
# Pure admission only; fork and source effects are owned by the imperative shell.
let Base = import "radicle-ci-live-distinct-status.ncl" in
let Commit =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[0-9a-f]{40}$" value
  )
in
let Range = fun low high =>
  std.contract.from_predicate (fun value =>
    std.is_number value && std.number.is_integer value && value >= low && value <= high
  )
in
fun inputs =>
  let base = Base (std.record.remove "self_ci" inputs) in
  let selected
    | {
      self_ci
        | {
          base_commit | Commit,
          root_selector | String | std.contract.from_predicate (fun value => value == "packages.x86_64-linux"),
          selected_task | String | std.contract.from_predicate (fun value => value == "kiln"),
          product_path | String | std.contract.from_predicate (fun value => value == "bin/kiln"),
          failure_marker | String | std.contract.from_predicate (fun value => value == "self_ci_genuine_failure"),
          unavailable_settle_seconds | Number | Range 1 120,
          source_limits
            | {
              max_identity_bytes | Number | std.contract.from_predicate (fun value => value == 4096),
              max_reference_bytes | Number | std.contract.from_predicate (fun value => value == 512),
              max_entries | Number | Range 4096 8192,
              max_path_bytes | Number | std.contract.from_predicate (fun value => value == 1024),
              max_file_bytes | Number | Range 1650000 4194304,
              max_total_bytes | Number | Range 54609295 67108864,
            },
        },
      ..
    }
    = inputs
  in
  base
  & {
    command_seconds | force = 1800,
    recovery_seconds | force = 1800,
    callback_timeout_milliseconds | force = 1200000,
    maximum_polls | force = 600,
    self_ci = {
      base_commit = selected.self_ci.base_commit,
      root_selector = selected.self_ci.root_selector,
      selected_task = selected.self_ci.selected_task,
      product_path = selected.self_ci.product_path,
      failure_marker = selected.self_ci.failure_marker,
      unavailable_settle_seconds = selected.self_ci.unavailable_settle_seconds,
      source_limits = selected.self_ci.source_limits,
    },
  }
