{
  description = "Pinned optional Kiln runtime build cohort";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    octet = {
      url = "github:OnixResearch/octet/cf04e894e53eb0947230118a086ef6066ddba38c";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    kamacite-build-support = {
      url = "git+ssh://git@github.com/OnixResearch/kamacite.git?rev=d76fe4abe543724d8fc0ac4b362187caf2e27622";
      flake = false;
    };
  };

  outputs = { self, ... }: { };
}
