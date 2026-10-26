let Admit = import "../dashboard-catalog.ncl" in
let Inputs = import "dashboard-catalog-inputs.ncl" in
let replace = fun key value record => std.record.insert key value (std.record.remove key record) in
let project = std.array.at 0 Inputs.projects in
let history = std.array.at 0 project.histories in
Admit (replace "projects" [replace "histories" [history, history] project] Inputs)
