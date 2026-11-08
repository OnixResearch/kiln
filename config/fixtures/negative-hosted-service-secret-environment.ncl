let positive = import "hosted-service-positive.ncl" in
let base = positive.execution in
let workload = std.array.first base.workload in
let secret = std.array.first workload.secrets in
let updated = workload & { secrets | force = [secret & { environment | force = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" }] } in
(import "../hosted-service.ncl") (positive & { execution | force = base & { workload | force = [updated] } })
