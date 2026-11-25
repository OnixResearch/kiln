# Optional owner-only web recovery. Nested native contracts are admitted by Rust.
let c = import "github-contracts.ncl" in
fun input =>
  let admitted = input | {
    principal | c.NonEmpty,
    local_operator | c.NonEmpty,
    database | c.AbsolutePath,
    enrollment | { repositories | Array String, actors | Array String },
    request | c.AbsolutePath,
    profile | { .. },
    transport_profile | { .. },
    socket_path | c.AbsolutePath,
    maximum_records | c.Positive,
  } in
  admitted & { schema = "kiln.dispatch-recovery-profile.v1" }
