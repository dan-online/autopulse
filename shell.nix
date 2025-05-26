{ pkgs ? import <nixpkgs> {}}: pkgs.mkShell {
    buildInputs = [
        pkgs.sqlite
        pkgs.postgresql
    ];
}