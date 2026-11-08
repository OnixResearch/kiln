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
let Nonempty256 =
  std.contract.from_predicate (fun value =>
    std.is_array value
    && std.array.length value > 0
    && std.array.length value <= 256
  )
in
let UniqueProjects =
  std.contract.from_predicate (fun projects =>
    let check = fun state project =>
      if std.array.elem project.project_id state.seen then
        { seen = state.seen, duplicate = true }
      else
        { seen = state.seen @ [project.project_id], duplicate = false }
    in
    !(std.array.fold_left check { seen = [], duplicate = false } projects).duplicate
  )
in
let UniquePaths =
  std.contract.from_predicate (fun values =>
    let check = fun state value =>
      if std.array.elem value state.seen then
        { seen = state.seen, duplicate = true }
      else
        { seen = state.seen @ [value], duplicate = false }
    in
    !(std.array.fold_left check { seen = [], duplicate = false } values).duplicate
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
            producers | Array Text | Nonempty256,
            toolchains | Array Text | Nonempty256,
            platforms | Array Text | Nonempty256,
            paths | Array Relative | Nonempty | UniquePaths,
          }
          | Nonempty256
          | UniqueProjects,
      }
      | std.contract.from_predicate (fun value =>
        value.maximum_total_bytes >= value.maximum_entry_bytes
      )
  in
  admitted & { schema = "kiln.web-cache.v1" }
