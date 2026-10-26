# r[verify kiln.dashboard.repositories.delivery]
let Publication = import "../dashboard-publication.ncl" in
Publication {
  schema_version = 1,
  dashboard_executable = "../bin/kiln-dashboard",
  catalog = "catalog.json",
  profile = "dashboard-profile.json",
  local_report = "last-good.html",
  destination = { kind = "local", path = "/tmp/kiln-publication-fixture/index.html" },
  max_report_bytes = 1048576,
  subprocess_timeout_seconds = 60,
  interval_ms = 1000,
}
