let Source = import "../iroh-observation-profile.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
let endpoint = replace "max_connections" 0 Source.profile.endpoint in
replace "endpoint" endpoint Source.profile | std.contract.from_predicate (fun value => value == Source.profile)
