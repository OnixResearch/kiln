# Typed builder for the enrolled implementation repositories and actors.
# Export to the JSON enrollment consumed by `kiln-implementer`.
let UniqueNonEmpty =
  std.contract.from_predicate (fun value =>
    std.is_array value
    && std.array.length value > 0
    && std.array.all
      (fun item =>
        std.is_string item
        && std.string.length item > 0
        && std.string.length item <= 256
        && !std.string.is_match "[\\x00-\\x1f\\x7f]" item
      )
      value
    && std.array.length (
      std.array.fold_left
        (fun seen item => if std.array.elem item seen then seen else seen @ [item])
        []
        value
    ) == std.array.length value
  )
in
fun input =>
  let settings =
    input
      | {
        repositories | UniqueNonEmpty,
        actors | UniqueNonEmpty,
      }
  in
  {
    repositories = settings.repositories,
    actors = settings.actors,
  }
