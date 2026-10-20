let Source = import "ucan-observation-positive.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
let auth = replace "max_token_bytes" 0 Source.profile.auth in
replace "auth" auth Source.profile | std.contract.from_predicate (fun value => value == Source.profile)
