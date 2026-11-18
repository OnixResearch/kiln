# Rejected: two files share one path, so the corpus cannot say which bytes a
# projection materializes.
let contracts = import "../workspace-corpus-contracts.ncl" in
contracts.example
& {
  files | force = [
    { path = "projects/alpha/LICENSE", text = "MIT\n", executable = false },
    { path = "projects/alpha/LICENSE", text = "Apache-2.0\n", executable = false },
  ],
}
  | contracts.Corpus
  | contracts.Consistent
