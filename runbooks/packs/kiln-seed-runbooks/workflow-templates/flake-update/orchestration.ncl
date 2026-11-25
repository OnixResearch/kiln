let WorkflowRevision = { workflow_name | String, content_hash | String } in
let RetryPolicy = { max_observed_failure_retries | Number } in
let BudgetDimensions = { rounds | Bool, agent_invocations | Bool, tokens | Bool, monetary_microunits | Bool, duration_millis | Bool } in
let BudgetVector = { rounds | Number, agent_invocations | Number, tokens | Number, monetary_microunits | Number, duration_millis | Number } in
let BudgetPolicy = { enforced | BudgetDimensions, limits | BudgetVector, per_round_upper_bound | BudgetVector } in
let AuthorityPolicy = { approvers | Array String } in
let TriggerBinding = { trigger_id | String, kind | String } in
let RedactionPolicy = { retain_safe_summaries | Bool } in
let OrchestrationDefinition = {
  schema_version | Number,
  id | String,
  objective | String,
  workflow | WorkflowRevision,
  completion | String,
  retry | optional | RetryPolicy,
  budgets | BudgetPolicy,
  authority | AuthorityPolicy,
  triggers | Array TriggerBinding,
  redaction | RedactionPolicy,
}
in
({
  authority = {
    approvers = ["operator:local"],
  },
  budgets = {
    enforced = {
      agent_invocations = false,
      duration_millis = true,
      monetary_microunits = false,
      rounds = true,
      tokens = false,
    },
    limits = {
      agent_invocations = 0,
      duration_millis = 1800000,
      monetary_microunits = 0,
      rounds = 1,
      tokens = 0,
    },
    per_round_upper_bound = {
      agent_invocations = 0,
      duration_millis = 1800000,
      monetary_microunits = 0,
      rounds = 1,
      tokens = 0,
    },
  },
  completion = "await_proposal",
  id = "flake-update",
  objective = "Update flake inputs, verify local builds, and await operator approval before Kiln publisher opens a PR.",
  redaction = {
    retain_safe_summaries = true,
  },
  schema_version = 1,
  triggers = [],
  workflow = {
    content_hash = "5bcd6a32798819bd3186f408d76932ef4f79ed6fbb29e0d485f1c1ea93791401",
    workflow_name = "flake-update",
  },
}
) | OrchestrationDefinition
