let Source = import "../mantle-foreign-realize-profile.ncl" in
let bindings = std.array.map (fun binding => std.record.insert "product_roots" { out = "/tmp/outside" } (std.record.remove "product_roots" binding)) Source.profile.bindings in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
replace "bindings" bindings Source.profile | std.contract.from_predicate (fun value => value == Source.profile)
