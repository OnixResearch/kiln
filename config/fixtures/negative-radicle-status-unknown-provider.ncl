let Source = import "../radicle-status-profile.ncl" in
"unknown-provider" | std.contract.from_predicate (fun value => value == Source.profile.signer.provider)