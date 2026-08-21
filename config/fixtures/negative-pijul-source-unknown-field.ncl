let source = import "../pijul-source-profile.ncl" in
source & {
  profile.ambient_authority = true,
}
