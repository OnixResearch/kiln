let Config = import "../default.ncl" in
(Config & {
  hosting.mirrors = [{
    route = "https://mirror.invalid/kiln",
    authority = "writable_projection",
  }],
})
