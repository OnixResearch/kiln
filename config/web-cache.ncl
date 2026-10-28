# Optional server-owned authenticated shared cache. Export and pass --cache.
# directory must already exist, be private (0700), and not overlap service state.
let c = import "github-contracts.ncl" in
let Bounded = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_number value && value >= 1 && value <= maximum && value == std.number.floor value
  )
in
let Text =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 256
    && std.string.is_match "^[ -~]+$" value
  )
in
let Relative =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 1024
    && !(std.string.is_match "(^/|(^|/)\\.\\.?(/|$)|\\\\)" value)
  )
in
let Nonempty =
  std.contract.from_predicate (fun value =>
    std.is_array value
    && std.array.length value > 0
    && std.array.length value <= 4096
  )
in
let Identifier =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 128
    && std.string.is_match "^[A-Za-z0-9][A-Za-z0-9_.-]*$" value
  )
in
fun input =>
  let admitted =
    input
      | {
        directory | c.AbsolutePath,
        maximum_entry_bytes | Bounded 8388608,
        maximum_total_bytes | Bounded 1099511627776,
        maximum_entries | Bounded 100000,
        maximum_age_seconds | Bounded 31536000,
        maximum_files | Bounded 4096,
        projects
          | Array {
            project_id | Identifier,
            producers | Array Text | Nonempty,
            toolchains | Array Text | Nonempty,
            platforms | Array Text | Nonempty,
            paths | Array Relative | Nonempty,
          }
          | Nonempty,
      }
  in
  admitted & { schema = "kiln.web-cache.v1" }
