(import "../web-cache.ncl") (
  (import "web-cache-positive.ncl")
  & {
    maximum_entry_bytes | force = 16777216,
    maximum_total_bytes | force = 1048576,
  }
)
