# Optional signed local work COB policy. No default store, identity or signing key.
let c = import "github-contracts.ncl" in
let Principal =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= 128
    && std.string.is_match "^[A-Za-z0-9_.:@/-]+$" value
  )
in
fun input =>
  let admitted =
    input
      | {
        git_directory | c.AbsolutePath,
        signer_seed_file | c.AbsolutePath,
        write_principals
          | Array Principal
          | std.contract.from_predicate (fun rows => std.array.length rows > 0 && std.array.length rows <= 128),
      }
  in
  {
    schema = "kiln.web-work.v1",
    git_directory = admitted.git_directory,
    signer_seed_file = admitted.signer_seed_file,
    write_principals = admitted.write_principals,
  }
