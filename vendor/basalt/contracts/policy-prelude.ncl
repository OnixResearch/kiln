# Basalt Nickel policy authoring prelude.
#
# Shared contracts and deterministic issue classes for reviewed policy source.
# Nickel owns reviewed source shape and checked projection inputs. Rust and UCAN
# retain token verification, capability semantics, backend execution,
# enforcement, and receipt authority.
#
# This prelude intentionally stops before UCAN containment, attenuation, caveat,
# proof, replay, token, or authorization semantics.

let SchemaVersion = "ucan-nickel-contracts.policy.v1" in
let SupportedBackendIds = ["nickel", "steel"] in
let MaximumFieldBytes = 4096 in
let MaximumIdBytes = 128 in
let MaximumAllowlistBytes = 2048 in
let MaximumAllowlistItems = 256 in
let MaximumPathSegments = 16 in
let MaximumPathSegmentBytes = 256 in

let ExactSchema =
  std.contract.from_predicate (fun value =>
    std.is_string value && value == SchemaVersion
  )
in

let NormalizedId =
  std.contract.from_predicate (fun value =>
    std.is_string value
    && std.string.length value > 0
    && std.string.length value <= MaximumIdBytes
    && std.string.is_match "^[A-Za-z0-9_.-]+$" value
  )
in

let SupportedBackendId =
  std.contract.from_predicate (fun value =>
    std.is_string value && std.array.elem value SupportedBackendIds
  )
in

let non_empty_string = fun value =>
  std.is_string value
  && std.string.length value > 0 in
let is_lexical_value = fun value =>
  std.is_string value
  && std.string.length value > 0
  && std.string.length value <= MaximumFieldBytes
  && !std.string.contains " " value in
let LexicalValue =
  std.contract.from_predicate is_lexical_value
in

let is_lexical_ability = fun value =>
  if std.is_string value then
    let ok = is_lexical_value value in
    let starts_or_ends_with_slash =
      std.string.is_match "^/.*|.*/$" value in
    let segments = std.string.split "/" value in
    ok
    && !starts_or_ends_with_slash
    && (std.array.length segments >= 1)
    && (std.array.length segments <= MaximumPathSegments)
    && (std.array.all (fun segment => non_empty_string segment
      && std.string.length segment <= MaximumPathSegmentBytes) segments)
  else
    false in
let LexicalAbility =
  std.contract.from_predicate is_lexical_ability
in

# A contract that requires every map key to equal its embedded `id` field.
let KeyIdAligned = fun label => {
  id | NormalizedId,
} & std.contract.from_predicate (fun value =>
  std.record.has_field "id" value
  && value.id == label
) in

let appears_more_than_once = fun values => fun target =>
  std.array.length (std.array.filter (fun item => item == target) values) > 1 in
let has_no_duplicates = fun values =>
  !(std.array.any (fun item => appears_more_than_once values item) values) in

# Require a non-empty collection with no duplicate normalized entries.
let UniqueNonEmpty = fun label =>
  std.contract.from_predicate (fun value =>
    std.is_array value
    && std.array.length value > 0
    && std.array.length value <= MaximumAllowlistItems
    && std.array.all (fun item =>
      std.is_string item
      && std.string.length item > 0
      && std.string.length item <= MaximumAllowlistBytes
      && !std.string.contains " " item
    ) value
    && has_no_duplicates value
  )
in

{
  schema_version = SchemaVersion,
  supported_backend_ids = SupportedBackendIds,
  identity = {
    schema = SchemaVersion,
    supported_backend_ids = SupportedBackendIds,
  },
  contracts = {
    exact_schema = ExactSchema,
    normalized_id = NormalizedId,
    supported_backend_id = SupportedBackendId,
    lexical_value = LexicalValue,
    lexical_ability = LexicalAbility,
    key_id_aligned = KeyIdAligned,
    unique_non_empty = UniqueNonEmpty,
  },
  predicates = {
    lexical_value = is_lexical_value,
    lexical_ability = is_lexical_ability,
  },
  limits = {
    max_field_bytes = MaximumFieldBytes,
    max_id_bytes = MaximumIdBytes,
    max_allowlist_bytes = MaximumAllowlistBytes,
    max_allowlist_items = MaximumAllowlistItems,
    max_path_segments = MaximumPathSegments,
    max_path_segment_bytes = MaximumPathSegmentBytes,
  },
}
