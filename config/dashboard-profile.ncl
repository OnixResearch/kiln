# r[impl kiln.observe.dashboard.profile]
# r[impl kiln.observe.dashboard.profile.scenario.match]
let Observation = import "observation-profile.ncl" in
let NonEmptyString =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.length value > 0
  )
in
let Positive =
  std.contract.from_predicate (fun value =>
    std.is_number value && value > 0
  )
in
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let Profile = {
  schema_version | Exact 1,
  profile_id | NonEmptyString,
  observation,
  run_page_limit | Positive,
  max_run_pages | Positive,
  event_limit | Positive,
  watch_interval_ms | Positive,
  non_claims | Array NonEmptyString,
}
in
{
  profile | Profile = {
    schema_version = 1,
    profile_id = "dashboard-v1",
    observation = Observation.profile,
    run_page_limit = 16,
    max_run_pages = 8,
    event_limit = 32,
    watch_interval_ms = 1000,
    non_claims = [
      "dashboard rendering does not prove CI success",
      "dashboard rendering does not prove provider correctness",
      "dashboard rendering does not prove source trust",
      "dashboard rendering does not prove journal completeness",
      "dashboard rendering does not establish release readiness",
      "dashboard rendering does not prove the observed revision is the newest existing revision",
    ],
  },
}
