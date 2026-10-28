(import "../web-sources.ncl") (
  (import "web-sources-inputs.ncl")
  & {
    sources | force =
      std.array.map
        (fun source => source & { kind | force = "mercurial" })
        (import "web-sources-inputs.ncl").sources,
  }
)
