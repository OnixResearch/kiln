let CelldProfile = import "celld-profile.ncl" in
{
  name = "kiln-event-store",
  main = "src/index.mjs",
  compatibility_date = "2026-01-01",
  compatibility_flags = ["nodejs_compat"],
  durable_objects = {
    bindings = [
      {
        name = "KILN_JOURNAL",
        class_name = "KilnJournal",
      },
    ],
  },
  migrations = [
    {
      tag = "v1",
      new_sqlite_classes = ["KilnJournal"],
    },
  ],
  vars = {
    KILN_CELLD_REVISION = CelldProfile.celld_revision,
    KILN_WORKER_PROTOCOL_VERSION = std.string.from_number CelldProfile.worker_protocol_version,
    KILN_SCHEMA_VERSION = std.string.from_number CelldProfile.schema_version,
    KILN_PATH = CelldProfile.path,
    KILN_READ_PATH = "/v1/read",
    KILN_RECONCILE_PATH = "/v1/reconcile",
    KILN_LEASE_PATH = "/v1/lease",
    KILN_MIGRATE_PATH = "/v1/migrate",
    KILN_MAX_RESPONSE_BYTES = std.string.from_number CelldProfile.bounds.max_response_bytes,
    KILN_ORIGIN = CelldProfile.origin,
    KILN_MAX_REQUEST_BYTES = std.string.from_number CelldProfile.bounds.max_request_bytes,
    KILN_TRUST_DOMAIN = CelldProfile.trust_domain,
  },
}
