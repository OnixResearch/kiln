# r[impl kiln.inventory.separation]
# This is an admission contract, not a provider discovery configuration.
let Integer = fun minimum maximum =>
  std.contract.from_predicate (fun value =>
    std.is_number value
    && value >= minimum
    && value <= maximum
    && std.number.floor value == value
  )
in
let PublicToken =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value >= 1
    && std.string.length value <= 256
    && std.string.is_match "^[A-Za-z0-9_.:@+-]+$" value
    && !(std.string.is_match "\\.\\." value)
  )
in
let Kind =
  std.contract.from_predicate (fun value =>
    std.array.elem value ["agents", "workers", "compute", "serving", "policy", "workflow_revisions"]
  )
in
let Source = {
  source | PublicToken,
  producer | PublicToken,
  scope | PublicToken,
  kind | Kind,
  # Trusted collector watermark; never derive this by lowering it to an old page.
  minimum_generation | Integer 0 9007199254740991,
}
in
let BoundedSources =
  std.contract.from_predicate (fun value =>
    std.is_array value && std.array.length value >= 1 && std.array.length value <= 64
  )
in
fun input =>
  let settings =
    input
      | {
        principal | PublicToken,
        sources | Array Source | BoundedSources,
        max_items | Integer 1 4096 | default = 128,
        max_candidates | Integer 1 128 | default = 32,
        max_age_ms | Integer 1 9007199254740991 | default = 30000,
        include_evidence | Bool | default = false,
      }
  in
  {
    schema = "kiln.observe.inventory.v2",
    principal = settings.principal,
    sources = settings.sources,
    max_items = settings.max_items,
    max_candidates = settings.max_candidates,
    max_age_ms = settings.max_age_ms,
    include_evidence = settings.include_evidence,
  }
