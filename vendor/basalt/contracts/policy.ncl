let Prelude = import "policy-prelude.ncl" in
let NonEmptyString = Prelude.contracts.lexical_value in
let NormalizedId = Prelude.contracts.normalized_id in
let ExactSchema = Prelude.contracts.exact_schema in
let SupportedBackendId = Prelude.contracts.supported_backend_id in
let LexicalAbility = Prelude.contracts.lexical_ability in
let UniqueNonEmpty = Prelude.contracts.unique_non_empty "allowlist" in
let IsLexicalAbility = Prelude.predicates.lexical_ability in
let AbilityAllowlist = std.contract.Sequence [
  UniqueNonEmpty,
  std.contract.from_predicate (fun value =>
    std.array.all IsLexicalAbility value
  ),
] in
let KeyIdAligned = Prelude.contracts.key_id_aligned in
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
  schema_version | ExactSchema,
  backends | { _ : BackendPolicy } | default = {},
  contracts | { _ : ContractPolicy },
} in
# Whole-policy validation: at least one contract, map keys align with embedded
# ids, and every contract backend is declared.
let has_at_least_one_contract = fun policy =>
  std.record.length policy.contracts >= 1 in
let keys_aligned = fun record =>
  std.record.fields record
  |> std.array.all (fun key =>
    let entry = std.record.get key record in
    entry.id == key
  ) in
let bans_undefined_backend = fun policy =>
  true in
let whole_policy_invariants = fun policy =>
  has_at_least_one_contract policy
  && keys_aligned policy.contracts
  && keys_aligned policy.backends
  && bans_undefined_backend policy in
let WholePolicy = std.contract.Sequence [Policy, std.contract.from_predicate whole_policy_invariants] in
{
  schema_version = Prelude.schema_version,
  backends = {
    nickel = {
      id = "nickel",
      description = "Nickel-authored static policy contracts.",
      supports_higher_order = false,
    },
    steel = {
      id = "steel",
      description = "Steel Scheme dynamic predicate contracts.",
      supports_higher_order = true,
    },
  },
  contracts = {
    document-read = {
      id = "document-read",
      description = "Read bounded document resources.",
      resource_prefixes = ["did:key:z6Mkresource/docs/"],
      abilities = ["document/read"],
    },
    document-write = {
      id = "document-write",
      description = "Write bounded document resources.",
      resource_prefixes = ["did:key:z6Mkresource/docs/"],
      abilities = ["document/write"],
    },
    mantle-build-submit = { id = "mantle-build-submit", description = "Submit one bound Mantle build.", resource_prefixes = ["mantle://gateway/v1/"], abilities = ["mantle/build/submit"], },
    mantle-input-upload = { id = "mantle-input-upload", description = "Upload one admitted Mantle input.", resource_prefixes = ["mantle://gateway/v1/"], abilities = ["mantle/input/upload"], },
    mantle-store-read = { id = "mantle-store-read", description = "Read one bound Mantle store resource.", resource_prefixes = ["mantle://gateway/v1/"], abilities = ["mantle/store/read"], },
    mantle-project-declare = { id = "mantle-project-declare", description = "Admit one owner-bound original declaration and rewritten Mantle project goal.", resource_prefixes = ["mantle://gateway/v1/"], abilities = ["mantle/project/declare"], },
    mantle-status-read = { id = "mantle-status-read", description = "Read one bound Mantle attempt status.", resource_prefixes = ["mantle://gateway/v1/"], abilities = ["mantle/status/read"], },
    mantle-log-read = { id = "mantle-log-read", description = "Read one bound Mantle log window.", resource_prefixes = ["mantle://gateway/v1/"], abilities = ["mantle/log/read"], },
    mantle-event-read = { id = "mantle-event-read", description = "Read one bound Mantle event page.", resource_prefixes = ["mantle://gateway/v1/"], abilities = ["mantle/event/read"], },
    mantle-result-read = { id = "mantle-result-read", description = "Read one bound Mantle signed result.", resource_prefixes = ["mantle://gateway/v1/"], abilities = ["mantle/result/read"], },
    mantle-usage-read = { id = "mantle-usage-read", description = "Read bounded Mantle account usage.", resource_prefixes = ["mantle://gateway/v1/"], abilities = ["mantle/usage/read"], },
    mantle-owned-attempt-cancel = { id = "mantle-owned-attempt-cancel", description = "Cancel one owned fenced Mantle attempt.", resource_prefixes = ["mantle://gateway/v1/"], abilities = ["mantle/attempt/cancel-owned"], },
    mantle-publish = { id = "mantle-publish", description = "Publish one separately admitted Mantle result.", resource_prefixes = ["mantle://gateway/v1/"], abilities = ["mantle/publish"], },
    mantle-admin = { id = "mantle-admin", description = "Perform one bound Mantle administrative operation.", resource_prefixes = ["mantle://gateway/v1/"], abilities = ["mantle/admin"], },
    kiln-claim-acquire = { id = "kiln-claim-acquire", description = "Acquire one fenced Kiln implementation claim.", resource_prefixes = ["kiln://implementation/v1/", "kiln://implementation/v2/"], abilities = ["kiln/claim/acquire"], },
    kiln-claim-renew = { id = "kiln-claim-renew", description = "Renew one fenced Kiln implementation claim.", resource_prefixes = ["kiln://implementation/v1/", "kiln://implementation/v2/"], abilities = ["kiln/claim/renew"], },
    kiln-dispatch = { id = "kiln-dispatch", description = "Dispatch one bound Kiln implementation attempt.", resource_prefixes = ["kiln://implementation/v1/", "kiln://implementation/v2/"], abilities = ["kiln/dispatch"], },
    kiln-candidate-submit = { id = "kiln-candidate-submit", description = "Submit one sealed candidate for an exact Kiln attempt.", resource_prefixes = ["kiln://implementation/v1/", "kiln://implementation/v2/"], abilities = ["kiln/candidate/submit"], },
    kiln-verification-admit = { id = "kiln-verification-admit", description = "Admit verification evidence for one exact Kiln candidate.", resource_prefixes = ["kiln://implementation/v1/", "kiln://implementation/v2/"], abilities = ["kiln/verification/admit"], },
    kiln-attempt-cancel = { id = "kiln-attempt-cancel", description = "Cancel one owned fenced Kiln attempt.", resource_prefixes = ["kiln://implementation/v1/", "kiln://implementation/v2/"], abilities = ["kiln/attempt/cancel"], },
    kiln-publication-publish = { id = "kiln-publication-publish", description = "Publish one separately verified Kiln candidate.", resource_prefixes = ["kiln://implementation/v1/", "kiln://implementation/v2/"], abilities = ["kiln/publication/publish"], },
    kiln-integration-land = { id = "kiln-integration-land", description = "Integrate one exact Kiln candidate through the selected route.", resource_prefixes = ["kiln://implementation/v1/", "kiln://implementation/v2/"], abilities = ["kiln/integration/land"], },
    kiln-lifecycle-complete = { id = "kiln-lifecycle-complete", description = "Complete native lifecycle for one integrated Kiln change.", resource_prefixes = ["kiln://implementation/v1/", "kiln://implementation/v2/"], abilities = ["kiln/lifecycle/complete"], },
    kiln-source-discover = { id = "kiln-source-discover", description = "Discover entries inside one authorized Kiln source scope.", resource_prefixes = ["kiln://source/v1/"], abilities = ["kiln/source/discover"], },
    kiln-source-read = { id = "kiln-source-read", description = "Read the content of one exact Kiln source entry.", resource_prefixes = ["kiln://source/v1/"], abilities = ["kiln/source/read"], },
    kiln-source-history = { id = "kiln-source-history", description = "Read the history of one Kiln source entry or subtree.", resource_prefixes = ["kiln://source/v1/"], abilities = ["kiln/source/history"], },
    kiln-source-prepare = { id = "kiln-source-prepare", description = "Prepare one projected Kiln workspace.", resource_prefixes = ["kiln://source/v1/"], abilities = ["kiln/source/prepare"], },
    kiln-source-modify = { id = "kiln-source-modify", description = "Modify one exact existing Kiln source entry.", resource_prefixes = ["kiln://source/v1/"], abilities = ["kiln/source/modify"], },
    kiln-source-create = { id = "kiln-source-create", description = "Create one exact new Kiln source entry.", resource_prefixes = ["kiln://source/v1/"], abilities = ["kiln/source/create"], },
    kiln-source-delegate = { id = "kiln-source-delegate", description = "Share one Kiln source scope onward.", resource_prefixes = ["kiln://source/v1/"], abilities = ["kiln/source/delegate"], },
    kiln-source-result = { id = "kiln-source-result", description = "Retrieve one result produced for a Kiln source scope.", resource_prefixes = ["kiln://source/v1/"], abilities = ["kiln/source/result"], },
    onixos-target-candidate-activation = {
      id = "onixos-target-candidate-activation",
      description = "Activate one OnixOS candidate target.",
      resource_prefixes = ["did:key:z6Mkonix/onixos/"],
      abilities = ["onix/target-candidate-activation"],
    },
    onixos-target-generation-persistence = {
      id = "onixos-target-generation-persistence",
      description = "Persist one OnixOS generation.",
      resource_prefixes = ["did:key:z6Mkonix/onixos/"],
      abilities = ["onix/target-generation-persistence"],
    },
    onixos-target-rollback = {
      id = "onixos-target-rollback",
      description = "Roll back one OnixOS target.",
      resource_prefixes = ["did:key:z6Mkonix/onixos/"],
      abilities = ["onix/target-rollback"],
    },
    onixos-state-sync-publish = {
      id = "onixos-state-sync-publish",
      description = "Publish one OnixOS state-sync update.",
      resource_prefixes = ["onix://state-sync/"],
      abilities = ["state_sync.publish"],
    },
    onixos-state-sync-delete = {
      id = "onixos-state-sync-delete",
      description = "Delete one OnixOS state-sync update.",
      resource_prefixes = ["onix://state-sync/"],
      abilities = ["state_sync.delete"],
    },
    onixos-state-sync-admin = {
      id = "onixos-state-sync-admin",
      description = "Administer one OnixOS state-sync update.",
      resource_prefixes = ["onix://state-sync/"],
      abilities = ["state_sync.admin"],
    },
  },
} | WholePolicy
