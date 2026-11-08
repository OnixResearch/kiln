# Shared admission contracts for hosted Nickel records: exact fields and CPU
# affinity grammar shared by the service and legacy execution profiles.
{
  ExactFields = fun allowed =>
    fun value =>
      std.array.all (fun field => std.array.elem field allowed) (std.record.fields value),
  CpuAffinity =
    std.contract.from_predicate (fun value =>
      std.is_string value
      && std.string.length value > 0
      && std.string.length value <= 256
      && !(std.string.is_match "\\x00" value)
      && std.array.all
        (fun item =>
          let parts = std.string.split "-" item in
          std.array.length parts >= 1
          && std.array.length parts <= 2
          && std.array.all (fun part => std.string.length part > 0 && std.string.is_match "^[0-9]+$" part) parts
          && (
            let start = std.string.to_number (std.array.first parts) in
            let end = if std.array.length parts == 1 then start else std.string.to_number (std.array.last parts) in
            std.is_number start
            && std.is_number end
            && start >= 0
            && end <= 4095
            && start <= end
            && std.number.floor start == start
            && std.number.floor end == end
          )
        )
        (std.string.split "," value)
      && (
        let entries =
          std.array.map
            (fun item =>
              let parts = std.string.split "-" item in
              {
                start = std.string.to_number (std.array.first parts),
                end =
                  if std.array.length parts == 1 then
                    std.string.to_number (std.array.first parts)
                  else
                    std.string.to_number (std.array.last parts),
              }
            )
            (std.string.split "," value)
        in
        let cpus =
          std.array.flat_map
            (fun entry => std.array.generate (fun offset => entry.start + offset) (entry.end - entry.start + 1))
            entries
        in
        let check = fun state cpu =>
          if std.array.elem cpu state.seen then
            { seen = state.seen, duplicate = true }
          else
            { seen = state.seen @ [cpu], duplicate = false }
        in
        !(std.array.fold_left check { seen = [], duplicate = false } cpus).duplicate
      )
    ),
  Identifier =
    std.contract.from_predicate (fun value =>
      std.is_string value
      && std.string.length value <= 128
      && std.string.is_match "^[A-Za-z0-9_.-]+$" value
    ),
}
