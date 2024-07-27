{ rustPlatform, lib }:
let
  fs = lib.fileset;
in
rustPlatform.buildRustPackage {
  pname = "pid-fan-controller";
  version = "0.1.1";
  src = fs.toSource {
    root = ./.;
    fileset = fs.unions [
      ./Cargo.lock
      ./Cargo.toml
      ./src
    ];
  };
  cargoLock.lockFile = ./Cargo.lock;
}
