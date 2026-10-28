let positive = import "hosted-service-positive.ncl" in
(import "../hosted-service.ncl") (
  positive
  & {
    execution | force =
      positive.execution
      & {
        runners | force =
          positive.execution.runners
          & {
            vm | force =
              positive.execution.runners.vm
              & {
                root_image | force = { path = "/nix/store/fixture-image", blake3 = "not-a-blake3" },
              },
          },
      },
  }
)
