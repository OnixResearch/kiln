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
let UniqueRepositories =
  std.contract.from_predicate (fun value =>
    let check = fun state repository =>
      if std.array.elem repository.id state.ids
      || std.array.elem repository.repository_id state.native
      || std.array.elem repository.git_directory state.paths then
        {
          ids = state.ids,
          native = state.native,
          paths = state.paths,
          duplicate = true,
        }
      else
        {
          ids = state.ids @ [repository.id],
          native = state.native @ [repository.repository_id],
          paths = state.paths @ [repository.git_directory],
          duplicate = false,
        }
    in
    !(std.array.fold_left check { ids = [], native = [], paths = [], duplicate = false } value.repositories).duplicate
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
      | UniqueRepositories
  in
  {
    schema = "kiln.web-cobs.v1",
    repositories = admitted.repositories,
  }
