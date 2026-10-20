let allowed = ["cohort"] in
let input = { cohort = "1.17.0", rogue = true } in
let unexpected = std.array.filter (fun key => !std.array.elem key allowed) (std.record.fields input) in
if std.array.length unexpected == 0 then "ok" else error "unknown-field-detected"
