# Synthetic admission values; the runner supplies the measured base commit.
(import "./radicle-ci-live-distinct-status-inputs.ncl")
& {
  self_ci = {
    base_commit = "305f85d80b9ea05fbd91db73de8a6662a55650ee",
    root_selector = "packages.x86_64-linux",
    selected_task = "kiln",
    product_path = "bin/kiln",
    failure_marker = "self_ci_genuine_failure",
    source_limits = {
      max_identity_bytes = 4096,
      max_reference_bytes = 512,
      max_entries = 8192,
      max_path_bytes = 1024,
      max_file_bytes = 4194304,
      max_total_bytes = 67108864,
    },
  },
}
