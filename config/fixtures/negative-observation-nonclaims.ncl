let Source = import "../observation-profile.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
let non_claims = std.array.drop 1 Source.profile.non_claims in
replace "non_claims" non_claims Source.profile | std.contract.from_predicate (fun value => value == Source.profile)
