let positive = import "hosted-service-positive.ncl" in
let base = positive.execution in
let vm = base.runners.microvm in
(import "../hosted-service.ncl") (
  positive
  & {
    execution | force =
      base
      & {
        runners | force =
          base.runners
          & {
            cloud = {
              kind = "cloud-hypervisor",
              cloud_hypervisor = vm.firecracker,
              kernel = vm.kernel,
              initrd = vm.initrd,
              guest_shell = "/bin/bash",
              vcpus = 1,
              memory_mib = 256,
              boot_timeout_ms = 10000,
              source_image_mib = 64,
              version = "v1.0",
            },
          }
      }
  }
)
