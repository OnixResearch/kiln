# Explicit opt-in policy. The packaged default policy stays unchanged.
let Base = import "../policy.ncl" in
Base & {
  contracts | force = {
    process-execute = {
      id = "process-execute",
      description = "Execute one independently admitted complete process context.",
      resource_prefixes = ["onix://process-execution/"],
      abilities = ["process/execute"],
    },
  },
}
