(import "../web-sources.ncl") (
  (import "web-sources-inputs.ncl")
  & {
    sources | force =
      std.array.map
        (fun source => source & { path | force = "relative/source" })
        (import "web-sources-inputs.ncl").sources,
  }
)
