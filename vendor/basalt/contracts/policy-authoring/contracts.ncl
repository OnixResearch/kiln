let Prelude = import "../policy-prelude.ncl" in
let NormalizedId = Prelude.contracts.normalized_id in
let SupportedBackendId = Prelude.contracts.supported_backend_id in
let UniqueNonEmpty = Prelude.contracts.unique_non_empty "allowlist" in
let IsLexicalAbility = Prelude.predicates.lexical_ability in
let AbilityAllowlist = std.contract.Sequence [
  UniqueNonEmpty,
  std.contract.from_predicate (fun value =>
    std.array.all IsLexicalAbility value
  ),
] in
let ContractPolicy = {
  id | NormalizedId,
  description | String | default = "",
  resource_prefixes | UniqueNonEmpty,
  abilities | AbilityAllowlist,
} in
let BackendPolicy = {
  id | SupportedBackendId,
  description | String | default = "",
  supports_higher_order | Bool | default = false,
} in
let Policy = {
  schema_version | Prelude.contracts.exact_schema,
  backends | { _ : BackendPolicy } | default = {},
  contracts | { _ : ContractPolicy },
} in
let keys_aligned = fun record =>
  std.record.fields record
  |> std.array.all (fun key =>
    let entry = std.record.get key record in
    entry.id == key
  ) in
let whole_policy_invariants = fun policy =>
  std.record.length policy.contracts >= 1
  && keys_aligned policy.contracts
  && keys_aligned policy.backends in
let WholePolicy = std.contract.Sequence [Policy, std.contract.from_predicate whole_policy_invariants] in
{
  Policy = WholePolicy,
  ContractPolicy = ContractPolicy,
  BackendPolicy = BackendPolicy,
}
