let positive = import "hosted-service-positive.ncl" in
let execution = positive.execution in
let workload = execution.workload [0] in
let long = std.array.fold_left (fun acc _ => acc ++ "a") "" (std.array.generate (fun _ => 0) 257) in
(import "../hosted-service.ncl") (positive & { execution | force = execution & { workload | force = [workload & { audience | force = long }] } })
