# Optional campaign controls for `kiln-web-service serve --campaign-controls`.
# Grants are exact principals narrowed to campaigns, commands and review roles;
# definitions are frozen campaign contracts with their admitted delivery route.
# The service rereads the exported profile for every control request.
let c = import "github-contracts.ncl" in
let Token =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 128
    && std.string.is_match "^[A-Za-z0-9_.:@/-]+$" value
  )
in
let CampaignId =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 256
    && std.string.trim value != ""
    && !(std.string.is_match "[\\x00-\\x1f\\x7f]" value)
  )
in
let Command =
  std.contract.from_predicate (fun value =>
    std.array.elem value ["start", "continue", "pause", "cancel", "issue_decision", "resolve_decision"]
  )
in
let Route =
  std.contract.from_predicate (fun value => std.array.elem value ["github_pull_request", "native_git"])
in
let unique = fun values =>
  let check = fun state value =>
    if std.array.elem value state.seen then
      { seen = state.seen, duplicate = true }
    else
      { seen = state.seen @ [value], duplicate = state.duplicate }
  in
  !(std.array.fold_left check { seen = [], duplicate = false } values).duplicate
in
let Unique = std.contract.from_predicate unique in
let Between = fun minimum maximum =>
  std.contract.from_predicate (fun values =>
    std.array.length values >= minimum && std.array.length values <= maximum
  )
in
let Grant = {
  principal | Token,
  campaigns | Array CampaignId | Between 1 64 | Unique,
  commands | Array Command | Between 1 6 | Unique,
  review_roles | Array Token | Between 0 32 | Unique | default = [],
  not_after_ms
    | std.contract.from_predicate (fun value => std.is_number value && value > 0 && value % 1 == 0)
    | optional,
}
in
let Definition = {
  contract | c.AbsolutePath,
  delivery_route | Route,
}
in
let DistinctRows =
  std.contract.from_predicate (fun profile =>
    unique (std.array.map (fun grant => grant.principal) profile.grants)
    && unique (std.array.map (fun definition => definition.contract) profile.definitions)
  )
in
fun input =>
  let admitted =
    input
      | {
        campaign_database | c.AbsolutePath,
        grants | Array Grant | Between 1 128,
        definitions | Array Definition | Between 0 64 | default = [],
      }
      | DistinctRows
  in
  {
    schema = "kiln.web-campaign-controls.v1",
    campaign_database = admitted.campaign_database,
    grants = admitted.grants,
    definitions = admitted.definitions,
  }
