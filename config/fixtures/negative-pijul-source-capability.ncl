let source = import "../pijul-source-profile.ncl" in
source & {
  profile.capabilities = ["discover", "resolve_exact", "acquire", "observe_identity", "mutate"],
}
