let Source = import "../radicle-status-profile.ncl" in
let _Profile = Source.profile in
"rad:zForeignRepository" | std.contract.from_predicate (fun value => value == "rad:zFixtureRepository")