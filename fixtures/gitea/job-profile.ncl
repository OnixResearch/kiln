# CONFORMANCE ONLY: synthetic repository and command; not deployment authority.
let make = import "../../config/gitea-job.ncl" in
make {
  owner = "example",
  repository = "project",
  git_base = "https://git.example.com",
  program_path = "/bin/sh",
  args = ["-c", "test -f source.txt && test ! -e .git"],
  environment = {},
  source_secret = null,
}
