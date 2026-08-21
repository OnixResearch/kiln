let source = import "../pijul-source-profile.ncl" in
source & {
  profile.limits.max_changes = 0,
}
