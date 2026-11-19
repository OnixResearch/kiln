let allowed = ["evaluator", "source_revision"] in
let input = {
  evaluator = "nickel-lang-cli-1.17.0",
  source_revision = "1320a983e6c3d1e2fb53dd2464b084b4903b1426",
  ambient = true,
} in
let unexpected = std.array.filter (fun key => !std.array.elem key allowed) (std.record.fields input) in
if std.array.length unexpected == 0 then input else error "unknown-field-detected"
