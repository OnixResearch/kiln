let CelldProfile = import "celld-profile.ncl" in
{
  name = "kiln-event-store",
  main = "src/index.ts",
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
    KILN_ORIGIN = CelldProfile.origin,
    KILN_TRUST_DOMAIN = CelldProfile.trust_domain,
  },
  observability = {
    enabled = false,
  },
}
