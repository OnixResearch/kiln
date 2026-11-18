# Rejected: two candidates share an identity, so their waits could not be told apart.
let contracts = import "../review-queue-wait-contracts.ncl" in
{
  schema = "kiln.review-queue-wait.load.v1",
  tick_ms = 1000,
  horizon_ticks = 8,
  max_entries = 4,
  modes = [{ mode = 'serial }],
  candidates = [
    { id = "alpha", arrival_tick = 0, ci = 'pass, ci_ticks = 1 },
    { id = "alpha", arrival_tick = 1, ci = 'fail, ci_ticks = 1 },
  ],
}
  | contracts.Load
  | contracts.Consistent
