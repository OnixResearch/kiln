(import "../web-sources.ncl") (
  (import "web-sources-inputs.ncl")
  & {
    sources | force =
      (import "web-sources-inputs.ncl").sources
      @ [
        { id = "other", label = "Duplicate", kind = "git", path = "/var/lib/kiln/sources/kiln" },
      ],
  }
)
