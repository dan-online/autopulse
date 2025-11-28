# TODO: migrate to flake and build dockerfile with nix
{
  pkgs ? import <nixpkgs> { },
}:
pkgs.mkShell {
  buildInputs = with pkgs; [
    nodejs
    yarn-berry
  ];
}
