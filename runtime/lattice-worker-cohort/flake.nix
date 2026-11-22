{
  description = "Pinned Lattice cohort for the Kiln Lattice worker guest";

  # Lattice keeps its own producer lock: the guest reuses the exact Lattice
  # package rather than rebuilding it against Kiln's nixpkgs.
  inputs.lattice.url = "github:OnixResearch/lattice/661499d282c2e79ebefbd0f9b35beb5f8d7795cd";

  outputs =
    { lattice, ... }:
    {
      revision = lattice.rev;
      source = lattice.outPath;
      lattice = lattice.packages.x86_64-linux.lattice;
    };
}
