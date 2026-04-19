{
  pkgs ? (import <nixpkgs> { }),
}:
pkgs.mkShell {
  nativeBuildInputs = [
    pkgs.cargo
    pkgs.rust-analyzer
  ];
}
