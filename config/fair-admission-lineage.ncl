# r[impl kiln.admission.profiles]
# Typed recursive budget-lineage profile. Exporting it grants nothing: the
# operator still roots the campaign and each transfer still needs a current
# parent attempt/fence in the selected implementation store. Kiln re-admits the
# export with the same bounds and derives BLAKE3 account identities from names.
let Integer = fun minimum maximum =>
  std.contract.from_predicate (fun x =>
    std.is_number x && x >= minimum && x <= maximum && x == std.number.truncate x
  )
in
let Token =
  std.contract.from_predicate (fun x =>
    std.is_string x
    && std.string.length x >= 1
    && std.string.length x <= 128
    && std.string.is_match "^[A-Za-z0-9_.-]+$" x
  )
in
let Blake3 =
  std.contract.from_predicate (fun x =>
    std.is_string x && std.string.is_match "^[0-9a-f]{64}$" x
  )
in
let Amounts =
  std.contract.from_predicate (fun x =>
    std.is_record x
    && std.array.length (std.record.fields x) >= 1
    && std.array.length (std.record.fields x) <= 16
    && std.array.all
      (fun key =>
        std.string.is_match "^[A-Za-z0-9_.-]{1,128}$" key
        && std.is_number (std.record.get key x)
        && std.record.get key x > 0
        && std.record.get key x <= 9007199254740991
        && std.record.get key x == std.number.truncate (std.record.get key x)
      )
      (std.record.fields x)
  )
in
let Account = {
  name | Token,
  parent | Token,
  deadline_ms | Integer 1 9007199254740991,
  allocation | Amounts,
}
in
fun input =>
  let s =
    input
      | {
        campaign | Token,
        tenant | Token,
        max_depth | Integer 1 8,
        max_children | Integer 1 4096,
        root | { deadline_ms | Integer 1 9007199254740991, quota | Amounts },
        accounts | Array Account | std.contract.from_predicate (fun x => std.array.length x <= 4095),
      }
  in
  # Each parent must be `root` or an earlier account, so the lineage is acyclic.
  let state =
    std.array.fold_left
      (fun acc a =>
        let known = std.record.has_field a.parent acc.nodes in
        let parent = if known then std.record.get a.parent acc.nodes else { depth = 0, deadline_ms = 0, remaining = {}, children = 0 } in
        let fits =
          known
          && !(std.record.has_field a.name acc.nodes)
          && parent.depth
          + 1 <= s.max_depth
          && parent.children
          + 1 <= s.max_children
          && a.deadline_ms <= parent.deadline_ms
          && std.array.all
            (fun k =>
              std.record.has_field k parent.remaining
              && std.record.get k a.allocation <= std.record.get k parent.remaining
            )
            (std.record.fields a.allocation)
        in
        if !fits || !acc.ok then
          { ok = false, nodes = acc.nodes }
        else
          let left =
            std.record.map
              (fun k v =>
                if std.record.has_field k a.allocation then v - std.record.get k a.allocation else v
              )
              parent.remaining
          in
          let updated = {
            depth = parent.depth,
            deadline_ms = parent.deadline_ms,
            remaining = left,
            children = parent.children + 1,
          }
          in
          let child = { depth = parent.depth + 1, deadline_ms = a.deadline_ms, remaining = a.allocation, children = 0 } in
          {
            ok = true,
            nodes =
              (std.record.remove a.parent acc.nodes)
              & { "%{a.parent}" = updated }
              & { "%{a.name}" = child },
          }
      )
      {
        ok = true,
        nodes = { root = { depth = 0, deadline_ms = s.root.deadline_ms, remaining = s.root.quota, children = 0 } },
      }
      s.accounts
  in
  # Export forces every field, so an invalid lineage cannot produce output.
  let checked =
    if state.ok then
      s
    else
      std.fail_with "lineage profile exceeds parent quota, deadline, depth or fan-out"
  in
  {
    version = 1,
    campaign = checked.campaign,
    tenant = checked.tenant,
    max_depth = checked.max_depth,
    max_children = checked.max_children,
    root = checked.root,
    accounts = checked.accounts,
  }
