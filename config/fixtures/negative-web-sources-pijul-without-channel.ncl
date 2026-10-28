(import "../web-sources.ncl") (
  (import "web-sources-inputs.ncl")
  & {
    sources | force = [
      {
        id = "pijul",
        label = "Pijul repository",
        kind = "pijul",
        path = "/var/lib/kiln/sources/pijul",
        executable = "/run/current-system/sw/bin/pijul",
      },
    ],
  }
)
