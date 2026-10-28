(import "../web-cache.ncl") (
  (import "web-cache-positive.ncl")
  & {
    projects | force = [
      {
        project_id = "internal",
        producers = ["reviewed-builder-v1"],
        toolchains = ["rustc-pinned-v1"],
        platforms = ["linux-x86_64"],
        paths = ["../escape"],
      }
    ],
  }
)
