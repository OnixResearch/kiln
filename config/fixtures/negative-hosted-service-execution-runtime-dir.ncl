let positive = import "hosted-service-positive.ncl" in
(import "../hosted-service.ncl") (positive & { execution | force = positive.execution & { manager_runtime_directory | force = "/run/user/abc" } })
