let config = import "../default.ncl" in
config & { task_operations.capacity.affects_tenant_fairness = true }
