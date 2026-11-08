let positive = import "hosted-service-positive.ncl" in
let base = positive.execution in
(import "../hosted-service.ncl") (positive & { execution | force = base & { runners | force = std.record.map (fun _ runner => runner) (base.runners & { "bad/name" | force = base.runners.local }) } })
