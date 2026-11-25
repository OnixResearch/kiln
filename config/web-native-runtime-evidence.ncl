# Optional exact Lattice inspection of retained campaign requests.
let c = import "github-contracts.ncl" in
fun input =>
  let admitted = input | {
    implementation_database | c.AbsolutePath,
    bound_input_directory | c.AbsolutePath,
    transport_profile | c.AbsolutePath,
  } in
  admitted & { schema = "kiln.native-runtime-evidence-profile.v1" }
