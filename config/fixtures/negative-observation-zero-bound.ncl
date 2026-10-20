let Source = import "../observation-profile.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
let bounds = replace "max_page_items" 0 Source.profile.bounds in
replace "bounds" bounds Source.profile | std.contract.from_predicate (fun value => value == Source.profile)
