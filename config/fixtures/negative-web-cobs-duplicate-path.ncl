(import "../web-cobs.ncl") (
  (import "web-cobs-inputs.ncl")
  & {
    repositories | force =
      (import "web-cobs-inputs.ncl").repositories
      @ [
        { id = "other", label = "Duplicate", repository_id = "fixture:other", git_directory = "/tmp/kiln-cobs-fixture.git" },
      ],
  }
)
