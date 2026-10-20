# Requirement ID registry contract.

let NonEmptyString = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let RegistrySchemaVersion = std.contract.from_predicate (fun value =>
  std.is_string value && value == "cairn.requirement-id-registry.v1"
) in
let RegistryStatus = std.contract.from_predicate (fun value =>
  std.is_string value && (value == "active" || value == "deprecated" || value == "retired")
) in
let RequirementId = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.is_match "^[A-Za-z0-9_.-]+$" value
) in
rec {
  Alias = {
    id | RequirementId,
    status | RegistryStatus,
    reason | NonEmptyString,
  },

  RegistryRow = {
    requirement_id | RequirementId,
    namespace | NonEmptyString,
    owner_repo | NonEmptyString,
    accepted_spec | NonEmptyString,
    status | RegistryStatus,
    aliases | Array Alias,
    allowed_consumers | Array NonEmptyString,
  },

  Reference = {
    requirement_id | RequirementId,
    consumer | NonEmptyString,
    expected_owner | String | default = "",
  },

  Registry = {
    schema_version | RegistrySchemaVersion,
    boundary | NonEmptyString,
    non_claims | Array NonEmptyString,
    rows | Array RegistryRow,
  },

  Input = {
    policy_hash | String | default = "",
    registry | Registry,
    references | Array Reference,
  },
}
