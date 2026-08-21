# r[verify kiln.radicle.mutation_conformance]
let Source = import "../radicle-mutation-profile.ncl" in
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let Request = {
  repository_rid | Exact "rad:zMutationFixture",
  object_type | Exact "xyz.radicle.patch",
  object_id | Exact "2222222222222222222222222222222222222222",
  payload_identity | Exact "ab3758f296630f83d4e381c41d023612b2106dce68845786b3959fdb963b1a73",
  action | Exact "review",
} in
{
  profile = Source.profile,
  request | Request = {
    repository_rid = "rad:zMutationFixture",
    object_type = "xyz.radicle.patch",
    object_id = "2222222222222222222222222222222222222222",
    payload_identity = "ab3758f296630f83d4e381c41d023612b2106dce68845786b3959fdb963b1a73",
    action = "review",
  },
}