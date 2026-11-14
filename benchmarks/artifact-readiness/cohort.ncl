let Contract = import "contract.ncl" in
{
  schema = "kiln.readiness.cohort.v1",
  samples = 3,
  workloads = [
    { id = "one-file", payload_bytes = 65536, files = 1, symlinks = 0, fanout = 0 },
    { id = "many-files", payload_bytes = 65536, files = 256, symlinks = 0, fanout = 0 },
    { id = "many-symlinks", payload_bytes = 65536, files = 1, symlinks = 256, fanout = 0 },
    { id = "shared-fanout", payload_bytes = 65536, files = 1, symlinks = 0, fanout = 4 },
    { id = "larger-bytes", payload_bytes = 262144, files = 1, symlinks = 0, fanout = 0 },
  ],
  decision = {
    proposal = "Reduce regular-file materialization overhead by changing archive layout; implementation and retention review required before go",
    minimum_samples = 30,
    minimum_p95_saving_fraction = 0.20,
    minimum_p95_saving_ms = 5,
    maximum_retained_byte_growth = 0,
  },
} | Contract.Cohort
