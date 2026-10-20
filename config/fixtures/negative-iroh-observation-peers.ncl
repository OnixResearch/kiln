let Source = import "../iroh-observation-profile.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
let peers = replace "allowed" [] Source.profile.peers in
replace "peers" peers Source.profile | std.contract.from_predicate (fun value => value == Source.profile)
