let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let Integer = fun low high => std.contract.from_predicate (fun x =>
  std.is_number x && x >= low && x <= high && std.number.floor x == x) in
let Name = std.contract.from_predicate (fun x => std.is_string x && std.string.is_match "^[a-z][a-z0-9-]{0,47}$" x) in
let Workload = {
  id | Name,
  payload_bytes | Integer 1 262144,
  files | Integer 1 512,
  symlinks | Integer 0 512,
  fanout | Integer 0 8,
} in
{
  Cohort = {
    schema | Exact "kiln.readiness.cohort.v1",
    samples | Integer 1 100,
    workloads | Array Workload | std.contract.from_predicate (fun values => std.array.length values >= 1 && std.array.length values <= 8),
    decision | {
      proposal | String,
      minimum_samples | Integer 30 100,
      minimum_p95_saving_fraction | Exact 0.20,
      minimum_p95_saving_ms | Exact 5,
      maximum_retained_byte_growth | Exact 0,
    },
  },
}
