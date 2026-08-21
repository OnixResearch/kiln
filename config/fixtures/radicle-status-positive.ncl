# r[verify kiln.radicle.status_conformance]
let Source = import "../radicle-status-profile.ncl" in
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let NonEmpty = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let Plan = {
  repository_rid | Exact "rad:zFixtureRepository",
  object_type | Exact (Source.profile.allowed_object_type),
  expected_head | Exact null,
  run_id | NonEmpty,
  idempotency_key | NonEmpty,
  outcome | std.contract.from_predicate (fun value => value == "success" || value == "failure"),
  max_payload_bytes | std.contract.from_predicate (fun value =>
    std.is_number value && value > 0 && value <= Source.profile.limits.max_payload_bytes
  ),
} in
{
  profile = Source.profile,
  plan | Plan = {
    repository_rid = "rad:zFixtureRepository",
    object_type = Source.profile.allowed_object_type,
    expected_head = null,
    run_id = "run-fixture",
    idempotency_key = "status-fixture",
    outcome = "success",
    max_payload_bytes = Source.profile.limits.max_payload_bytes,
  },
}