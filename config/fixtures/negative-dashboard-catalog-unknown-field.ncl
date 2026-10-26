let Admit = import "../dashboard-catalog.ncl" in
let Inputs = import "dashboard-catalog-inputs.ncl" in
Admit (std.record.insert "unexpected_field" true Inputs)
