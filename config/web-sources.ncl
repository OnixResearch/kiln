# Optional read-only local source policy; export JSON and pass --sources explicitly.
# Repository paths and executables are server-owned and never accepted from browser requests.
let c = import "github-contracts.ncl" in
let Text = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= maximum
    && std.string.is_match "^[ -~]+$" value
  )
in
let Kind =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.array.any (fun kind => kind == value) ["git", "jj", "pijul"]
  )
in
let Channel =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value <= 128
    && std.string.is_match "^[A-Za-z0-9_.-]+$" value
  )
in
let UniqueSources =
  std.contract.from_predicate (fun value =>
    let check = fun state source =>
      if std.array.elem source.id state.ids || std.array.elem source.path state.paths then
        { ids = state.ids, paths = state.paths, duplicate = true }
      else
        {
          ids = state.ids @ [source.id],
          paths = state.paths @ [source.path],
          duplicate = false,
        }
    in
    !(std.array.fold_left check { ids = [], paths = [], duplicate = false } value.sources).duplicate
  )
in
let Source = {
  id | c.Component,
  label | Text 128,
  kind | Kind,
  path | c.AbsolutePath,
  channel | Channel | optional,
  executable | c.AbsolutePath | optional,
}
in
fun input =>
  let admitted =
    input
      | {
        sources
          | Array Source
          | std.contract.from_predicate (fun rows =>
            std.array.length rows <= 64
            && std.array.all
              (fun source =>
                if source.kind == "pijul" then
                  std.record.has_field "channel" source
                else
                  !std.record.has_field "channel" source
                  && !std.record.has_field "executable" source
              )
              rows
          ),
      }
      | UniqueSources
  in
  {
    schema = "kiln.web-sources.v1",
    sources = admitted.sources,
  }
