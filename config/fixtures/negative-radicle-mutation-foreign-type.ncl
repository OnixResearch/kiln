let Source = import "../radicle-mutation-profile.ncl" in
"xyz.radicle.issue" | std.contract.from_predicate (fun value => value == Source.profile.object_type)