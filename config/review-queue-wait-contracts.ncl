# Typed authoring contract for the off-by-default review-queue wait fixture
# (#61). Rust admission (`kiln_web_service::code_review::queue_wait::Load::admit`)
# is authoritative and rechecks every rule.
let MaxTickMs = 60000 in
let MaxHorizonTicks = 256 in
let MaxQueueEntries = 100 in
let MaxCandidates = 16 in
let MaxCiTicks = 16 in
let MaxWindow = 8 in
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let Integer = fun low high =>
  std.contract.from_predicate (fun value =>
    std.is_number value && value >= low && value <= high && std.number.floor value == value
  )
in
let Name =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[a-z][a-z0-9-]{0,47}$" value
  )
in
let distinct = fun values =>
  std.array.all (fun value => std.array.length (std.array.filter (fun other => other == value) values) == 1) values
in
let Window = {
  floor | Integer 1 MaxWindow,
  ceiling | Integer 1 MaxWindow,
  increase | Integer 1 MaxWindow,
  decrease | Integer 1 MaxWindow,
}
in
let Mode = {
  mode | [| 'serial, 'speculative |],
  window | Window | optional,
}
in
let Candidate = {
  id | Name,
  arrival_tick | Integer 0 MaxHorizonTicks,
  ci | [| 'pass, 'fail, 'unknown |],
  ci_ticks | Integer 1 MaxCiTicks,
  deadline_ticks | Integer 1 MaxHorizonTicks | optional,
}
in
{
  Load = {
    schema | Exact "kiln.review-queue-wait.load.v1",
    tick_ms | Integer 1 MaxTickMs,
    horizon_ticks | Integer 1 MaxHorizonTicks,
    max_entries | Integer 1 MaxQueueEntries,
    modes | Array Mode,
    candidates | Array Candidate,
  },
  # Rules that span fields: bounded unique candidates and modes, arrivals
  # inside the horizon, and a window exactly for speculative mode.
  Consistent =
    std.contract.from_predicate (fun load =>
      std.array.length load.candidates >= 1
      && std.array.length load.candidates <= MaxCandidates
      && std.array.length load.modes >= 1
      && distinct (std.array.map (fun candidate => candidate.id) load.candidates)
      && distinct (std.array.map (fun mode => mode.mode) load.modes)
      && std.array.all (fun candidate => candidate.arrival_tick <= load.horizon_ticks) load.candidates
      && std.array.all
        (fun mode =>
          if mode.mode == 'speculative then
            std.record.has_field "window" mode
            && mode.window.floor <= mode.window.ceiling
            && mode.window.ceiling <= load.max_entries
          else
            !(std.record.has_field "window" mode)
        )
        load.modes
    ),
  # The frozen load: passing, failing and unknown CI, one submitter deadline
  # and one arrival beyond the queue bound, run in both queue modes.
  example = {
    schema = "kiln.review-queue-wait.load.v1",
    tick_ms = 1000,
    horizon_ticks = 40,
    max_entries = 7,
    modes = [
      { mode = 'serial },
      { mode = 'speculative, window = { floor = 1, ceiling = 3, increase = 1, decrease = 1 } },
    ],
    candidates = [
      { id = "alpha", arrival_tick = 0, ci = 'pass, ci_ticks = 3 },
      { id = "bravo", arrival_tick = 0, ci = 'pass, ci_ticks = 3 },
      { id = "charlie", arrival_tick = 0, ci = 'fail, ci_ticks = 2 },
      { id = "delta", arrival_tick = 1, ci = 'pass, ci_ticks = 3, deadline_ticks = 8 },
      { id = "echo", arrival_tick = 1, ci = 'pass, ci_ticks = 3 },
      { id = "foxtrot", arrival_tick = 2, ci = 'unknown, ci_ticks = 2 },
      { id = "golf", arrival_tick = 2, ci = 'pass, ci_ticks = 2 },
      { id = "hotel", arrival_tick = 3, ci = 'pass, ci_ticks = 2 },
    ],
  },
}
