# Synthetic admission inputs, not installed executables or production identities.
{
  id = "patchgate-fixture",
  program = "/fixture/bin/patchgate",
  executable_identity = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  repository_root = "/fixture/repository",
  environment = { HOME = "/fixture/home" },
  native = {
    main = { channel = "main", state = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" },
    inbox = { channel = "inbox", state = "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB" },
    configuration = {
      source = "main",
      path = "patchgate.json",
      sha256 = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
      values = {
        policy_path = "POLICY.md",
        protected = ["src/**"],
        documentation_paths = ["docs/**"],
        max_change_chars = 8000,
        timeout_seconds = 300,
        check_timeout_seconds = 900,
        max_review_requests = 128,
        review_timeout_seconds = 900,
      },
    },
    policy = {
      path = "POLICY.md",
      sha256 = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
    },
  },
  limits = {
    max_changes = 32,
    max_edges = 256,
    max_request_bytes = 262144,
    max_report_bytes = 4194304,
    max_stderr_bytes = 65536,
    max_executable_bytes = 134217728,
    timeout_ms = 900000,
    poll_interval_ms = 10,
    teardown_timeout_ms = 5000,
  },
}
