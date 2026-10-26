let Admit = import "../dashboard-catalog.ncl" in
let Inputs = import "dashboard-catalog-inputs.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
Admit (replace "bounds" (replace "max_histories" 0 Inputs.bounds) Inputs)
