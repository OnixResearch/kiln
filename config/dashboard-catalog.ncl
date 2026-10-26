# r[impl kiln.dashboard.repositories.catalog]
# Paths resolve against the exported JSON catalog, never the ambient directory.
# All labels are operator declarations, not source verification claims.
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let Integer = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_number value && value >= 1 && value <= maximum && std.number.floor value == value
  )
in
let Label =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 1024
    && std.string.is_match "[^[:space:]]" value
    && !std.string.is_match "[\\x00-\\x1f\\x7f]" value
  )
in
let Identifier =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 128
    && std.string.is_match "^[A-Za-z0-9._-]+$" value
  )
in
let Path =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && !std.string.is_match "[\\x00-\\x1f\\x7f]" value
  )
in
let Stream =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[^:[:cntrl:]]+:[^:[:cntrl:]]+:[^[:cntrl:]]+$" value
  )
in
let Journal = { kind | Exact "journal", path | Path } in
let Store = { kind | Exact "store", path | Path, stream | Stream } in
let Input =
  std.contract.custom (fun label value =>
    if std.is_record value && std.record.has_field "kind" value then
      if value.kind == "journal" then
        std.contract.check Journal label value
      else if value.kind == "store" then
        std.contract.check Store label value
      else
        'Error { message = "unsupported history input kind" }
    else
      'Error { message = "history input requires a kind" }
  )
in
let Nullable = fun Contract =>
  std.contract.custom (fun label value =>
    if value == null then 'Ok value else std.contract.check Contract label value
  )
in
let History = {
  id | Identifier,
  label | Label,
  source | Nullable Label | optional,
  revision | Nullable Label | optional,
  input | Input,
}
in
let Project = { id | Identifier, label | Label, histories | Array History } in
let Bounds = {
  max_catalog_bytes | Integer 1048576,
  max_projects | Integer 32,
  max_histories | Integer 128,
  max_input_bytes | Integer 268435456,
  max_rendered_bytes | Integer 67108864,
}
in
let Shape = { schema_version | Exact 1, bounds | Bounds, projects | Array Project } in
let Unique = fun ids =>
  std.array.all
    (fun id =>
      std.array.length (std.array.filter (fun other => other == id) ids) == 1
    )
    ids
in
let Invariants =
  std.contract.from_predicate (fun value =>
    std.array.length value.projects > 0
    && std.array.length value.projects <= value.bounds.max_projects
    && Unique (std.array.map (fun project => project.id) value.projects)
    && std.array.all
      (fun project =>
        std.array.length project.histories > 0
        && Unique (std.array.map (fun history => history.id) project.histories)
      )
      value.projects
    && std.array.fold_left (fun count project => count + std.array.length project.histories) 0 value.projects <= value.bounds.max_histories
  )
in
fun settings => settings | Shape | Invariants
