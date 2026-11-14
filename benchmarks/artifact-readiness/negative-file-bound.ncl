let Contract = import "contract.ncl" in
std.record.update "workloads"
  [{ id = "invalid", payload_bytes = 65536, files = 0, symlinks = 0, fanout = 0 }]
  (import "cohort.ncl") | Contract.Cohort
