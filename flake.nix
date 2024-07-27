{
  description = "PID fan controller";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-23.11";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystem = f: nixpkgs.lib.genAttrs systems (system: f system);
    in
    {
      packages = forAllSystem (system: {
        default = nixpkgs.legacyPackages.${system}.callPackage ./package.nix { };
      });
      nixosModules = {
        pid-fan-controller = import ./settings.nix self;
        default = self.nixosModules.pid-fan-controller;
      };
    };
}
