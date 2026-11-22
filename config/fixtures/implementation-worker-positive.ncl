# Reviewed transport-v2 profile for the Lattice implementation CLI.
let worker = import "../implementation-worker.ncl" in
{
  transport | worker.Transport = {
    schema = "kiln.lattice-implementation-cli.v2",
    lattice_program = "/nix/store/00000000000000000000000000000000-lattice/bin/lattice",
    executable_blake3 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    state_directory = "/var/lib/kiln/lattice-state",
    output_directory = "/var/lib/kiln/lattice-transport",
  },
}
