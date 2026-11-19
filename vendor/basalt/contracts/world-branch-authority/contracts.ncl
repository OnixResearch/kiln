let ProfileGeneration = 1 in
let RulePriority = 100 in
let MoltenRevision = "edc508229fb39d918473509c8f9e3c639c5d7275" in
let Modes = [
  "copyable",
  "attenuated",
  "linear",
  "simulation-only",
  "promotion-gated",
  "replace-before-activation",
  "non-branchable",
] in
let NonClaims = [
  "a policy decision does not mint or move a capability",
  "a policy receipt is not current authority",
  "token parsing signature verification revocation and replay remain consumer-owned",
  "derivation transfer adapter activation and effect enforcement remain Molten-owned",
  "linear policy does not prove source deactivation or destination activation",
  "simulation-only policy does not prove simulator parity or prevent host escape",
  "promotion-gated policy does not authorize effect dispatch",
  "branch-authority evidence does not prove release eligibility",
] in
let Rules = [
  { rule_id = "public-artifact-copy", capability_class = "public-artifact", mode = "copyable", priority = RulePriority, require_current = true, require_ucan = true, required_limits = [] },
  { rule_id = "scoped-service-attenuation", capability_class = "scoped-service", mode = "attenuated", priority = RulePriority, require_current = true, require_ucan = true, required_limits = ["narrower-resource-or-ability"] },
  { rule_id = "exclusive-lease-transfer", capability_class = "exclusive-lease", mode = "linear", priority = RulePriority, require_current = true, require_ucan = true, required_limits = ["generation-fenced-transfer", "source-deactivation"] },
  { rule_id = "external-effect-simulation", capability_class = "external-effect", mode = "simulation-only", priority = RulePriority, require_current = true, require_ucan = false, required_limits = ["exact-simulation-adapter"] },
  { rule_id = "deferred-effect-promotion", capability_class = "deferred-effect", mode = "promotion-gated", priority = RulePriority, require_current = true, require_ucan = true, required_limits = ["promotion-recheck", "release-reservation"] },
  { rule_id = "host-secret-replacement", capability_class = "host-secret", mode = "replace-before-activation", priority = RulePriority, require_current = true, require_ucan = false, required_limits = ["fresh-destination-grant"] },
  { rule_id = "bearer-credential-denial", capability_class = "bearer-credential", mode = "non-branchable", priority = RulePriority, require_current = true, require_ucan = false, required_limits = [] },
] in
let ConsumerMappingsData = {
  schema = "basalt.world-branch-authority-consumer-mappings.v1",
  molten = {
    revision = MoltenRevision,
    policy_owner = "basalt",
    enforcement_owner = "molten",
    retained_owners = [
      "token-parsing",
      "currentness",
      "derivation",
      "linear-transfer",
      "adapter-selection",
      "activation",
      "promotion",
      "effect-enforcement",
    ],
    mappings = std.array.map (fun rule => {
      source_class = rule.capability_class,
      mode = rule.mode,
      claim = "fixture-only",
    }) Rules,
  },
  non_claims = [
    "the mapping fixture creates no runtime dependency",
    "the mapping fixture does not prove native enforcement",
    "the mapping fixture contains no bearer capability material",
  ],
} in
let exact_fields = fun allowed value =>
  std.is_record value
  && std.array.length (std.record.fields value) == std.array.length allowed
  && std.array.all (fun field => std.array.elem field allowed) (std.record.fields value)
in
let field = fun name value => std.record.get name value in
let valid_rule = fun value =>
  exact_fields ["rule_id", "capability_class", "mode", "priority", "require_current", "require_ucan", "required_limits"] value
  && std.is_string (field "rule_id" value)
  && std.string.length (field "rule_id" value) > 0
  && std.is_string (field "capability_class" value)
  && std.string.length (field "capability_class" value) > 0
  && std.array.elem (field "mode" value) Modes
  && field "priority" value == RulePriority
  && std.is_bool (field "require_current" value)
  && std.is_bool (field "require_ucan" value)
  && std.is_array (field "required_limits" value)
in
let valid_policy = fun value =>
  exact_fields ["schema", "profile_id", "generation", "default_mode", "rules", "non_claims"] value
  && field "schema" value == "basalt.world-branch-authority-policy.v1"
  && field "profile_id" value == "basalt-world-branch-authority-v1"
  && field "generation" value == ProfileGeneration
  && field "default_mode" value == "non-branchable"
  && field "rules" value == Rules
  && std.array.all valid_rule (field "rules" value)
  && field "non_claims" value == NonClaims
in
let valid_mappings = fun value =>
  exact_fields ["schema", "molten", "non_claims"] value
  && value.schema == "basalt.world-branch-authority-consumer-mappings.v1"
  && value.molten.revision == MoltenRevision
  && value.molten.policy_owner == "basalt"
  && value.molten.enforcement_owner == "molten"
  && std.array.length value.molten.mappings == std.array.length Rules
  && std.array.all (fun mapping => std.array.elem mapping.mode Modes) value.molten.mappings
  && value.non_claims == ConsumerMappingsData.non_claims
in
{
  Policy = std.contract.from_predicate valid_policy,
  ConsumerMappings = std.contract.from_predicate valid_mappings,
  identity = {
    generation = ProfileGeneration,
    rule_priority = RulePriority,
    molten_revision = MoltenRevision,
  },
  modes = Modes,
  rules = Rules,
  non_claims = NonClaims,
  consumer_mappings = ConsumerMappingsData,
}
