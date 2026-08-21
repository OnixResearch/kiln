let source = import "../pijul-source-profile.ncl" in
source & {
  profile.pijul_source_revision = "crates.io:sha256:substituted",
}
