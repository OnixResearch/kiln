let Source = import "../radicle-mutation-profile.ncl" in
let _Profile = Source.profile in
"3333333333333333333333333333333333333333" | std.contract.from_predicate (fun value => value == "2222222222222222222222222222222222222222")