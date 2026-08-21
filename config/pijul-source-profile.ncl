# r[impl kiln.source.pijul_configuration]
# r[impl kiln.source.pijul_capabilities]
# r[impl kiln.source.pijul_nonclaims]
let Exact = fun expected => std.contract.from_predicate (fun value => value == expected) in
let Positive = std.contract.from_predicate (fun value =>
  std.is_number value && value > 0
) in
let Denied = Exact false in
let ProfileId = "pijul-source-v1" in
let PijulVersion = "1.0.0-beta.11" in
let LibpijulVersion = "1.0.0-beta.11" in
let RepositoryAdapterVersion = "1.0.0-beta.10" in
let PijulSourceRevision = "crates.io:sha256:8a4fc27aa81ee061310d57fce2df9cc45f3149ddb00bdfab2b816beb0359b13d" in
let LibpijulSourceRevision = "crates.io:sha256:f625e90234ef1fd164dfe7980ffb04a3036a08e31e051cc29e6273a0c9a7a7e7" in
let RepositoryAdapterSourceRevision = "crates.io:sha256:12b9f22e42a5149909b2d1d20706a5ee5042a1ec60330d94124a213e4e294aa5" in
let PijulLicense = "GPL-2.0-or-later" in
let LibpijulLicense = "GPL-2.0-or-later" in
let RepositoryAdapterLicense = "GPL-2.0" in
let FormatCohort = "sanakirja-pristine-v1" in
let Platforms = ["x86_64-linux"] in
let Capabilities = ["discover", "resolve_exact", "acquire", "observe_identity"] in
let EntryKinds = ["regular", "executable", "directory"] in
let NonClaims = [
  "local Pijul observation does not prove source trust",
  "local Pijul observation does not prove signature validity",
  "local Pijul observation does not prove complete history",
  "local Pijul observation does not prove remote availability",
  "local Pijul observation does not grant collaboration authority",
  "local Pijul observation does not grant source mutation authority",
  "equal canonical trees do not prove equal Pijul history",
] in
let MaxChannels = 64 in
let MaxRepositoryMembers = 65536 in
let MaxRepositoryBytes = 536870912 in
let MaxChanges = 4096 in
let MaxEntries = 16384 in
let MaxPathBytes = 4096 in
let MaxFileBytes = 16777216 in
let MaxTotalBytes = 268435456 in
let Profile = {
  id | Exact ProfileId,
  pijul_version | Exact PijulVersion,
  libpijul_version | Exact LibpijulVersion,
  repository_adapter_version | Exact RepositoryAdapterVersion,
  pijul_source_revision | Exact PijulSourceRevision,
  libpijul_source_revision | Exact LibpijulSourceRevision,
  repository_adapter_source_revision | Exact RepositoryAdapterSourceRevision,
  pijul_license | Exact PijulLicense,
  libpijul_license | Exact LibpijulLicense,
  repository_adapter_license | Exact RepositoryAdapterLicense,
  format_cohort | Exact FormatCohort,
  platforms | Exact Platforms,
  capabilities | Exact Capabilities,
  allowed_entry_kinds | Exact EntryKinds,
  max_channels | Positive,
  max_repository_members | Positive,
  max_repository_bytes | Positive,
  limits | {
    max_changes | Positive,
    max_entries | Positive,
    max_path_bytes | Positive,
    max_file_bytes | Positive,
    max_total_bytes | Positive,
  },
  allow_network | Denied,
  allow_credentials | Denied,
  allow_collaboration | Denied,
  allow_triggers | Denied,
  allow_status_publication | Denied,
  allow_mutation | Denied,
  non_claims | Exact NonClaims,
} in
{
  profile | Profile = {
    id = ProfileId,
    pijul_version = PijulVersion,
    libpijul_version = LibpijulVersion,
    repository_adapter_version = RepositoryAdapterVersion,
    pijul_source_revision = PijulSourceRevision,
    libpijul_source_revision = LibpijulSourceRevision,
    repository_adapter_source_revision = RepositoryAdapterSourceRevision,
    pijul_license = PijulLicense,
    libpijul_license = LibpijulLicense,
    repository_adapter_license = RepositoryAdapterLicense,
    format_cohort = FormatCohort,
    platforms = Platforms,
    capabilities = Capabilities,
    allowed_entry_kinds = EntryKinds,
    max_channels = MaxChannels,
    max_repository_members = MaxRepositoryMembers,
    max_repository_bytes = MaxRepositoryBytes,
    limits = {
      max_changes = MaxChanges,
      max_entries = MaxEntries,
      max_path_bytes = MaxPathBytes,
      max_file_bytes = MaxFileBytes,
      max_total_bytes = MaxTotalBytes,
    },
    allow_network = false,
    allow_credentials = false,
    allow_collaboration = false,
    allow_triggers = false,
    allow_status_publication = false,
    allow_mutation = false,
    non_claims = NonClaims,
  },
}
