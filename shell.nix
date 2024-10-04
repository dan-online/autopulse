{ pkgs ? import <nixpkgs> {} }:
  pkgs.mkShell {
    buildInputs = [
      pkgs.sqlite
      pkgs.postgresql
      pkgs.libmysqlclient

      pkgs.ncurses # - required for mysql
    ];
}