# Shared admission contracts for hosted Nickel records, reused by the service
# and legacy execution profiles.
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
  CacheText =
    std.contract.from_predicate (fun value =>
      std.is_string value
      && std.string.length value > 0
      && std.string.length value <= 256
      && std.string.is_match "^[ -~]+$" value
    ),
  Audience =
    std.contract.from_predicate (fun value =>
      std.is_string value
      && std.string.length value > 0
      && std.string.length value <= 512
      && !(std.string.is_match "[\\x00-\\x20\\x7f]" value)
    ),
  Environment =
    std.contract.from_predicate (fun value =>
      std.is_string value
      && std.string.length value <= 128
      && std.string.is_match "^[A-Za-z_][A-Za-z0-9_]*$" value
    ),
  CacheRelative =
    std.contract.from_predicate (fun value =>
      std.is_string value
      && std.string.length value > 0
      && std.string.length value <= 1024
      && std.string.is_match "^[^/\\\\\\x00-\\x1f\\x7f]+(/[^/\\\\\\x00-\\x1f\\x7f]+)*$" value
      && !(std.string.is_match "(^|/)\\.\\.?(/|$)" value)
    ),
  RecordAtMost = fun maximum =>
    std.contract.from_predicate (fun value =>
      std.is_record value && std.array.length (std.record.fields value) <= maximum
    ),
  GuestShell =
    std.contract.from_predicate (fun value =>
      std.is_string value
      && std.string.length value > 0
      && std.string.length value <= 4096
      && std.string.is_match "^/" value
      && !(std.string.is_match "[\\x00-\\x1f\\x7f]" value)
      && !(std.string.is_match "(^|/)\\.\\.(/|$)" value)
    ),
  WorkloadAudience =
    std.contract.from_predicate (fun value =>
      std.is_string value
      && std.string.length value > 0
      && std.string.length value <= 256
      && !(std.string.is_match "[\\x00-\\x20\\x7f]" value)
    ),
  HexIdentity =
    std.contract.from_predicate (fun value =>
      std.is_string value && std.string.is_match "^[0-9a-fA-F]{64}$" value
    ),
  PathUnder = fun root path =>
    let parts = fun value => std.array.filter (fun part => part != "") (std.string.split "/" value)
    in
    let root_parts = parts root in
    let path_parts = parts path in
    std.array.length path_parts >= std.array.length root_parts
    && std.array.slice 0 (std.array.length root_parts) path_parts == root_parts,
  Ipv4Group = fun value =>
    let octets = std.string.split "." value in
    std.array.length octets == 4
    && std.array.all
      (fun octet =>
        std.string.length octet > 0
        && std.string.is_match "^[0-9]+$" octet
        && (octet == "0" || !(std.string.is_match "^0" octet))
        && (
          let number = std.string.to_number octet in
          std.is_number number && number >= 0 && number <= 255 && std.number.floor number == number
        )
      )
      octets,
  Ipv6GroupCount = fun host =>
    let parts = std.array.filter (fun part => part != "") (std.string.split ":" host) in
    let compressed = std.string.is_match "::" host in
    let dotted = std.array.filter (fun part => std.string.is_match "\\." part) parts in
    let hex = std.array.filter (fun part => !(std.string.is_match "\\." part)) parts in
    let hex_ok =
      std.array.all (fun part => std.string.is_match "^[0-9a-fA-F]{1,4}$" part) hex
    in
    let groups = std.array.length parts in
    hex_ok
    && std.array.length (std.string.split "::" host) <= 2
    && !(std.string.is_match ":::" host)
    && (
      if !(std.array.length dotted == 0) then
        std.array.length dotted == 1
        && std.array.last parts == std.array.first dotted
        && Ipv4Group (std.array.last parts)
        && (
          if compressed then
            groups <= 6
          else
            groups == 7
        )
      else if compressed then
        groups <= 7
      else
        groups == 8
    ),
}
