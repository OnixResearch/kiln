{
  description = "Pinned Lattice cohort for the Kiln Lattice worker guest";

  # Lattice keeps its own producer lock: the guest reuses the exact Lattice
  # package rather than rebuilding it against Kiln's nixpkgs.
  inputs.lattice.url = "github:OnixResearch/lattice/5a8e1d51c2e36100424b761c4fef65e6cd4c68dc";

  outputs =
    { lattice, ... }:
    {
      revision = lattice.rev;
      source = lattice.outPath;
      lattice = lattice.packages.x86_64-linux.lattice;
    };
}
