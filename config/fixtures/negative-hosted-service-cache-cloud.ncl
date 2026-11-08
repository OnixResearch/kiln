let positive = import "hosted-service-cloud-hypervisor-positive.ncl" in
let base = positive.execution in
(import "../hosted-service.ncl") (
  positive
  & {
    execution | force =
      base
      & {
        cache | force = {
          directory = "/private/cache",
          maximum_entry_bytes = 1048576,
          maximum_total_bytes = 1073741824,
          maximum_entries = 4096,
          maximum_age_seconds = 3600,
          maximum_files = 4096,
          policies = [
            {
              organization = "acme",
              project = "app",
              runner = "cloud",
              worker = {
                producer = "hosted",
                toolchain = "rust",
                platform = "linux",
                inputs = ["Cargo.lock"],
                paths = ["target"],
                maximum_entry_bytes = 1024,
                maximum_files = 16,
              },
            }
          ],
        },
      }
  }
)
