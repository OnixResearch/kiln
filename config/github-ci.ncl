# r[impl kiln.github.profile] r[impl kiln.github.nonclaims]
# Apply this builder to reviewed operator inputs, then export the result as JSON.
# No deployed repository, executable identity, workflow or credentials are invented.
let c = import "github-contracts.ncl" in
let RunRecords = 4096 in
let DeliveryRecords = 16384 in
let RecordBytes = 1048576 in
let BodyBytes = 1048576 in
let HeaderBytes = 16384 in
let ReadMilliseconds = 3000 in
let WriteMilliseconds = 2000 in
# Concurrent queued plus active HTTP sockets; runtime admission caps this at 64.
let Connections = 16 in
let PollMilliseconds = 1000 in
let WorkflowPolls = 3600 in
let ActiveRuns = 4 in
let StatusResponseBytes = 1048576 in
let StatusPages = 10 in
let StatusMilliseconds = 10000 in
fun input =>
let settings = input | {
  owner | c.Owner,
  repository | c.Component,
  repository_id | c.Positive,
  replay_database_path | c.AbsolutePath,
  webhook_secret | c.SecretReference,
  status_secret | c.SecretReference,
  workflow | c.Workflow,
} in
{
  schema = "kiln.github-ci.v1",
  event_policy = {
    owner = settings.owner,
    repository = settings.repository,
    repository_id = settings.repository_id,
    allow_fork_pull_requests = false,
  },
  listen_address = "127.0.0.1:8787",
  webhook_path = "/github/webhook",
  replay_database_path = settings.replay_database_path,
  maximum_run_records = RunRecords,
  maximum_delivery_records = DeliveryRecords,
  maximum_record_bytes = RecordBytes,
  maximum_body_bytes = BodyBytes,
  maximum_header_bytes = HeaderBytes,
  read_timeout_ms = ReadMilliseconds,
  write_timeout_ms = WriteMilliseconds,
  maximum_connections = Connections,
  worker_poll_interval_ms = PollMilliseconds,
  maximum_workflow_polls = WorkflowPolls,
  maximum_active_runs = ActiveRuns,
  webhook_secret = settings.webhook_secret,
  status_secret = settings.status_secret,
  status = {
    owner = settings.owner,
    repository = settings.repository,
    context = "kiln/ci",
    maximum_response_bytes = StatusResponseBytes,
    maximum_pages = StatusPages,
    timeout_ms = StatusMilliseconds,
  },
  workflow = settings.workflow,
  non_claims = c.NonClaims,
} | c.ServiceProfile
