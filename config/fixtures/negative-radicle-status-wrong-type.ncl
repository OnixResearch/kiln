let Source = import "../radicle-status-profile.ncl" in
"xyz.radicle.issue" | std.contract.from_predicate (fun value => value == Source.profile.allowed_object_type)