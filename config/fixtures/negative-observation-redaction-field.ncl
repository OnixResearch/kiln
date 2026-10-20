let Source = import "../observation-profile.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
let redaction = replace "visible_fields" ["external_reference", "unknown_field"] Source.profile.redaction in
replace "redaction" redaction Source.profile | std.contract.from_predicate (fun value => value == Source.profile)
