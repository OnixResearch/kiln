let positive = import "hosted-service-positive.ncl" in
(import "../hosted-service.ncl") (
  positive
  & {
    execution | force =
      positive.execution
      & {
        cache | force =
          positive.execution.cache
          & {
            policies | force = [
              {
                organization = "acme",
                project = "app",
                runner = "local",
                worker = {
                  producer = "hosted",
                  toolchain = "compiler-v1",
                  platform = "linux",
                  inputs = ["Cargo.lock"],
                  paths = ["../credentials"],
                  maximum_entry_bytes = 4096,
                  maximum_files = 16,
                },
              }
            ],
          },
      },
  }
)
