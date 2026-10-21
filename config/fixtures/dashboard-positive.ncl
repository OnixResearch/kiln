# r[verify kiln.observe.dashboard.profile.scenario.match]
let Source = import "../dashboard-profile.ncl" in
let Selection = {
  run
    | std.contract.from_predicate (fun value =>
      std.is_string value && value == "run-1"
    ),
}
in
{
  profile = Source.profile,
  selection | Selection = {
    run = "run-1",
  },
}
