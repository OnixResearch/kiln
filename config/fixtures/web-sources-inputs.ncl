{
  sources = [
    {
      id = "kiln",
      label = "Kiln repository",
      kind = "git",
      path = "/var/lib/kiln/sources/kiln",
    },
    {
      id = "lattice",
      label = "Lattice repository",
      kind = "jj",
      path = "/var/lib/kiln/sources/lattice",
    },
    {
      id = "redox",
      label = "Redox repository",
      kind = "pijul",
      path = "/var/lib/kiln/sources/redox",
      channel = "main",
      executable = "/run/current-system/sw/bin/pijul",
    },
  ],
}
