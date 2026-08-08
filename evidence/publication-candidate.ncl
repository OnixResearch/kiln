let Hash = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value == 64
) in
let Commit = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value == 40
) in
{
  schema = "kiln.radicle-publication-candidate.v1",
  source_revision | Commit = "36984e367eafd44c22d2b4f4f47121deb51eece3",
  source_change = "oyzuupzrwuysrvtonpurwqykoozkzrpq",
  archive = {
    format = "git-archive-tar",
    blake3 | Hash = "ce60d4c323a3966eecd7a93fda32c55c0442c434a2216eaa9fc002fd12ff28f2",
  },
  license = "MIT OR Apache-2.0",
  lifecycle = {
    state = "local-validation-complete-publication-not-authorized",
    task_gate_receipts | Array Hash = [
      "46139d1e8e3948742f11cb4c998b3ed67fb92f7b2e517d353d5e6397b2e51f5a",
      "19e0fac3cca1acb6ed9923e168c4f69dc3deb91bcad005541b128c70b002c4de",
      "df9014d3d9cd7db7517e056e681b2dd4aa20df64bcb4544b3ecd168ded687c4f",
    ],
  },
  rollback_revision | Commit = "b16f8792849cd3e2f2802c9a5063d5567a68f389",
  hosting_profile_blake3 | Hash = "1f650f733802d74f9064da668a337948a86f82e2f9340f2d68e6c670fbf15de0",
  authority = {
    radicle_initialized = false,
    signed = false,
    pushed = false,
    announced = false,
    mirrors_mutated = false,
  },
  non_claims = [
    "the candidate is a local source archive and is not a Radicle publication",
    "the unassigned RID prevents published-state admission",
    "remote mutation requires a separate explicit operator authorization",
  ],
}
