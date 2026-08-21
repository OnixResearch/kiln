let source = import "../pijul-source-profile.ncl" in
source & {
  profile.allow_credentials = true,
}
