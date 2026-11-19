let AgentOsRevision = "44a6c8022f26ef0edd56e3d224f8296ee6387232" in
let AgentOsVmConfigBlob = "e38172cfa97b21922fafcb822d254929bf1c191f" in
let AgentOsEvaluationBlob = "1ab663a8a2f8bab31218f39c49193bd63d31f82e" in
let AgentOsDefaultDenyBlob = "0fd804e703fe38d50efd353514ebea16f36452ac" in
let AgentOsLicenseBlob = "f23ef9d48af638ddf215b378916767349a9591a8" in
let AnimusRevision = "4f08f89d652a6e6c83b2a41fd9d3c7651daf5d68" in
let MoltenRevision = "4734913c1c230a35fc121b948cdf54ac0619dd2a" in
let ProfileGeneration = 1 in
let RulePriority = 100 in
let NonClaims = [
  "a permission allow does not execute an effect",
  "a permission receipt is not live authority",
  "native parsing and normalization remain consumer-owned",
  "filesystem policy does not replace Cap Root or capability-relative I/O",
  "network, process, environment, and binding implementations remain consumer-owned",
  "human approval cannot override a denial or missing authority",
  "consumer currentness and release decisions remain consumer-owned",
  "the agentOS reference is a bounded comparison, not parity or sandbox evidence",
] in
let Rules = [
  { rule_id = "filesystem-read-input", scope = "filesystem", operation = "read", match_mode = "exact", subject_pattern = "cap://workspace/input", priority = RulePriority, decision = "allow" },
  { rule_id = "filesystem-write-output", scope = "filesystem", operation = "write", match_mode = "prefix", subject_pattern = "cap://workspace/output/", priority = RulePriority, decision = "allow" },
  { rule_id = "network-connect-api", scope = "network", operation = "connect", match_mode = "exact", subject_pattern = "net://api.example.invalid:443", priority = RulePriority, decision = "allow" },
  { rule_id = "process-spawn-reviewed", scope = "process", operation = "spawn", match_mode = "exact", subject_pattern = "blake3:1111111111111111111111111111111111111111111111111111111111111111", priority = RulePriority, decision = "allow" },
  { rule_id = "environment-read-public", scope = "environment", operation = "read", match_mode = "exact", subject_pattern = "env://PUBLIC_SETTING", priority = RulePriority, decision = "allow" },
  { rule_id = "binding-invoke-molten", scope = "binding", operation = "invoke", match_mode = "prefix", subject_pattern = "binding://molten.fabric/", priority = RulePriority, decision = "allow" },
] in
let ConsumerMappingNonClaims = [
  "fixtures do not create consumer runtime dependencies",
  "fixtures do not prove consumer mapping completeness",
  "fixtures do not prove native enforcement or effect execution",
] in
let ConsumerMappingsData = {
  schema = "basalt.runtime-effect-consumer-mappings.v1",
  animus = {
    revision = AnimusRevision,
    source_paths = [
      "crates/animus-std/src/document.rs",
      "crates/animus-runtime/src/toolexecutionstages/core.rs",
    ],
    mappings = [
      { source = "document-read", scope = "filesystem", operation = "read", subject = "cap://workspace/input", claim = "fixture-only" },
      { source = "document-output", scope = "filesystem", operation = "write", subject = "cap://workspace/output/report", claim = "fixture-only" },
      { source = "extractor", scope = "process", operation = "spawn", subject = "blake3:1111111111111111111111111111111111111111111111111111111111111111", claim = "fixture-only" },
    ],
  },
  molten = {
    revision = MoltenRevision,
    source_paths = [
      "crates/molten-core/src/planning.rs",
      "crates/molten-core/src/system_extension/tests.rs",
    ],
    mappings = [
      { source = "transport-send", scope = "network", operation = "connect", subject = "net://api.example.invalid:443", claim = "fixture-only" },
      { source = "fabric-port", scope = "binding", operation = "invoke", subject = "binding://molten.fabric/status", claim = "fixture-only" },
    ],
  },
  non_claims = ConsumerMappingNonClaims,
} in
let exact_fields = fun allowed value =>
  std.is_record value
  && std.array.length (std.record.fields value) == std.array.length allowed
  && std.array.all (fun field => std.array.elem field allowed) (std.record.fields value)
in
let field = fun name value => std.record.get name value in
let valid_policy = fun value =>
  exact_fields ["schema", "profile_id", "generation", "default_decision", "require_contract", "require_verified_ucan", "rules", "reference", "non_claims"] value
  && field "schema" value == "basalt.runtime-effect-policy.v1"
  && field "profile_id" value == "basalt-runtime-effects-v1"
  && field "generation" value == ProfileGeneration
  && field "default_decision" value == "deny"
  && field "require_contract" value == true
  && field "require_verified_ucan" value == true
  && field "rules" value == Rules
  && field "reference" value == {
    repository = "https://github.com/rivet-dev/agentos",
    revision = AgentOsRevision,
    vm_config_blob = AgentOsVmConfigBlob,
    evaluation_blob = AgentOsEvaluationBlob,
    default_deny_blob = AgentOsDefaultDenyBlob,
    license_blob = AgentOsLicenseBlob,
    license = "Apache-2.0",
    claim = "bounded-comparison-only",
  }
  && field "non_claims" value == NonClaims
in
let NonEmptyString = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let OneOf = fun choices => std.contract.from_predicate (fun value =>
  std.is_string value && std.array.elem value choices
) in
let Mapping = {
  source | NonEmptyString,
  scope | OneOf ["filesystem", "network", "process", "environment", "binding"],
  operation | NonEmptyString,
  subject | NonEmptyString,
  claim | OneOf ["fixture-only"],
} in
let Consumer = {
  revision | NonEmptyString,
  source_paths | Array NonEmptyString,
  mappings | Array Mapping,
} in
let mapping_invariants = fun value =>
  value.schema == "basalt.runtime-effect-consumer-mappings.v1"
  && value.animus.revision == AnimusRevision
  && std.array.length value.animus.source_paths > 0
  && std.array.length value.animus.mappings > 0
  && value.molten.revision == MoltenRevision
  && std.array.length value.molten.source_paths > 0
  && std.array.length value.molten.mappings > 0
  && value.non_claims == ConsumerMappingNonClaims
in
let ConsumerMappingsContract = std.contract.Sequence [{
  schema | NonEmptyString,
  animus | Consumer,
  molten | Consumer,
  non_claims | Array String,
}, std.contract.from_predicate mapping_invariants] in
{
  Policy = std.contract.from_predicate valid_policy,
  ConsumerMappings = ConsumerMappingsContract,
  identity = {
    agentos_revision = AgentOsRevision,
    agentos_vm_config_blob = AgentOsVmConfigBlob,
    agentos_evaluation_blob = AgentOsEvaluationBlob,
    agentos_default_deny_blob = AgentOsDefaultDenyBlob,
    agentos_license_blob = AgentOsLicenseBlob,
    animus_revision = AnimusRevision,
    molten_revision = MoltenRevision,
    generation = ProfileGeneration,
    rule_priority = RulePriority,
  },
  rules = Rules,
  consumer_mappings = ConsumerMappingsData,
  non_claims = NonClaims,
}
