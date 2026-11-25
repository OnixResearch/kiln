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
      duration_millis = 600000,
      monetary_microunits = 0,
      rounds = 1,
      tokens = 0,
    },
    per_round_upper_bound = {
      agent_invocations = 0,
      duration_millis = 600000,
      monetary_microunits = 0,
      rounds = 1,
      tokens = 0,
    },
  },
  completion = "run_succeeded",
  id = "security-audit",
  objective = "Execute read-only Cairn, cargo-audit, and Octet checks and retain a Provenance summary receipt.",
  redaction = {
    retain_safe_summaries = true,
  },
  schema_version = 1,
  triggers = [],
  workflow = {
    content_hash = "2cc6f8223cf41a03ea0affa8b2b141d9f03814371d73e2242e2c8ba8f1ef1842",
    workflow_name = "security-audit",
  },
}
) | OrchestrationDefinition
