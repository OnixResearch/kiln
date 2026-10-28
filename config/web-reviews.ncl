# Optional read-only retained Patchgate receipt policy; export JSON and pass
# --reviews explicitly. The directory is server-owned.
let c = import "github-contracts.ncl" in
let Text = fun maximum =>
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= maximum
    && std.string.is_match "^[ -~]+$" value
  )
in
fun input =>
  let admitted =
    input
      | {
        label | Text 128,
        directory | c.AbsolutePath,
      }
  in
  {
    schema = "kiln.web-reviews.v1",
    label = admitted.label,
    directory = admitted.directory,
  }
