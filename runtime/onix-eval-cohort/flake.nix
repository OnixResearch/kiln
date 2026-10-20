{
  description = "Exact optional Onix evaluator and Nickel plugin for Kiln discovery";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/34ca302a9572963c02e385c056be37c85ff51b77";
    nix-onix.url = "github:onixcomputer/nix/f93f28ad9dce007baae13aa461c16abf656d1171";
    onix-wasm.url = "github:onixcomputer/onix-wasm/54bf09574c42a21f9f38dc7e01bc44440e6fbc07";
    onixpkgs = {
      url = "git+ssh://git@github.com/OnixResearch/onixpkgs.git?rev=3bdf51c00947ffd6b8bd3355ff83509a03752042";
      flake = false;
    };
  };

  outputs = { nixpkgs, nix-onix, onix-wasm, onixpkgs, ... }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
      evaluator = pkgs.callPackage "${onixpkgs}/pkgs/nix-eval-jobs" {
        nixOnix = nix-onix;
      };
      plugin = onix-wasm.packages.${system}.nickel-plugin;
      jobset = builtins.path {
        path = ./jobset;
        name = "kiln-onix-discovery-jobset-v1";
      };
      manifest = pkgs.writeText "kiln-nix-eval-jobs-onix-cohort.json" (
        builtins.toJSON (import ./manifest.nix { inherit evaluator plugin jobset; })
      );
    in {
      lib = { inherit jobset; };
      packages.${system} = { inherit evaluator plugin manifest; };
      checks.${system}.cohort = import ./check.nix {
        inherit pkgs evaluator plugin manifest jobset;
      };
    };
}
