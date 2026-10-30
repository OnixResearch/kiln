# r[impl kiln.tangled.profile]
# Apply this builder to reviewed operator inputs, then export the result as JSON
# for `kiln_adapter_tangled::profile::admit_profile`. No repository, knot,
# approver, executor or credential is invented here.
let c = import "tangled-contracts.ncl" in
# Request bounds for knot and actor calls.
let BodyBytes = 1048576 in
let HeaderBytes = 16384 in
let ReadMilliseconds = 3000 in
let WriteMilliseconds = 2000 in
# Tangled spindles refuse service-auth tokens living longer than five minutes;
# knots mint 60-second tokens.
let TokenLifetimeSeconds = 300 in
let ClockSkewSeconds = 60 in
let TokenRecords = 65536 in
let PipelineRecords = 4096 in
let RecordBytes = 1048576 in
# Tangled lexicons cap one pipeline at 50 workflows.
let WorkflowFiles = 50 in
let WorkflowBytes = 65536 in
let ChangedPaths = 8192 in
let ChangedPathBytes = 524288 in
let AcquisitionMilliseconds = 120000 in
let ResolverMilliseconds = 5000 in
let DidDocumentBytes = 65536 in
let KeyCacheSeconds = 300 in
let ActiveRuns = 4 in
let WorkflowPolls = 3600 in
let PollMilliseconds = 1000 in
let StatusMilliseconds = 10000 in
let StatusAttempts = 10 in
fun input =>
  let settings =
    input
      | {
        spindle_did | c.WebDid,
        listen_address | String,
        state_directory | c.AbsolutePath,
        git_program | c.AbsolutePath,
        plc_directory_url | c.HttpsUrl | default = "https://plc.directory",
        repositories | Array c.Repository,
        engines | Array c.Engine,
        workflow | c.Workflow,
        status_endpoint | c.HttpsUrl,
        status_secret_path | c.AbsolutePath,
      }
  in
  {
    schema = "kiln.tangled-spindle.v1",
    spindle_did = settings.spindle_did,
    listen_address = settings.listen_address,
    database_path = settings.state_directory ++ "/tangled.sqlite",
    source_directory = settings.state_directory ++ "/sources",
    git_program = settings.git_program,
    plc_directory_url = settings.plc_directory_url,
    repositories = settings.repositories,
    engines = settings.engines,
    workflow = settings.workflow,
    status = {
      endpoint = settings.status_endpoint,
      secret_path = settings.status_secret_path,
      timeout_ms = StatusMilliseconds,
      maximum_attempts = StatusAttempts,
    },
    limits = {
      maximum_body_bytes = BodyBytes,
      maximum_header_bytes = HeaderBytes,
      read_timeout_ms = ReadMilliseconds,
      write_timeout_ms = WriteMilliseconds,
      maximum_token_lifetime_seconds = TokenLifetimeSeconds,
      clock_skew_seconds = ClockSkewSeconds,
      maximum_tokens = TokenRecords,
      maximum_pipelines = PipelineRecords,
      maximum_record_bytes = RecordBytes,
      maximum_workflow_files = WorkflowFiles,
      maximum_workflow_bytes = WorkflowBytes,
      maximum_changed_paths = ChangedPaths,
      maximum_changed_path_bytes = ChangedPathBytes,
      acquisition_timeout_ms = AcquisitionMilliseconds,
      resolver_timeout_ms = ResolverMilliseconds,
      maximum_did_document_bytes = DidDocumentBytes,
      key_cache_seconds = KeyCacheSeconds,
      maximum_active_runs = ActiveRuns,
      maximum_workflow_polls = WorkflowPolls,
      worker_poll_interval_ms = PollMilliseconds,
    },
    non_claims = c.NonClaims,
  } | c.Profile
