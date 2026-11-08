let positive = import "hosted-service-cloud-hypervisor-positive.ncl" in
let base = positive.execution in
(import "../hosted-service.ncl") (positive & { execution | force = base & { runners | force = base.runners & { cloud | force = base.runners.cloud & { memory_mib = 64 } } } })
