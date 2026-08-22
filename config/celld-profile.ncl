let Contracts = import "contracts.ncl" in
let MaxRequestBytes = 1048576 in
let MaxResponseBytes = 1048576 in
let MaxReplayPageBytes = 1048576 in
let MaxEventsPerAppend = 32 in
let MaxOutboxRecords = 64 in
let MaxEventBytes = 65536 in
let MaxPartitionCount = 256 in
let MaxLeaseTicks = 300 in
let MaxMigrationSteps = 16 in
let TimeoutMillis = 30000 in
({
  celld_release = "v0.2.1",
  celld_revision = "ae8fac053d79f971bfcb996054bb43eb2f9b05da",
  license = "Apache-2.0",
  worker_protocol_version = 1,
  schema_version = 1,
  origin = "https",
  path = "/v1/append",
  tls_policy = "https",
  auth_profile = "hmac-sha256",
  redirects_disabled = true,
  trust_domain = "single",
  output_gate_enabled = true,
  storage_probe_required = true,
  bounds = {
    max_request_bytes = MaxRequestBytes,
    max_response_bytes = MaxResponseBytes,
    max_replay_page_bytes = MaxReplayPageBytes,
    max_events_per_append = MaxEventsPerAppend,
    max_outbox_records = MaxOutboxRecords,
    max_event_bytes = MaxEventBytes,
    max_partition_count = MaxPartitionCount,
    max_lease_ticks = MaxLeaseTicks,
    max_migration_steps = MaxMigrationSteps,
    timeout_millis = TimeoutMillis,
  },
  non_claims = [
    "a celld acknowledgement does not prove external effect execution",
    "one-object transactions do not provide global serializability",
    "an alpha fleet admission does not prove hostile tenant isolation",
    "worker responses do not prove CI success or provider correctness",
    "storage probe results do not prove object-store durability",
  ],
}) | Contracts.CelldProfile
