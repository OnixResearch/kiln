let Integer = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_number value && value >= 1 && value <= maximum && std.number.floor value == value
  )
in
let Between = fun minimum maximum =>
  std.contract.from_predicate (fun value =>
    std.is_number value
    && value >= minimum
    && value <= maximum
    && std.number.floor value == value
  )
in
let NonEmptyAtMost = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_array value && std.array.length value >= 1 && std.array.length value <= maximum
  )
in
let UniqueWorkerIds =
  std.contract.from_predicate (fun value =>
    let check = fun state worker =>
      if std.array.elem worker.worker_id state.seen then
        { seen = state.seen, duplicate = true }
      else
        { seen = state.seen @ [worker.worker_id], duplicate = false }
    in
    !(std.array.fold_left check { seen = [], duplicate = false } value.workers).duplicate
  )
in
let Component =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 128
    && std.string.is_match "^[a-zA-Z0-9_.-]+$" value
    && value != "."
    && value != ".."
  )
in
let Identity =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[0-9a-f]{64}$" value
  )
in
let Worker = {
  worker_id | Component,
  principal | std.contract.from_predicate (fun value => std.is_string value && std.string.length value >= 1 && std.string.length value <= 256 && std.string.is_match "^[!-~]+$" value),
  project_id | Component,
  target_id | Identity,
  maximum_slots | Integer 256,
}
in
fun input =>
  let settings =
    input
      | {
        maximum_ttl_ms | Between 300 120000 | default = 30000,
        maximum_clock_skew_ms
          | std.contract.from_predicate (fun value =>
            std.is_number value && value >= 0 && value <= 10000 && std.number.floor value == value
          )
          | default
          = 1000,
        workers | Array Worker | NonEmptyAtMost 256,
      }
      | UniqueWorkerIds
  in
  {
    schema = "kiln.worker-status.v1",
    maximum_ttl_ms = settings.maximum_ttl_ms,
    maximum_clock_skew_ms = settings.maximum_clock_skew_ms,
    workers = settings.workers,
  }
