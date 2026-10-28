(import "../web-cache.ncl") {
  directory = "/var/lib/kiln-shared-cache",
  maximum_entry_bytes = 1048576,
  maximum_total_bytes = 16777216,
  maximum_entries = 64,
  maximum_age_seconds = 86400,
  maximum_files = 128,
  projects = [
    {
      project_id = "internal",
      producers = ["reviewed-builder-v1"],
      toolchains = ["rustc-pinned-v1"],
      platforms = ["linux-x86_64"],
      paths = ["target/cache"],
    }
  ],
}
