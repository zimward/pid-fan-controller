{pkgs ? import <nixpkgs> {}}:
pkgs.rustPlatform.buildRustPackage {
  pname = "pid-fan-controller";
  version = "0.1.1";
  src = ./.;
  cargoLock.lockFile = ./Cargo.lock;
}
