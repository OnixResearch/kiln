# r[verify kiln.dashboard.live.collection.scenario.admission]
let Refresh = import "../dashboard-refresh.ncl" in
Refresh {
  schema_version = 1,
  candidate_root = "/var/lib/kiln/self-ci/candidates",
  catalog_output = "/var/lib/kiln/dashboard/catalog.json",
  publication_config = "/var/lib/kiln/dashboard/publication.json",
  project_id = "kiln-self-ci",
  project_label = "Kiln dedicated fork / observed updates",
  max_candidates = 128,
  max_histories = 16,
  max_journal_bytes = 1048576,
}
