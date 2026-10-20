# r[verify kiln.observe.profile.scenario.match]
let Source = import "../observation-profile.ncl" in
let Request = {
  state_revision
    | std.contract.from_predicate (fun value =>
      std.is_number value && value >= 0
    ),
  view | std.contract.from_predicate (fun value => value == "state_summary"),
}
in
{
  profile = Source.profile,
  request | Request = {
    state_revision = 3,
    view = "state_summary",
  },
}
