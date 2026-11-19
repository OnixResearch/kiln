let Identity = {
  bounded_http_rid = "rad:z4ARxEt93T6gJTrTZpUyKHMAemPiv",
  bounded_http_url = "https://seed.radicle.garden/z4ARxEt93T6gJTrTZpUyKHMAemPiv.git",
  bounded_http_revision = "5abc2b93e0f288aba251a0750ce2d716eb9757fc",
  ring_version = "0.17.14",
  ring_checksum = "a4689e6c2294d81e88dc6261c768b63bc4fcdb852be6d1352498b114f61383b7",
} in
let NonClaims = [
  "source identity does not prove cryptographic correctness",
  "source identity does not prove remote availability",
  "dependency review does not transfer issuer or authorization authority",
] in
let exact_fields = fun allowed value =>
  std.is_record value
  && std.array.length (std.record.fields value) == std.array.length allowed
  && std.array.all (fun field => std.array.elem field allowed) (std.record.fields value)
in
let valid = fun value =>
  exact_fields ["schema", "bounded_http", "rs256_verifier", "non_claims"] value
  && value.schema == "basalt.oidc-exchange.sources.v1"
  && exact_fields ["rid", "url", "revision"] value.bounded_http
  && value.bounded_http.rid == Identity.bounded_http_rid
  && value.bounded_http.url == Identity.bounded_http_url
  && value.bounded_http.revision == Identity.bounded_http_revision
  && exact_fields ["version", "crate_checksum", "algorithm"] value.rs256_verifier
  && value.rs256_verifier.version == Identity.ring_version
  && value.rs256_verifier.crate_checksum == Identity.ring_checksum
  && value.rs256_verifier.algorithm == "RS256"
  && value.non_claims == NonClaims
in
{
  Source = std.contract.from_predicate valid,
  identity = Identity,
  non_claims = NonClaims,
}
