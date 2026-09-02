# CONFORMANCE ONLY: synthetic repository and command; not deployment authority.
let make = import "../../config/github-job.ncl" in
make {
  owner = "example",
  repository = "project",
  program_path = "/bin/sh",
  args = ["-c", "test -f source.txt && test ! -e .git"],
  environment = {},
  source_secret = null,
}
