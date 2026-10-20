let Source = import "../http-observation-profile.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
let content = replace "view_content_type" "text/plain" Source.profile.content in
replace "content" content Source.profile | std.contract.from_predicate (fun value => value == Source.profile)
