# r[impl kiln.github.profile] r[impl kiln.github.source]
# Export once; --identity pins BLAKE3 of those exact JSON bytes, including whitespace.
let c = import "github-contracts.ncl" in
let Entries = 65536 in
let PathBytes = 4096 in
let FileBytes = 16777216 in
let TotalBytes = 268435456 in
let TransferBytes = 268435456 in
let AcquisitionMilliseconds = 120000 in
let CommandMilliseconds = 900000 in
let OutputBytes = 1048576 in
let TeardownMilliseconds = 2000 in
fun input =>
let settings = input | {
  owner | c.Owner,
  repository | c.Component,
  program_path | c.AbsolutePath,
  args | Array String,
  environment | { _ : String },
  source_secret | c.Nullable c.SecretReference,
} in
{
  schema_version = 1,
  owner = settings.owner,
  repository = settings.repository,
  program_path = settings.program_path,
  args = settings.args,
  environment = settings.environment,
  source_secret = settings.source_secret,
  snapshot_limits = {
    max_entries = Entries,
    max_path_bytes = PathBytes,
    max_file_bytes = FileBytes,
    max_total_bytes = TotalBytes,
  },
  maximum_transfer_bytes = TransferBytes,
  acquisition_timeout_ms = AcquisitionMilliseconds,
  command_timeout_ms = CommandMilliseconds,
  maximum_output_bytes = OutputBytes,
  teardown_grace_ms = TeardownMilliseconds,
} | c.JobProfile
