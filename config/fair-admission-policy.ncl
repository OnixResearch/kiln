# r[impl kiln.admission.bounded]
# r[impl kiln.admission.profiles]
# Exporting this contract does not enable admission. The operator must explicitly
# configure the selected durable implementation store with the exported policy.
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
let PositiveMap =
  std.contract.from_predicate (fun x =>
    std.is_record x
    && std.array.length (std.record.fields x) >= 1
    && std.array.length (std.record.fields x) <= 4096
    && std.array.all
      (fun key =>
        std.string.length key >= 1
        && std.string.length key <= 128
        && std.string.is_match "^[A-Za-z0-9_.-]+$" key
        && std.is_number (std.record.get key x)
        && std.record.get key x > 0
        && std.record.get key x <= 9007199254740991
        && std.record.get key x == std.number.truncate (std.record.get key x)
      )
      (std.record.fields x)
  )
in
let Group = {
  scope | std.contract.from_predicate (fun x => std.array.elem x ["tenant", "repository", "campaign"]),
  group | Token,
  max_items | Integer 1 4096,
  max_bytes | Integer 1 16777216,
  max_active | Integer 1 4096,
}
in
# Optional serving-owner snapshot gate for worker requests. A snapshot is
# evidence for waiting only; the owner's request grant still decides dispatch.
# Absent keeps the existing request path and policy identity.
let Serving = {
  max_snapshot_age_ms | Integer 1 9007199254740991,
  capacity_rule | std.contract.from_predicate (fun x => std.array.elem x ["known_required", "owner_admission_decides"]),
}
in
fun input =>
  let s =
    input
      | {
        accounting_basis | Blake3,
        max_items | Integer 1 4096,
        max_bytes | Integer 1 16777216,
        max_wait_ms | Integer 1 9007199254740991,
        max_observation_age_ms | Integer 1 9007199254740991,
        max_retry_after_ms | Integer 1 9007199254740991,
        max_children | Integer 1 4096,
        max_active | Integer 1 4096,
        strategy | std.contract.from_predicate (fun x => std.array.elem x ["weighted_scalar", "dominant_share"]),
        weights | PositiveMap,
        shares | PositiveMap,
        group_limits | Array Group | std.contract.from_predicate (fun x => std.array.length x <= 4096),
        serving | Serving | optional,
      }
  in
  let Checked =
    std.contract.from_predicate (fun x =>
      x.max_active <= x.max_items
      && std.array.all
        (fun g =>
          g.max_items <= x.max_items
          && g.max_bytes <= x.max_bytes
          && g.max_active <= g.max_items
        )
        x.group_limits
      && std.array.length (std.record.fields x.weights) <= 16
      && std.array.length (
        std.array.fold_left
          (fun seen g =>
            let k = g.scope ++ ":" ++ g.group in
            if std.array.elem k seen then seen else seen @ [k]
          )
          []
          x.group_limits
      ) == std.array.length x.group_limits
      && (
        !(std.record.has_field "serving" x)
        || x.serving.max_snapshot_age_ms <= x.max_wait_ms
      )
    )
  in
  let checked = s | Checked in
  {
    version = 1,
    accounting_basis = checked.accounting_basis,
    max_items = checked.max_items,
    max_bytes = checked.max_bytes,
    max_wait_ms = checked.max_wait_ms,
    max_observation_age_ms = checked.max_observation_age_ms,
    max_retry_after_ms = checked.max_retry_after_ms,
    max_children = checked.max_children,
    max_active = checked.max_active,
    shares = checked.shares,
    group_limits = checked.group_limits,
    strategy =
      if checked.strategy == "dominant_share" then
        "DominantShare"
      else
        { WeightedScalar = { weights = checked.weights } },
  }
  & (
    if std.record.has_field "serving" checked then
      {
        serving = {
          max_snapshot_age_ms = checked.serving.max_snapshot_age_ms,
          capacity_rule = checked.serving.capacity_rule,
        }
      }
    else
      {}
  )
