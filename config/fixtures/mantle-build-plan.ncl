let mantle = import "/etc/kiln/mantle/lib.ncl" in
{
  name = "kiln-adapter-conformance",
  builder = "/bin/sh",
  args = ["-c", "printf 'kiln-mantle-adapter\n' > $out"],
} | mantle.Derivation
