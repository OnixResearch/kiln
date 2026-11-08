(import "../web-sources.ncl") (
  (import "web-sources-inputs.ncl")
  & {
    sources | force =
      (import "web-sources-inputs.ncl").sources
      @ [
        { id = "kiln", label = "Duplicate", kind = "git", path = "/var/lib/kiln/sources/other" },
      ],
  }
)
