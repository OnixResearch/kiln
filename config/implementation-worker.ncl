# Human-authored configuration for the explicit candidate-producing CLI route.
# Export with Nickel; dynamic attempt fields come from actual claim observations.
let Token =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.length value > 0
  )
in
let Digest =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.string.is_match "^[a-f0-9]{64}$" value
  )
in
let Positive = std.contract.from_predicate (fun value => std.is_number value && value > 0) in
{
  Transport = {
    schema | std.contract.from_predicate (fun value => value == "kiln.lattice-implementation-cli.v1"),
    lattice_program | Token,
    executable_blake3 | Digest,
    state_directory | Token,
    output_directory | Token,
  },
  Omp = {
    revision | std.contract.from_predicate (fun value => value == "e325ffc73ab86e03bdd20a91b77d8e2b3964c906"),
    executable_blake3 | Digest,
    base_identity | Digest,
    model | Token,
    prompt | Token,
  },
  Bounds = {
    expires_at_ms | Positive,
    output_limit_bytes | std.contract.from_predicate (fun value => std.is_number value && value > 0 && value <= 16777216),
  },
}
