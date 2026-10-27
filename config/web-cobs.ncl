# Optional read-only native COB policy; export JSON and pass --cobs explicitly.
# Repository paths are server-owned and never accepted from browser requests.
let c = import "github-contracts.ncl" in
let Text = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= maximum
    && std.string.is_match "^[ -~]+$" value
  )
in
let Repository = {
  id | c.Component,
  label | Text 128,
  repository_id | Text 256,
  git_directory | c.AbsolutePath,
}
in
fun input =>
  let admitted =
    input
      | {
        repositories
          | Array Repository
          | std.contract.from_predicate (fun rows => std.array.length rows <= 64),
      }
  in
  {
    schema = "kiln.web-cobs.v1",
    repositories = admitted.repositories,
  }
