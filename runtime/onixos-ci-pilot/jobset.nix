# Kiln owns this projection, not the OnixOS inventory/service implementation.
# Inputs are immutable consumer trees; plugin paths never come from ambient lookup.
{ nixpkgs, consumerInputs }:
let
  consumer = builtins.fetchTree consumerInputs.consumer;
  wasmSource = builtins.fetchTree consumerInputs.wasmSource;
  plugin = builtins.fetchTree consumerInputs.plugin;
  wasm = import "${wasmSource}/nix/wasm.nix" { plugins = plugin; };
  shim = import "${consumer}/nix/lib.nix" {
    inherit wasm;
    inherit (nixpkgs) lib;
    onixLib = "${consumer}/lib";
  };
  outputs = shim.mkFlake {
    inputs = { inherit nixpkgs; };
    inventoryFile = "${consumer}/examples/home-activation/inventory.ncl";
    modules.home-tools.file = "${consumer}/examples/home-activation/services/home-tools.ncl";
  };
  application = outputs.apps.x86_64-linux.onix-home-workstation-alice.package;
  negativeExitStatus = 19;
in
{
  cases = {
    success.home-activation = application;
    # Real application construction runs before this explicit negative control.
    build-failure.home-activation = application.overrideAttrs (previous: {
      name = "kiln-onixos-home-application-negative-control";
      buildCommand = previous.buildCommand + "\nexit ${toString negativeExitStatus}\n";
    });
    evaluation-failure = throw "kiln-onixos-pilot: explicit evaluation failure control";
  };
}
