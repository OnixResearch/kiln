let Source = import "ucan-observation-positive.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
let auth = replace "revoked_proof_references" ["not-a-digest"] Source.profile.auth in
replace "auth" auth Source.profile | std.contract.from_predicate (fun value => value == Source.profile)
