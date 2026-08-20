# r[impl kiln.git.profile] r[impl kiln.git.capabilities] r[impl kiln.git.nonclaims]
let NonEmpty = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let Positive = std.contract.from_predicate (fun value =>
  std.is_number value && value > 0
) in
let ObjectFormat = [| 'Sha1 |] in
let Denied = std.contract.from_predicate (fun value => value == false) in
let Profile = {
  id | NonEmpty,
  library_source | NonEmpty,
  native_library_source | NonEmpty,
  license | NonEmpty,
  native_license | NonEmpty,
  object_format | ObjectFormat,
  repository_format_version | Number,
  platforms | Array NonEmpty,
  capabilities | Array NonEmpty,
  limits | {
    max_entries | Positive,
    max_path_bytes | Positive,
    max_file_bytes | Positive,
    max_total_bytes | Positive,
  },
  allow_network | Denied,
  allow_credentials | Denied,
  allow_mutation | Denied,
  allow_signing | Denied,
  allow_collaboration | Denied,
  allow_gitlinks | Denied,
  non_claims | Array NonEmpty,
} in
let MaxEntries = 4096 in
let MaxPathBytes = 512 in
let MaxFileBytes = 16777216 in
let MaxTotalBytes = 268435456 in
{
  profile | Profile = {
    id = "git-source-v1",
    library_source = "crates.io:git2:0.20.2",
    native_library_source = "crates.io:libgit2-sys:0.18.7+1.9.6",
    license = "MIT OR Apache-2.0",
    native_license = "GPL-2.0-only with libgit2 linking exception",
    object_format = 'Sha1,
    repository_format_version = 0,
    platforms = ["linux"],
    capabilities = ["discover", "resolve_exact", "acquire", "observe_identity"],
    limits = {
      max_entries = MaxEntries,
      max_path_bytes = MaxPathBytes,
      max_file_bytes = MaxFileBytes,
      max_total_bytes = MaxTotalBytes,
    },
    allow_network = false,
    allow_credentials = false,
    allow_mutation = false,
    allow_signing = false,
    allow_collaboration = false,
    allow_gitlinks = false,
    non_claims = [
      "local Git observations do not prove source or signature trust",
      "local Git observations do not prove complete history or remote availability",
      "local Git observations do not grant forge or collaboration authority",
    ],
  },
}
