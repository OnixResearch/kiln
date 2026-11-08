(import "../web-cobs.ncl") (
  (import "web-cobs-inputs.ncl")
  & {
    repositories | force =
      (import "web-cobs-inputs.ncl").repositories
      @ [
        { id = "other", label = "Duplicate", repository_id = "fixture:local-onix-cobs", git_directory = "/tmp/kiln-cobs-other.git" },
      ],
  }
)
