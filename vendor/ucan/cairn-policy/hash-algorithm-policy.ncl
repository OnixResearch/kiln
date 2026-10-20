# Contract shape for hash algorithm policy manifests consumed by
# `cairn hash-policy verify --input <manifest.json>`.

let NonEmptyString = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in

let NonEmptyArray = std.contract.from_predicate (fun value =>
  std.is_array value && std.array.length value > 0
) in

let Algorithm = std.contract.from_predicate (fun value =>
  std.is_string value && (value == "BLAKE3" || value == "SHA256" || value == "SHA-256" || value == "sha256")
) in

{
  Exception = {
    algorithm | Algorithm,
    owner | NonEmptyString,
    scope | NonEmptyString,
    reason | NonEmptyString,
    allowed_consumers | NonEmptyArray | Array NonEmptyString,
    migration_status | NonEmptyString,
    non_claims | NonEmptyArray | Array NonEmptyString,
  },

  Policy = {
    default_algorithm | Algorithm,
    exceptions | Array Exception | default = [],
    non_claims | NonEmptyArray | Array NonEmptyString,
  },

  Field = {
    schema | NonEmptyString,
    field_path | NonEmptyString,
    algorithm | Algorithm,
    digest | String | default = "",
    stack_owned_identity | Bool,
  },

  Manifest = {
    policy | Policy,
    fields | Array Field,
  },
}
