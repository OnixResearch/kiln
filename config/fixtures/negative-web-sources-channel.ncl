(import "../web-sources.ncl") (
  (import "web-sources-inputs.ncl")
  & {
    sources | force =
      std.array.map
        (fun source => if source.kind == "pijul" then source & { channel | force = "bad channel" } else source)
        (import "web-sources-inputs.ncl").sources,
  }
)
